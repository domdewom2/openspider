//! Lebenszyklus: Fortsetzen nach Pause, Absturz oder Limit und Konfigurationsänderung während der Pause
//! (CRAWL-125, CRAWL-126, CRAWL-128).
//!
//! **Fortsetzen** ([`resume_crawl`]): Zustand und Warteschlange liegen in der Crawl-Datei (`resume.rs`). Der
//! Aufruf prüft zuerst den Zustand (`Pausiert` und `Unterbrochen` lassen sich fortsetzen, `Abgeschlossen` nur mit
//! nicht leerer Warteschlange – z. B. nach einem Limit –, `Fehlgeschlagen` und ein lebender Lauf nicht), wendet
//! die geänderte Konfiguration an, verwirft Warteschlangeneinträge, die die Include/Exclude-Regeln jetzt
//! ausschließen, setzt `owner_kind`/`owner_pid` neu und startet denselben Lauf wie `start_crawl`.
//!
//! **Änderbar in der Pause:** Limits, Include/Exclude, Geschwindigkeit/Threads, Erfassungsschalter,
//! Speichern/Crawlen-Schalter, User-Agent, Header, Authentifizierung (wirken auf noch nicht abgerufene URLs;
//! den Abrufer baut der Aufrufer aus der neuen Konfiguration). **Nicht änderbar:** Modus, Start-URL (nicht Teil
//! der Konfiguration), Crawl-Bereich, Rendering-Modus – [`check_config_change`] weist sie mit „neuer Crawl
//! nötig“ ab. [`apply_paused_config`] wendet eine Änderung sofort an (Konfigurationsdialog „OK“): sie speichert
//! die Konfiguration und verwirft passende Warteschlangeneinträge, damit der Zähler „verbleibend“ gleich sinkt.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use os_fetch::Fetch;
use os_model::{
    CrawlConfig, CrawlMode, CrawlState, NotCrawledReason, QueueOp, Timestamp, UrlId, UrlState,
    UrlUpdate, WriteBatch,
};
use os_store::{CrawlFile, OpenMode, SqliteSink};
use os_url::{RuleDecision, RuleSet, Scope};

use crate::crawl::{self, CrawlHandle, Setup, MAX_THREADS};
use crate::error::EngineError;
use crate::frontier::{CrawlScope, COUNTER_EXCLUDED, COUNTER_NOT_INCLUDED};
use crate::resume::{self, PrimedFetch};
use crate::types::EngineDeps;

/// Ergebnis von [`apply_paused_config`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PausedConfigReport {
    /// Warteschlangeneinträge, die die neuen Regeln verworfen haben.
    pub queue_removed: u64,
    /// Einträge, die in der Warteschlange bleiben.
    pub queue_remaining: u64,
}

fn changed(name: &str) -> EngineError {
    EngineError::Invalid(format!(
        "{name} lässt sich bei einem begonnenen Crawl nicht ändern – neuer Crawl nötig"
    ))
}

/// Prüft, ob die neue Konfiguration für einen pausierten oder fortgesetzten Crawl zulässig ist.
pub fn check_config_change(old: &CrawlConfig, new: &CrawlConfig) -> Result<(), EngineError> {
    if old.mode != new.mode {
        return Err(changed("Der Modus"));
    }
    let (a, b) = (&old.scope, &new.scope);
    if a.all_subdomains != b.all_subdomains
        || a.subfolder_only != b.subfolder_only
        || a.crawl_outside_start_folder != b.crawl_outside_start_folder
    {
        return Err(changed("Der Crawl-Bereich"));
    }
    if old.rendering.mode != new.rendering.mode {
        return Err(changed("Der Rendering-Modus"));
    }
    Ok(())
}

fn join_err(e: tokio::task::JoinError) -> EngineError {
    EngineError::Join(e.to_string())
}

/// Ob ein Prozess den Crawl gerade abarbeitet (Zustand „Running“ mit frischem Heartbeat).
pub(crate) fn ensure_not_running(
    file: &CrawlFile,
    deps: &EngineDeps,
) -> Result<CrawlState, EngineError> {
    let state = file.effective_state(&*deps.clock)?;
    if state == CrawlState::Running {
        let pid = file.get_meta("owner_pid")?.unwrap_or_default();
        let who = if pid.is_empty() {
            String::new()
        } else {
            format!(" (Prozess {pid})")
        };
        return Err(EngineError::Invalid(format!(
            "Der Crawl läuft noch{who} – pausieren oder beenden, dann fortsetzen"
        )));
    }
    Ok(state)
}

/// Verwirft Warteschlangeneinträge, die die Regeln in `cfg` ausschließen; die Start-URL ist ausgenommen.
fn purge_queue(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    start_encoded: Option<&str>,
    deps: &EngineDeps,
) -> Result<u64, EngineError> {
    // eigener RuleSet: die Trefferzähler des Laufs bleiben unberührt
    let rules = RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    if cfg.scope.include.is_empty() && cfg.scope.exclude.is_empty() {
        return Ok(0);
    }
    let mut batch = WriteBatch::default();
    let (mut excluded, mut not_included) = (0i64, 0i64);
    {
        let mut stmt = file
            .conn()
            .prepare("SELECT q.url_id, u.url FROM queue q JOIN urls u ON u.id = q.url_id")
            .map_err(os_store::StoreError::from)?;
        let mut rows = stmt.query([]).map_err(os_store::StoreError::from)?;
        while let Some(row) = rows.next().map_err(os_store::StoreError::from)? {
            let id = UrlId(row.get(0).map_err(os_store::StoreError::from)?);
            let url: String = row.get(1).map_err(os_store::StoreError::from)?;
            if start_encoded == Some(url.as_str()) {
                continue;
            }
            let reason = match rules.decide(&url) {
                RuleDecision::Allowed => continue,
                RuleDecision::Excluded(_) => {
                    excluded += 1;
                    NotCrawledReason::Excluded
                }
                RuleDecision::NotIncluded => {
                    not_included += 1;
                    NotCrawledReason::NotIncluded
                }
            };
            batch.queue.push(QueueOp::Pop { id });
            batch.url_updates.push(UrlUpdate::State {
                id,
                state: UrlState::Discovered,
                reason: Some(reason),
            });
            batch
                .url_updates
                .push(UrlUpdate::InTables { id, value: false });
        }
    }
    let removed = (excluded + not_included) as u64;
    if removed == 0 {
        return Ok(0);
    }
    if excluded > 0 {
        batch
            .counters
            .push((COUNTER_EXCLUDED.to_string(), excluded));
    }
    if not_included > 0 {
        batch
            .counters
            .push((COUNTER_NOT_INCLUDED.to_string(), not_included));
    }
    let mut sink = SqliteSink::open(file.path(), deps.profile)?.with_clock(deps.clock.clone());
    os_store::CrawlSink::write(&mut sink, batch)?;
    Ok(removed)
}

fn queue_len(file: &CrawlFile) -> Result<u64, EngineError> {
    let n: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .map_err(os_store::StoreError::from)?;
    Ok(n.max(0) as u64)
}

fn start_encoded(file: &CrawlFile) -> Result<Option<String>, EngineError> {
    Ok(file.meta()?.start_url)
}

/// Wendet eine geänderte Konfiguration auf einen pausierten (oder unterbrochenen, beendeten) Crawl an: prüft
/// die Änderung ([`check_config_change`]), speichert die Konfiguration in der Datei und verwirft
/// Warteschlangeneinträge, die Include/Exclude jetzt ausschließen. Ein laufender Crawl wird abgewiesen.
pub fn apply_paused_config(
    file: &Path,
    new: &CrawlConfig,
    deps: &EngineDeps,
) -> Result<PausedConfigReport, EngineError> {
    let handle = CrawlFile::open(file, OpenMode::ReadWrite)?;
    ensure_not_running(&handle, deps)?;
    let old = handle.config()?;
    check_config_change(&old, new)?;
    crawl::check_rendering(new)?;
    let mut cfg = new.clone();
    cfg.speed.threads = cfg.speed.threads.clamp(1, MAX_THREADS);
    // ungültige Regeln vor dem Speichern abweisen
    RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    handle.set_config_with_clock(&cfg, &*deps.clock)?;
    let start = start_encoded(&handle)?;
    let removed = purge_queue(&handle, &cfg, start.as_deref(), deps)?;
    Ok(PausedConfigReport {
        queue_removed: removed,
        queue_remaining: queue_len(&handle)?,
    })
}

/// Alles, was `resume_crawl` ohne Async vorbereitet.
struct Prepared {
    setup_cfg: Arc<CrawlConfig>,
    rules: RuleSet,
    restored: resume::Restored,
    scope: CrawlScope,
    start_encoded: String,
    start_url: Option<url::Url>,
    crawl_id: os_model::CrawlId,
    started_at: Timestamp,
    elapsed: Duration,
    paused_total: Duration,
    mode: CrawlMode,
    first: WriteBatch,
}

fn millis_between(later: Timestamp, earlier: Timestamp) -> Duration {
    Duration::from_millis(
        u64::try_from((later.unix_millis() - earlier.unix_millis()).max(0)).unwrap_or(0),
    )
}

fn prepare(
    path: &Path,
    config: Option<CrawlConfig>,
    deps: &EngineDeps,
) -> Result<Prepared, EngineError> {
    let file = CrawlFile::open(path, OpenMode::ReadWrite)?;
    let state = ensure_not_running(&file, deps)?;
    let meta = file.meta()?;
    match state {
        CrawlState::Failed => {
            return Err(EngineError::Invalid(
                "Der Crawl ist fehlgeschlagen und lässt sich nicht fortsetzen".to_string(),
            ))
        }
        CrawlState::Completed if queue_len(&file)? == 0 => {
            return Err(EngineError::Invalid(
                "Der Crawl ist abgeschlossen (Warteschlange leer) – es gibt nichts fortzusetzen"
                    .to_string(),
            ))
        }
        _ => {}
    }

    let old = file.config()?;
    let mut cfg = match config {
        Some(new) => {
            check_config_change(&old, &new)?;
            file.set_config_with_clock(&new, &*deps.clock)?;
            new
        }
        None => old,
    };
    cfg.mode = meta.mode;
    cfg.speed.threads = cfg.speed.threads.clamp(1, MAX_THREADS);
    crawl::check_rendering(&cfg)?;
    let rules = RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let start_enc = meta.start_url.clone();
    purge_queue(&file, &cfg, start_enc.as_deref(), deps)?;

    let mut restored = resume::restore(&file, &cfg, meta.mode, true)?;
    let (scope, start_encoded, start_url) = match meta.mode {
        CrawlMode::Spider => {
            let enc = start_enc.ok_or_else(|| {
                EngineError::Invalid("Die Crawl-Datei enthält keine Start-URL".to_string())
            })?;
            let norm = os_url::parse_start_url(&enc)
                .map_err(|e| EngineError::Invalid(format!("Start-URL: {e}")))?;
            (
                CrawlScope::new(Scope::new(&norm, &cfg.scope).with_cdns(&cfg.cdns)),
                norm.encoded.clone(),
                Some(norm.url),
            )
        }
        CrawlMode::List => {
            let anchor = restored
                .anchor
                .as_deref()
                .and_then(|a| os_url::parse_start_url(a).ok())
                .ok_or_else(|| {
                    EngineError::Invalid("Die Crawl-Datei enthält keine Listen-URL".to_string())
                })?;
            let hosts = std::mem::take(&mut restored.list_hosts);
            (
                CrawlScope::with_hosts(Scope::new(&anchor, &cfg.scope).with_cdns(&cfg.cdns), hosts),
                String::new(),
                None,
            )
        }
    };

    // Zeiten: Die Zeit zwischen Pause (bzw. letztem Heartbeat) und jetzt gilt als Pause.
    let now = deps.clock.now();
    let started_at = meta.started_at.unwrap_or(now);
    let paused_before = file
        .get_meta("paused_total_ms")?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let paused_from = file
        .get_meta("paused_at")?
        .filter(|v| !v.is_empty())
        .and_then(|v| Timestamp::from_db(&v).ok())
        .or(meta.heartbeat_at)
        .unwrap_or(now);
    let gap = millis_between(now, paused_from);
    let paused_total = Duration::from_millis(paused_before) + gap;
    let elapsed = millis_between(now, started_at);

    let mut first = WriteBatch {
        meta: vec![
            (
                "state".to_string(),
                CrawlState::Running.as_str().to_string(),
            ),
            ("heartbeat_at".to_string(), now.to_db()),
            ("owner_kind".to_string(), deps.owner_kind.to_string()),
            ("owner_pid".to_string(), std::process::id().to_string()),
            ("ended_at".to_string(), String::new()),
            ("end_reason".to_string(), String::new()),
            ("paused_at".to_string(), String::new()),
            ("pause_reason".to_string(), String::new()),
            (
                "paused_total_ms".to_string(),
                paused_total.as_millis().to_string(),
            ),
        ],
        ..WriteBatch::default()
    };
    if let Some(message) = crate::disk_guard::start_warning(path) {
        first.events.push(os_model::CrawlEvent {
            at: now,
            kind: "warn".to_string(),
            code: "disk_space_low".to_string(),
            message,
        });
    }

    Ok(Prepared {
        setup_cfg: Arc::new(cfg),
        rules,
        restored,
        scope,
        start_encoded,
        start_url,
        crawl_id: meta.crawl_id,
        started_at,
        elapsed,
        paused_total,
        mode: meta.mode,
        first,
    })
}

/// Setzt einen pausierten oder abgebrochenen Crawl fort (optional mit geänderter Konfiguration).
///
/// Der Abrufer gehört zur Konfiguration (User-Agent, Header, Authentifizierung, Auflösung): Wer sie in der
/// Pause geändert hat, übergibt einen neu gebauten Abrufer. Die robots.txt-Dateien bereits bekannter Hosts
/// kommen aus der Crawl-Datei und werden nicht noch einmal abgerufen.
pub async fn resume_crawl<F: Fetch>(
    fetcher: Arc<F>,
    file: PathBuf,
    config: Option<CrawlConfig>,
    deps: EngineDeps,
) -> Result<CrawlHandle, EngineError> {
    let prepared = {
        let (path, deps) = (file.clone(), deps.clone());
        tokio::task::spawn_blocking(move || prepare(&path, config, &deps))
            .await
            .map_err(join_err)??
    };
    let Prepared {
        setup_cfg,
        rules,
        mut restored,
        scope,
        start_encoded,
        start_url,
        crawl_id,
        started_at,
        elapsed,
        paused_total,
        mode,
        first,
    } = prepared;

    // Sitemaps: bereits abgerufene Dateien überspringt der Ingest (`Core::sitemap_keys`); noch fehlende
    // (neu konfigurierte, durch einen Abbruch verlorene) werden jetzt geholt.
    let mut seeds = crawl::initial_sitemap_seeds(&setup_cfg, start_url.as_ref());
    if mode == CrawlMode::List {
        for s in std::mem::take(&mut restored.list_sitemaps) {
            if let Ok(norm) = os_url::parse_start_url(&s) {
                seeds.push(crate::sitemaps::SitemapSeed {
                    url: norm.url,
                    found_via: "list".to_string(),
                });
            }
        }
    }

    let writer = crate::writer_thread::WriterThread::spawn(crawl::open_sink(&deps, &file)?);
    if writer.sender().send(first).await.is_err() {
        return Err(match writer.finish().err() {
            Some(e) => EngineError::Store(e),
            None => EngineError::Join("Writer-Thread ist beendet".to_string()),
        });
    }
    let primed_origins: HashSet<String> = restored
        .primed
        .iter()
        .map(|(_, p)| p.origin.clone())
        .collect();
    let primed_fetch = Arc::new(PrimedFetch::new(fetcher, &restored.primed));
    Ok(crawl::launch(Setup {
        fetcher: primed_fetch,
        cfg: setup_cfg,
        rules,
        deps,
        file,
        crawl_id,
        core: restored.core,
        scope,
        start_encoded,
        writer,
        tracker_started_at: started_at,
        tracker_elapsed: elapsed,
        crawled: restored.crawled,
        paused_total,
        primed_origins,
        seeds,
    }))
}

// STUB(M2-E02)
pub(crate) fn admit_added<F: os_fetch::Fetch>(
    _core: &mut crate::frontier::Core,
    _urls: &[String],
    _shared: &crate::worker::Shared<F>,
) -> Result<crate::AddUrlsSummary, EngineError> {
    Err(EngineError::NotImplemented("M2-E02"))
}
// STUB(M2-E02)
pub fn add_urls_to_file(
    _file: &mut CrawlFile,
    _urls: &[String],
    _cfg: &CrawlConfig,
) -> Result<crate::AddUrlsSummary, EngineError> {
    Err(EngineError::NotImplemented("M2-E02"))
}
