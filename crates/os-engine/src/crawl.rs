//! Crawl-Kern: Start eines Crawls, Steuerschleife und das Handle zur Steuerung (Datenfluss M1-Plan 3.7).
//!
//! **Ablauf:** `start_crawl` prüft die Eingaben, legt die Crawl-Datei an, startet den Writer-Thread und
//! plant die Start-URL ein. Die Steuerschleife (`run`) hält so viele Worker am Laufen, wie `speed.threads`
//! (live über [`CrawlHandle::set_threads`] änderbar) vorgibt, veröffentlicht den Fortschritt (alle 100 ms)
//! und schreibt den Heartbeat (alle 2 s). Sie endet bei leerer Warteschlange, erreichtem URL-Limit, Stopp
//! oder Pause. Dann wartet sie auf die laufenden Worker, schreibt den Endzustand, führt den abschließenden
//! Checkpoint aus, speichert die Weiterleitungs-Flags und ruft `EngineDeps::on_finished` (nicht bei Pause).
//!
//! `pause` beendet den Lauf mit Zustand `Paused`; die Warteschlange liegt in der Datei. Das Fortsetzen
//! (`resume_crawl`) setzt P19 um.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_fetch::robots::{RobotsGate, RobotsGateConfig};
use os_fetch::{CookieJar, Fetch};
use os_model::{
    Clock, CrawlConfig, CrawlEvent, CrawlId, CrawlMode, CrawlState, DiscoveryKind, EndReason,
    Progress, RenderingMode, WriteBatch,
};
use os_parse::ParseConfig;
use os_store::{CrawlFile, CrawlInit, CrawlSink, OpenMode, SqliteSink};
use os_url::{RuleSet, Scope};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{interval, interval_at, Instant, MissedTickBehavior};
use url::Url;

use crate::error::EngineError;
use crate::frontier::{Cand, Core, Pop, Purpose};
use crate::progress::ProgressTracker;
use crate::sitemaps::SitemapSeed;
use crate::types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps};
use crate::worker::{self, Control, Shared, STOP_NONE, STOP_PAUSE, STOP_STOP};
use crate::writer_thread::WriterThread;

/// Höchstzahl paralleler Threads (OS-Grenze, CRAWL-112).
pub const MAX_THREADS: u32 = 200;
/// Abstand der Fortschrittsmeldungen.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
/// Abstand des Heartbeats in `crawl_meta` (M1-Plan 3.7).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);

/// Steuerung eines laufenden Crawls.
pub struct CrawlHandle {
    crawl_id: CrawlId,
    progress: watch::Receiver<Progress>,
    control: Arc<Control>,
    task: JoinHandle<Result<CrawlSummary, EngineError>>,
}

impl CrawlHandle {
    /// Kennung des Crawls.
    pub fn crawl_id(&self) -> CrawlId {
        self.crawl_id
    }

    /// Aktueller Fortschritt.
    pub fn progress(&self) -> Progress {
        self.progress.borrow().clone()
    }

    /// Beobachter des Fortschritts.
    pub fn watch(&self) -> watch::Receiver<Progress> {
        self.progress.clone()
    }

    /// Pausiert den Crawl: keine neuen Abrufe, laufende werden gespeichert, Zustand `Paused`, die
    /// Warteschlange bleibt in der Datei. `wait()` liefert danach die Zusammenfassung (kein Abschluss-Hook).
    pub fn pause(&self) {
        self.control.stop.store(STOP_PAUSE, Ordering::SeqCst);
        self.control.notify.notify_one();
    }

    /// Beendet den Crawl; die Warteschlange bleibt erhalten. Laufende Abrufe werden noch gespeichert.
    pub fn stop(&self) {
        self.control.stop.store(STOP_STOP, Ordering::SeqCst);
        self.control.notify.notify_one();
    }

    /// Ändert die Zahl paralleler Threads (1 bis 200); wirkt sofort auf neue Abrufe (CRAWL-112: ≤ 2 s).
    pub fn set_threads(&self, n: u32) {
        self.control
            .threads
            .store(n.clamp(1, MAX_THREADS), Ordering::SeqCst);
        self.control.notify.notify_one();
    }

    /// Wartet auf das Ende und liefert die Zusammenfassung.
    pub async fn wait(self) -> Result<CrawlSummary, EngineError> {
        self.task
            .await
            .map_err(|e| EngineError::Join(e.to_string()))?
    }
}

/// Baut das robots.txt-Gate. Einzige Stelle, die den Konstruktor aufruft: nach M1-N04 ist es
/// `RobotsGateConfig::from_crawl_config(cfg)` ohne Uhr (Token aus `user_agents::resolve`).
fn build_gate(cfg: &CrawlConfig, clock: Arc<dyn Clock>) -> RobotsGate {
    RobotsGate::new(RobotsGateConfig::from_crawl_config(cfg, clock))
}

/// Parse-Konfiguration: Pixelbreiten an, Token wie das robots-Gate.
fn parse_config(cfg: &CrawlConfig) -> ParseConfig {
    let mut p = ParseConfig::from_crawl_config(cfg);
    p.robots_token = os_fetch::user_agents::resolve(&cfg.user_agent).robots_token;
    p.pixel = true;
    p
}

/// Ausgangspunkte für Sitemaps beim Start: `sitemaps.urls` und eine Start-URL, die auf `.xml`/`.xml.gz` endet.
fn initial_sitemap_seeds(cfg: &CrawlConfig, start: &Url) -> Vec<SitemapSeed> {
    let mut seeds: Vec<SitemapSeed> = cfg
        .sitemaps
        .urls
        .iter()
        .filter_map(|u| Url::parse(u.trim()).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|url| SitemapSeed {
            url,
            found_via: "config".to_string(),
        })
        .collect();
    let path = start.path().to_ascii_lowercase();
    if path.ends_with(".xml") || path.ends_with(".xml.gz") {
        seeds.push(SitemapSeed {
            url: start.clone(),
            found_via: "start".to_string(),
        });
    }
    seeds
}

/// Startet einen neuen Crawl.
pub async fn start_crawl<F: Fetch>(
    fetcher: Arc<F>,
    req: CrawlRequest,
    deps: EngineDeps,
) -> Result<CrawlHandle, EngineError> {
    let CrawlSource::Spider { start_url } = &req.source else {
        return Err(EngineError::NotImplemented("Listenmodus (M1-P19)"));
    };
    let start = os_url::parse_start_url(start_url)
        .map_err(|e| EngineError::Invalid(format!("Start-URL: {e}")))?;
    let mut cfg = req.config.clone();
    cfg.mode = CrawlMode::Spider;
    cfg.speed.threads = cfg.speed.threads.clamp(1, MAX_THREADS);
    if cfg.rendering.mode != RenderingMode::Text {
        // CRAWL-077: kein stiller Rückfall auf den Textmodus; JavaScript-Rendering kommt mit M2.
        return Err(EngineError::Invalid(
            "JavaScript-Rendering ist in dieser Version nicht verfügbar (rendering.mode = text)"
                .to_string(),
        ));
    }
    let rules = RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let cfg = Arc::new(cfg);

    // Crawl-Datei
    let now = deps.clock.now();
    let crawl_id = deps.ids.new_crawl_id(now);
    drop(CrawlFile::create(
        &req.file,
        &CrawlInit {
            crawl_id,
            mode: CrawlMode::Spider,
            start_url: Some(start.encoded.clone()),
            list_source: None,
            project: req.project.clone(),
            name: req.name.clone(),
            config: (*cfg).clone(),
            created_at: now,
        },
        deps.profile,
    )?);
    let sink: Box<dyn CrawlSink> = match &deps.sink_factory {
        Some(factory) => factory(&req.file, deps.profile)?,
        None => Box::new(SqliteSink::open(&req.file, deps.profile)?.with_clock(deps.clock.clone())),
    };
    let writer = WriterThread::spawn(sink);

    // gemeinsamer Zustand
    let max_depth = cfg.limits.max_depth;
    let mut core = Core::new(cfg.limits.max_urls, max_depth);
    if cfg.advanced.cookies == os_model::CookieMode::SessionOnly {
        core.sessions[0] = Some(Arc::new(CookieJar::new()));
    }
    let limit = (cfg.limits.max_urls > 0).then_some(cfg.limits.max_urls);
    let tracker = ProgressTracker::new(now, deps.clock.monotonic(), limit);
    let control = Arc::new(Control {
        notify: tokio::sync::Notify::new(),
        threads: AtomicU32::new(cfg.speed.threads),
        stop: AtomicU8::new(STOP_NONE),
    });
    let scope = Scope::new(&start, &cfg.scope);

    // Start-URL und Metadaten
    let mut first = WriteBatch::default();
    core.discover(
        &Cand {
            norm: &start,
            internal: true,
            kind: DiscoveryKind::Start,
            store: true,
            crawl: true,
            reason: None,
            resource_like: false,
            apply_rules: false,
            purpose: Purpose::Page,
        },
        Some(0),
        0,
        0,
        &rules,
        &mut first,
    );
    first.meta = vec![
        (
            "state".to_string(),
            CrawlState::Running.as_str().to_string(),
        ),
        ("started_at".to_string(), now.to_db()),
        ("heartbeat_at".to_string(), now.to_db()),
        ("owner_kind".to_string(), deps.owner_kind.to_string()),
        ("owner_pid".to_string(), std::process::id().to_string()),
    ];
    if writer.sender().send(first).await.is_err() {
        return Err(writer_error(writer.finish().err()));
    }

    let (progress_tx, progress_rx) = watch::channel(Progress {
        state: CrawlState::Running,
        end_reason: None,
        crawled: 0,
        queued: 1,
        percent: 0,
        rate: 0.0,
        avg_response_time: None,
        elapsed_s: 0,
        eta_s: None,
        active_threads: 0,
        started_at: now,
        avg_rate: 0.0,
        paused_s: 0,
        discovered: 1,
    });

    let sh = Arc::new(Shared {
        fetcher,
        gate: build_gate(&cfg, deps.clock.clone()),
        pcfg: parse_config(&cfg),
        clock: deps.clock.clone(),
        scope,
        rules,
        core: Mutex::new(core),
        ctl: Arc::clone(&control),
        writer: writer.sender(),
        progress: Mutex::new(tracker),
        abort: AtomicBool::new(false),
        error: Mutex::new(None),
        host_gates: Mutex::new(std::collections::HashMap::new()),
        max_per_host: cfg.speed.max_per_host.filter(|n| *n > 0),
        start_url: start.encoded.clone(),
        cfg: Arc::clone(&cfg),
    });
    let seeds = initial_sitemap_seeds(&cfg, &start.url);
    let file = req.file.clone();
    let hook = deps.on_finished.clone();
    let task = tokio::spawn(run(RunParts {
        sh,
        writer,
        progress_tx,
        crawl_id,
        file,
        hook,
        seeds,
        clock: deps.clock.clone(),
    }));
    Ok(CrawlHandle {
        crawl_id,
        progress: progress_rx,
        control,
        task,
    })
}

fn writer_error(e: Option<os_store::StoreError>) -> EngineError {
    match e {
        Some(e) => EngineError::Store(e),
        None => EngineError::Join("Writer-Thread ist beendet".to_string()),
    }
}

struct RunParts<F: Fetch> {
    sh: Arc<Shared<F>>,
    writer: WriterThread,
    progress_tx: watch::Sender<Progress>,
    crawl_id: CrawlId,
    file: PathBuf,
    hook: Option<crate::types::FinishedHook>,
    seeds: Vec<SitemapSeed>,
    clock: Arc<dyn Clock>,
}

/// Wie der Lauf endete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    QueueEmpty,
    LimitReached,
    Stopped,
    Paused,
    Failed,
}

fn publish<F: Fetch>(sh: &Shared<F>, tx: &watch::Sender<Progress>) {
    let (queued, discovered, active) = {
        let core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
        (
            core.frontier.len() as u64,
            core.discovered(),
            core.active as u32,
        )
    };
    let snapshot = sh
        .progress
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .snapshot(sh.clock.monotonic(), queued, discovered, active);
    tx.send_replace(snapshot);
}

fn rule_events<F: Fetch>(sh: &Shared<F>, batch: &mut WriteBatch) {
    for w in sh.rules.take_warnings() {
        let list = match w.kind {
            os_url::RuleKind::Include => "Include",
            os_url::RuleKind::Exclude => "Exclude",
        };
        batch
            .counters
            .push((format!("regex_too_complex:{}", w.pattern), 1));
        batch.events.push(CrawlEvent {
            at: sh.clock.now(),
            kind: "warn".to_string(),
            code: "regex_too_complex".to_string(),
            message: format!("{list}-Zeile {}: {} ({})", w.line + 1, w.message, w.pattern),
        });
    }
}

async fn run<F: Fetch>(parts: RunParts<F>) -> Result<CrawlSummary, EngineError> {
    let RunParts {
        sh,
        writer,
        progress_tx,
        crawl_id,
        file,
        hook,
        seeds,
        clock,
    } = parts;
    let ctl = Arc::clone(&sh.ctl);

    // Sitemaps aus Konfiguration bzw. Start-URL
    if !seeds.is_empty() {
        if let Err(worker::Failure::Engine(e)) = worker::ingest_sitemaps(&sh, seeds).await {
            sh.fail(e);
        }
    }

    let mut set: JoinSet<()> = JoinSet::new();
    let mut progress_tick = interval(PROGRESS_INTERVAL);
    progress_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut heartbeat = interval_at(Instant::now() + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let end = loop {
        if sh.abort.load(Ordering::SeqCst) || sh.writer.failed() {
            break End::Failed;
        }
        match ctl.stop.load(Ordering::SeqCst) {
            STOP_STOP => break End::Stopped,
            STOP_PAUSE => break End::Paused,
            _ => {}
        }
        // freie Plätze mit Arbeit füllen
        let target = ctl.threads.load(Ordering::SeqCst) as usize;
        loop {
            let popped = {
                let mut core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
                if core.active >= target {
                    None
                } else {
                    match core.try_pop() {
                        Pop::Item(item) => Some(item),
                        Pop::Empty | Pop::LimitBlocked => None,
                    }
                }
            };
            match popped {
                Some(item) => {
                    set.spawn(worker::process(Arc::clone(&sh), item));
                }
                None => break,
            }
        }
        // Ende?
        {
            let core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
            if core.active == 0 {
                if core.frontier.is_empty() {
                    break End::QueueEmpty;
                }
                if core.started >= core.max_urls {
                    break End::LimitReached;
                }
            }
        }
        tokio::select! {
            _ = ctl.notify.notified() => {}
            Some(joined) = set.join_next(), if !set.is_empty() => {
                if let Err(e) = joined {
                    if e.is_panic() {
                        sh.fail(EngineError::Join(format!("Worker abgestürzt: {e}")));
                    }
                }
            }
            _ = progress_tick.tick() => publish(&sh, &progress_tx),
            _ = heartbeat.tick() => {
                let mut b = WriteBatch::default();
                b.meta.push(("heartbeat_at".to_string(), clock.now().to_db()));
                rule_events(&sh, &mut b);
                if sh.writer.send(b).await.is_err() {
                    sh.abort.store(true, Ordering::SeqCst);
                }
            }
        }
    };

    // laufende Worker zu Ende bringen
    while let Some(joined) = set.join_next().await {
        if let Err(e) = joined {
            if e.is_panic() {
                sh.fail(EngineError::Join(format!("Worker abgestürzt: {e}")));
            }
        }
    }

    // Endzustand
    let (state, reason) = match end {
        End::QueueEmpty => (CrawlState::Completed, Some(EndReason::QueueEmpty)),
        End::LimitReached => (CrawlState::Completed, Some(EndReason::LimitReached)),
        End::Stopped => (CrawlState::Completed, Some(EndReason::Stopped)),
        End::Paused => (CrawlState::Paused, None),
        End::Failed => (CrawlState::Failed, None),
    };
    let now_mono = clock.monotonic();
    let crawled = {
        let mut tracker = sh.progress.lock().unwrap_or_else(|p| p.into_inner());
        tracker.finish(now_mono, state, reason);
        tracker.crawled()
    };
    publish(&sh, &progress_tx);

    let mut last = WriteBatch::default();
    last.meta
        .push(("state".to_string(), state.as_str().to_string()));
    last.meta.push((
        "end_reason".to_string(),
        reason.map(|r| r.as_str().to_string()).unwrap_or_default(),
    ));
    let at = clock.now().to_db();
    last.meta.push(("heartbeat_at".to_string(), at.clone()));
    if state != CrawlState::Paused {
        last.meta.push(("ended_at".to_string(), at));
    }
    for hit in sh.rules.include_hits() {
        last.counters
            .push((format!("include_hits:{}", hit.line + 1), hit.hits as i64));
    }
    rule_events(&sh, &mut last);
    let mut write_ok = sh.writer.send(last).await.is_ok();
    write_ok = write_ok && writer.flush().await.is_ok();
    write_ok = write_ok && writer.checkpoint().await.is_ok();

    let flags = sh
        .core
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .redirects
        .flags();
    let shared_error = sh.error.lock().unwrap_or_else(|p| p.into_inner()).take();
    drop(sh); // gibt den letzten Absender frei, damit der Writer-Thread enden kann
    let finished = writer.finish();

    if let Some(e) = shared_error {
        return Err(e);
    }
    if let Err(e) = finished {
        return Err(EngineError::Store(e));
    }
    if !write_ok || end == End::Failed {
        return Err(EngineError::Join(
            "Crawl wegen eines Fehlers beim Schreiben abgebrochen".to_string(),
        ));
    }
    if !flags.is_empty() {
        persist_flags(&file, &flags)?;
    }

    let summary = CrawlSummary {
        crawl_id,
        file,
        state,
        end_reason: reason,
        crawled,
    };
    if state == CrawlState::Completed {
        if let Some(hook) = hook {
            let s = summary.clone();
            tokio::task::spawn_blocking(move || hook(&s))
                .await
                .map_err(|e| EngineError::Join(e.to_string()))?;
        }
    }
    Ok(summary)
}

/// Schreibt die Flags (`url_flags`) mit einer eigenen Verbindung: der Schreibweg des Stores kennt sie noch
/// nicht (`WriteBatch` hat kein Feld dafür, Schnittstellenbedarf an den Orchestrator).
fn persist_flags(
    path: &Path,
    flags: &[(os_model::UrlId, &'static str)],
) -> Result<(), EngineError> {
    use os_store::StoreError;
    let file = CrawlFile::open(path, OpenMode::ReadWrite)?;
    let conn = file.conn();
    let tx = conn.unchecked_transaction().map_err(StoreError::from)?;
    for (id, flag) in flags {
        tx.execute(
            "INSERT OR REPLACE INTO url_flags (url_id, flag, value) VALUES (?1, ?2, NULL)",
            (id.0, *flag),
        )
        .map_err(StoreError::from)?;
    }
    tx.commit().map_err(StoreError::from)?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .map_err(StoreError::from)?;
    Ok(())
}
