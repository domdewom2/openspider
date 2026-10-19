//! Crawl-Kern: Start eines Crawls, Steuerschleife und das Handle zur Steuerung (Datenfluss M1-Plan 3.7).
//!
//! **Ablauf:** `start_crawl` prüft die Eingaben, legt die Crawl-Datei an, startet den Writer-Thread und
//! plant die Start-URL (Spider) bzw. die Listen-URLs (Listenmodus, CRAWL-027/028) ein. Die Steuerschleife
//! (`run`) hält so viele Worker am Laufen, wie `speed.threads` (live über [`CrawlHandle::set_threads`]
//! änderbar) vorgibt, veröffentlicht den Fortschritt (alle 100 ms), schreibt den Heartbeat (alle 2 s) und
//! prüft alle 5 s den freien Speicher (`disk_guard`). Sie endet bei leerer Warteschlange, erreichtem
//! URL-Limit, Stopp oder Pause. Dann wartet sie auf die laufenden Worker, schreibt den Endzustand, führt den
//! abschließenden Checkpoint aus und ruft `EngineDeps::on_finished` (nicht bei Pause, CRAWL-141). Die
//! Weiterleitungs-Flags (`url_flags`) gehen schon während des Laufs mit den Seiten-Batches in den Writer
//! (M1.md 3.11).
//!
//! **Pause** (CRAWL-126): `pause` beendet den Lauf mit Zustand `Paused`. Es startet kein neuer Abruf mehr,
//! laufende werden zu Ende geführt und gespeichert; die Warteschlange liegt ohnehin in der Datei. Der Lauf
//! vermerkt `paused_at` und leert `owner_pid`. Das Fortsetzen (`resume_crawl`, Datei `lifecycle.rs`) baut den
//! Kernzustand aus der Datei wieder auf (Datei `resume.rs`) und startet über [`launch`] denselben Lauf.

use std::collections::HashSet;
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
use os_store::{CrawlFile, CrawlInit, CrawlSink, SqliteSink};
use os_url::{NormUrl, RuleSet, Scope};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{interval, interval_at, Instant, MissedTickBehavior};
use url::Url;

use crate::diag;
use crate::error::EngineError;
use crate::frontier::{Cand, Core, CrawlScope, Pop, Purpose};
use crate::list_input::{self, ListOutcome};
use crate::progress::ProgressTracker;
use crate::sitemaps::SitemapSeed;
use crate::types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps, FinishedHook};
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
    /// Sendet ohne geborgten Handle; App-Sitzungssperren können vor dem Warten freigegeben werden.
    pub fn request_add_urls(
        &self,
        urls: Vec<String>,
    ) -> impl std::future::Future<Output = Result<crate::AddUrlsSummary, EngineError>> + Send + use<>
    {
        let sender = self.control.add_urls.clone();
        async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            sender
                .send((urls, tx))
                .map_err(|_| EngineError::Invalid("Crawl nicht mehr aktiv".into()))?;
            rx.await
                .map_err(|_| EngineError::Invalid("Crawl wurde beendet".into()))?
        }
    }

    /// Sendet zusätzliche Eingaben an die Steuerschleife (CRAWL-131).
    pub async fn add_urls(&self, urls: Vec<String>) -> Result<crate::AddUrlsSummary, EngineError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.control
            .add_urls
            .send((urls, tx))
            .map_err(|_| EngineError::Invalid("Crawl nicht mehr aktiv".into()))?;
        rx.await
            .map_err(|_| EngineError::Invalid("Crawl wurde beendet".into()))?
    }

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

/// Baut das robots.txt-Gate ausschließlich über `RobotsGateConfig::from_crawl_config` (M1-N04: ohne Uhr,
/// Token aus `user_agents::resolve`, nie die Preset-Kennung; kein Struct-Literal).
fn build_gate(cfg: &CrawlConfig) -> RobotsGate {
    RobotsGate::new(RobotsGateConfig::from_crawl_config(cfg))
}

/// Parse-Konfiguration: Pixelbreiten an, Token wie das robots-Gate.
fn parse_config(cfg: &CrawlConfig) -> ParseConfig {
    let mut p = ParseConfig::from_crawl_config(cfg);
    p.robots_token = os_fetch::user_agents::resolve(&cfg.user_agent).robots_token;
    p.pixel = true;
    p
}

/// Ausgangspunkte für Sitemaps beim Start: `sitemaps.urls` und eine Start-URL, die auf `.xml`/`.xml.gz` endet.
pub(crate) fn initial_sitemap_seeds(cfg: &CrawlConfig, start: Option<&Url>) -> Vec<SitemapSeed> {
    let mut seeds = crate::sitemaps::configured_seeds(cfg);
    if let Some(start) = start {
        if list_input::is_sitemap_url(start) {
            seeds.push(SitemapSeed {
                url: start.clone(),
                found_via: "start".to_string(),
            });
        }
    }
    seeds
}

/// Rückfall `JavaScript-Rendering` → Fehler (CRAWL-077: kein stiller Rückfall auf den Textmodus).
pub(crate) fn check_rendering(cfg: &CrawlConfig) -> Result<(), EngineError> {
    if cfg.rendering.mode != RenderingMode::Text {
        return Err(EngineError::Invalid(
            "JavaScript-Rendering ist in dieser Version nicht verfügbar (rendering.mode = text)"
                .to_string(),
        ));
    }
    Ok(())
}

/// Öffnet die Senke des Writers (Standard: `SqliteSink`).
pub(crate) fn open_sink(deps: &EngineDeps, file: &Path) -> Result<Box<dyn CrawlSink>, EngineError> {
    Ok(match &deps.sink_factory {
        Some(factory) => factory(file, deps.profile)?,
        None => Box::new(SqliteSink::open(file, deps.profile)?.with_clock(deps.clock.clone())),
    })
}

/// Startet einen neuen Crawl.
pub async fn start_crawl<F: Fetch>(
    fetcher: Arc<F>,
    req: CrawlRequest,
    deps: EngineDeps,
) -> Result<CrawlHandle, EngineError> {
    match &req.source {
        CrawlSource::Spider { start_url } => {
            let mut start = os_url::parse_start_url(start_url)
                .map_err(|e| EngineError::Invalid(format!("Start-URL: {e}")))?;
            if req.config.advanced.crawl_fragments {
                if let Some((_, fragment)) = start_url.split_once('#') {
                    start.url.set_fragment(Some(fragment));
                    start = crate::fragments::normalize(&start.url, true);
                }
            }
            start_spider(fetcher, req, deps, start).await
        }
        CrawlSource::List { items, label } => {
            let (items, label) = (items.clone(), label.clone());
            start_list(fetcher, req, deps, items, label).await
        }
    }
}

/// Gemeinsame Teile eines neuen Crawls.
struct Fresh {
    cfg: Arc<CrawlConfig>,
    rules: RuleSet,
    crawl_id: CrawlId,
    now: os_model::Timestamp,
}

fn prepare_fresh(
    req: &CrawlRequest,
    deps: &EngineDeps,
    mode: CrawlMode,
) -> Result<Fresh, EngineError> {
    let mut cfg = req.config.clone();
    cfg.mode = mode;
    cfg.speed.threads = cfg.speed.threads.clamp(1, MAX_THREADS);
    check_rendering(&cfg)?;
    let rules = RuleSet::compile(
        &cfg.scope.include,
        &cfg.scope.exclude,
        cfg.scope.regex_partial_match,
    )
    .map_err(|e| EngineError::Invalid(e.to_string()))?;
    let now = deps.clock.now();
    Ok(Fresh {
        cfg: Arc::new(cfg),
        rules,
        crawl_id: deps.ids.new_crawl_id(now),
        now,
    })
}

/// Metadaten des ersten Batches.
fn start_meta(deps: &EngineDeps, now: os_model::Timestamp) -> Vec<(String, String)> {
    vec![
        (
            "state".to_string(),
            CrawlState::Running.as_str().to_string(),
        ),
        ("started_at".to_string(), now.to_db()),
        ("heartbeat_at".to_string(), now.to_db()),
        ("owner_kind".to_string(), deps.owner_kind.to_string()),
        ("owner_pid".to_string(), std::process::id().to_string()),
    ]
}

/// Ereignis „Wenig freier Speicherplatz“ beim Start (CRAWL-124).
fn start_disk_event(file: &Path, deps: &EngineDeps) -> Option<CrawlEvent> {
    crate::disk_guard::start_warning(file).map(|message| CrawlEvent {
        at: deps.clock.now(),
        kind: "warn".to_string(),
        code: "disk_space_low".to_string(),
        message,
    })
}

async fn start_spider<F: Fetch>(
    fetcher: Arc<F>,
    req: CrawlRequest,
    deps: EngineDeps,
    start: NormUrl,
) -> Result<CrawlHandle, EngineError> {
    let Fresh {
        cfg,
        rules,
        crawl_id,
        now,
    } = prepare_fresh(&req, &deps, CrawlMode::Spider)?;
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
    let writer = WriterThread::spawn(open_sink(&deps, &req.file)?);

    let mut core = Core::new(cfg.limits.max_urls, cfg.limits.max_depth);
    core.limits = cfg.limits.clone();
    if cfg.advanced.cookies == os_model::CookieMode::SessionOnly {
        core.sessions[0] = Some(Arc::new(CookieJar::new()));
    }
    let scope = CrawlScope::new(Scope::new(&start, &cfg.scope).with_cdns(&cfg.cdns));

    // Start-URL und Metadaten. Ist die Start-URL eine Sitemap-Datei, holt sie ausschließlich der Sitemap-Ingest
    // (K-41a); ihre URL-Zeile entsteht mit dem Ergebnis des Abrufs, nicht als Crawl-URL.
    let seeds = initial_sitemap_seeds(&cfg, Some(&start.url));
    let start_is_sitemap = seeds.iter().any(|s| s.found_via == "start");
    let mut first = WriteBatch::default();
    if !start_is_sitemap {
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
                source: None,
            },
            Some(0),
            0,
            0,
            &rules,
            &mut first,
        );
    }
    first.meta = start_meta(&deps, now);
    first.events.extend(start_disk_event(&req.file, &deps));
    if writer.sender().send(first).await.is_err() {
        return Err(writer_error(writer.finish().err()));
    }
    Ok(launch(Setup {
        fetcher,
        cfg,
        rules,
        deps,
        file: req.file,
        crawl_id,
        core,
        scope,
        start_encoded: start.encoded,
        writer,
        tracker_started_at: now,
        tracker_elapsed: Duration::ZERO,
        crawled: 0,
        paused_total: Duration::ZERO,
        primed_origins: HashSet::new(),
        seeds,
    }))
}

async fn start_list<F: Fetch>(
    fetcher: Arc<F>,
    req: CrawlRequest,
    deps: EngineDeps,
    items: Vec<String>,
    label: String,
) -> Result<CrawlHandle, EngineError> {
    let Fresh {
        cfg,
        rules,
        crawl_id,
        now,
    } = prepare_fresh(&req, &deps, CrawlMode::List)?;
    let mut plan = list_input::plan(&items, &cfg)?;
    if cfg.advanced.crawl_fragments {
        crate::fragments::restore_list_fragments(&mut plan, &cfg)?;
    }
    // Anker des Crawl-Bereichs: die erste gültige Listen-URL, sonst die erste Sitemap-Adresse
    let anchor = plan
        .entries
        .iter()
        .find(|e| {
            matches!(
                e.outcome,
                ListOutcome::Ok | ListOutcome::Excluded | ListOutcome::NotIncluded
            )
        })
        .or_else(|| {
            plan.entries
                .iter()
                .find(|e| e.outcome == ListOutcome::Sitemap)
        })
        .and_then(|e| e.url.clone())
        .ok_or_else(|| EngineError::Invalid("Die Liste enthält keine gültige URL".to_string()))?;

    let file_handle = CrawlFile::create(
        &req.file,
        &CrawlInit {
            crawl_id,
            mode: CrawlMode::List,
            start_url: None,
            list_source: Some(label),
            project: req.project.clone(),
            name: req.name.clone(),
            config: (*cfg).clone(),
            created_at: now,
        },
        deps.profile,
    )?;

    let max_depth = cfg.limits.max_depth.or(Some(0));
    let mut core = Core::new(cfg.limits.max_urls, max_depth);
    core.limits = cfg.limits.clone();
    core.list_mode = true;
    if cfg.advanced.cookies == os_model::CookieMode::SessionOnly {
        core.sessions[0] = Some(Arc::new(CookieJar::new()));
    }
    let mut scope = CrawlScope::new(Scope::new(&anchor, &cfg.scope).with_cdns(&cfg.cdns));
    for e in &plan.entries {
        if let Some(u) = &e.url {
            if let Some(host) = u.url.host_str() {
                scope.add_host(host);
            }
        }
    }

    let mut first = WriteBatch::default();
    let mut seeds = initial_sitemap_seeds(&cfg, None);
    let mut rows: Vec<(u64, String, Option<i64>, &'static str)> = Vec::new();
    for e in &plan.entries {
        let id = match (&e.url, e.outcome) {
            (Some(norm), ListOutcome::Ok | ListOutcome::Excluded | ListOutcome::NotIncluded) => {
                // „Nur Sitzung“ (CRAWL-088): Jede Listen-URL ist ein eigener Seed mit eigenem Cookie-Speicher,
                // den ihre Weiterleitungsziele erben.
                let session = if e.outcome == ListOutcome::Ok
                    && cfg.advanced.cookies == os_model::CookieMode::SessionOnly
                {
                    core.new_session(Some(Arc::new(CookieJar::new())))
                } else {
                    0
                };
                Some(
                    core.discover(
                        &Cand {
                            norm,
                            internal: true,
                            kind: DiscoveryKind::ListInput,
                            store: true,
                            crawl: true,
                            reason: None,
                            resource_like: false,
                            apply_rules: true,
                            purpose: Purpose::Page,
                            source: None,
                        },
                        Some(0),
                        0,
                        session,
                        &rules,
                        &mut first,
                    )
                    .0,
                )
            }
            (Some(norm), ListOutcome::Duplicate) => core.lookup(&norm.encoded).map(|s| s.id.0),
            (Some(norm), ListOutcome::Sitemap) => {
                seeds.push(SitemapSeed {
                    url: norm.url.clone(),
                    found_via: "list".to_string(),
                });
                None
            }
            _ => None,
        };
        rows.push((e.position, e.original.clone(), id, e.outcome.as_str()));
    }
    {
        let conn = file_handle.conn();
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(os_store::StoreError::from)?;
        for (position, original, id, outcome) in &rows {
            // Zahlen und Ergebnis stehen als Literale im SQL (nur Ganzzahlen bzw. feste Wörter), der Text
            // der Eingabe als Parameter.
            let url_id = id.map_or_else(|| "NULL".to_string(), |i| i.to_string());
            conn.execute(
                &format!(
                    "INSERT INTO list_input (position, original, url_id, outcome) \
                     VALUES ({position}, ?1, {url_id}, '{outcome}')"
                ),
                [original.as_str()],
            )
            .map_err(os_store::StoreError::from)?;
        }
        conn.execute_batch("COMMIT")
            .map_err(os_store::StoreError::from)?;
    }
    drop(file_handle);

    let c = &plan.counts;
    for (key, value) in [
        ("list_read", c.read),
        ("list_invalid", c.invalid),
        ("list_ignored_lines", c.ignored_lines),
        ("list_duplicates", c.duplicates),
        ("list_excluded", c.excluded),
        ("list_not_included", c.not_included),
        ("list_sitemaps", c.sitemaps),
        ("list_to_crawl", c.to_crawl),
    ] {
        first.counters.push((key.to_string(), value as i64));
    }
    first.meta = start_meta(&deps, now);
    first.events.extend(start_disk_event(&req.file, &deps));
    let writer = WriterThread::spawn(open_sink(&deps, &req.file)?);
    if writer.sender().send(first).await.is_err() {
        return Err(writer_error(writer.finish().err()));
    }
    Ok(launch(Setup {
        fetcher,
        cfg,
        rules,
        deps,
        file: req.file,
        crawl_id,
        core,
        scope,
        start_encoded: String::new(),
        writer,
        tracker_started_at: now,
        tracker_elapsed: Duration::ZERO,
        crawled: 0,
        paused_total: Duration::ZERO,
        primed_origins: HashSet::new(),
        seeds,
    }))
}

fn writer_error(e: Option<os_store::StoreError>) -> EngineError {
    match e {
        Some(e) => EngineError::Store(e),
        None => EngineError::Join("Writer-Thread ist beendet".to_string()),
    }
}

/// Alles, was [`launch`] braucht, um den Lauf eines Crawls zu starten (neu oder fortgesetzt).
pub(crate) struct Setup<F: Fetch> {
    pub fetcher: Arc<F>,
    pub cfg: Arc<CrawlConfig>,
    pub rules: RuleSet,
    pub deps: EngineDeps,
    pub file: PathBuf,
    pub crawl_id: CrawlId,
    pub core: Core,
    pub scope: CrawlScope,
    pub start_encoded: String,
    pub writer: WriterThread,
    pub tracker_started_at: os_model::Timestamp,
    /// Bisherige Laufzeit des Crawls (Fortsetzen), Pausen eingeschlossen.
    pub tracker_elapsed: Duration,
    pub crawled: u64,
    pub paused_total: Duration,
    pub primed_origins: HashSet<String>,
    pub seeds: Vec<SitemapSeed>,
}

/// Bausteine für den gemeinsamen Zustand (auch für „Neu crawlen“ benutzt).
pub(crate) struct SharedInit<F: Fetch> {
    pub fetcher: Arc<F>,
    pub cfg: Arc<CrawlConfig>,
    pub clock: Arc<dyn Clock>,
    pub scope: CrawlScope,
    pub rules: RuleSet,
    pub core: Core,
    pub writer: crate::writer_thread::WriterSender,
    pub tracker: ProgressTracker,
    pub start_encoded: String,
    pub primed_origins: HashSet<String>,
}

/// Gemeinsamer Zustand aus den Bausteinen; liefert auch die Steuerung (`Control`).
pub(crate) fn make_shared<F: Fetch>(mut init: SharedInit<F>) -> (Arc<Shared<F>>, Arc<Control>) {
    init.core.limits = init.cfg.limits.clone();
    if init.cfg.scope.subfolder_only {
        if let Ok(start) = Url::parse(&init.start_encoded) {
            let path = start.path();
            let folder = path.rfind('/').map_or("/", |i| &path[..=i]).to_string();
            init.core.folder_policy = Some((folder, init.cfg.scope.crawl_outside_start_folder));
        }
    }
    let (add_urls, add_rx) = tokio::sync::mpsc::unbounded_channel();
    let control = Arc::new(Control {
        add_urls,
        notify: tokio::sync::Notify::new(),
        threads: AtomicU32::new(init.cfg.speed.threads),
        stop: AtomicU8::new(STOP_NONE),
        reason: Mutex::new(None),
    });
    let sh = Arc::new(Shared {
        sitemap_ingest_lock: Default::default(),
        add_rx: tokio::sync::Mutex::new(add_rx),
        fetcher: init.fetcher,
        gate: build_gate(&init.cfg),
        pcfg: parse_config(&init.cfg),
        clock: init.clock,
        scope: init.scope,
        rules: init.rules,
        core: Mutex::new(init.core),
        ctl: Arc::clone(&control),
        writer: init.writer,
        progress: Mutex::new(init.tracker),
        abort: AtomicBool::new(false),
        error: Mutex::new(None),
        host_pacer: crate::host_pace::HostPacer::new(init.cfg.speed.max_per_host),
        start_url: init.start_encoded,
        cfg: init.cfg,
        primed_origins: init.primed_origins,
    });
    (sh, control)
}

/// Fortschrittsrechner für einen (fortgesetzten) Lauf.
pub(crate) fn make_tracker(
    cfg: &CrawlConfig,
    clock: &dyn Clock,
    started_at: os_model::Timestamp,
    elapsed: Duration,
    crawled: u64,
    paused_total: Duration,
) -> ProgressTracker {
    let limit = (cfg.limits.max_urls > 0).then_some(cfg.limits.max_urls);
    let mut tracker = ProgressTracker::new(started_at, clock.monotonic(), limit);
    if crawled > 0 || !paused_total.is_zero() || !elapsed.is_zero() {
        tracker.resume_with_elapsed(crawled, paused_total, elapsed);
    }
    tracker
}

/// Startet den Lauf: Steuerschleife als Task, Handle zur Steuerung.
pub(crate) fn launch<F: Fetch>(setup: Setup<F>) -> CrawlHandle {
    let Setup {
        fetcher,
        cfg,
        rules,
        deps,
        file,
        crawl_id,
        mut core,
        scope,
        start_encoded,
        writer,
        tracker_started_at,
        tracker_elapsed,
        crawled,
        paused_total,
        primed_origins,
        seeds,
    } = setup;
    let deps = crate::memory::resource_deps(deps);
    // Nur Kennung und Betriebsparameter loggen: URLs und Konfiguration können Geheimnisse enthalten.
    tracing::info!(%crawl_id, threads = cfg.speed.threads, "Crawl gestartet");
    // RAM-Fenster der Warteschlange (K-78 b); beim Fortsetzen hat `resume::restore` es schon eingeschaltet
    if !core.frontier.spill_enabled() {
        core.frontier
            .enable_spill(crate::queue::queue_window(), file.clone());
    }
    let hidden_result = crate::frontier::restore_hidden(&file, &cfg, &mut core);
    let mut tracker = make_tracker(
        &cfg,
        &*deps.clock,
        tracker_started_at,
        tracker_elapsed,
        crawled,
        paused_total,
    );
    tracker.track_file(&file);
    let (sh, control) = make_shared(SharedInit {
        fetcher,
        cfg,
        clock: deps.clock.clone(),
        scope,
        rules,
        core,
        writer: writer.sender(),
        tracker,
        start_encoded,
        primed_origins,
    });
    if let Err(error) = hidden_result {
        sh.fail(error);
    }
    let (queued, discovered) = {
        let c = sh.core.lock().unwrap_or_else(|p| p.into_inner());
        (c.frontier.len() as u64, c.discovered())
    };
    let initial = sh
        .progress
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .snapshot(deps.clock.monotonic(), queued, discovered, 0);
    crate::memory::guard_start(&sh);
    let (progress_tx, progress_rx) = watch::channel(initial);
    let task = tokio::spawn(run(RunParts {
        sh,
        writer,
        progress_tx,
        crawl_id,
        file,
        hook: deps.on_finished.clone(),
        seeds,
        clock: deps.clock.clone(),
    }));
    CrawlHandle {
        crawl_id,
        progress: progress_rx,
        control,
        task,
    }
}

struct RunParts<F: Fetch> {
    sh: Arc<Shared<F>>,
    writer: WriterThread,
    progress_tx: watch::Sender<Progress>,
    crawl_id: CrawlId,
    file: PathBuf,
    hook: Option<FinishedHook>,
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
    let (queued, discovered, active, memory) = {
        let core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
        (
            core.frontier.len() as u64,
            core.discovered(),
            core.active as u32,
            core.memory(),
        )
    };
    diag::record_core_memory(&memory);
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

    // Sitemaps aus Konfiguration bzw. Start-URL bzw. Liste
    if !seeds.is_empty() {
        if let Err(worker::Failure::Engine(e)) =
            crate::sitemap_ingest::ingest_sitemaps(&sh, seeds).await
        {
            sh.fail(e);
        }
    }

    let mut set: JoinSet<()> = JoinSet::new();
    let mut progress_tick = interval(PROGRESS_INTERVAL);
    progress_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut heartbeat = interval_at(Instant::now() + HEARTBEAT_INTERVAL, HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut disk_tick = interval_at(
        Instant::now() + crate::disk_guard::CHECK_INTERVAL,
        crate::disk_guard::CHECK_INTERVAL,
    );
    disk_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // Hintergrundarbeit für das RAM-Fenster der Warteschlange (Auslagern, Nachladen), höchstens eine zugleich
    let mut maint: JoinSet<()> = JoinSet::new();
    let queue_failures = Arc::new(AtomicU32::new(0));

    let mut last_snapshot = clock.monotonic();
    let mut added = sh.add_rx.lock().await;
    let end = loop {
        diag::inc(&diag::STATS.run_loops);
        if sh.abort.load(Ordering::SeqCst) || sh.writer.failed() {
            break End::Failed;
        }
        match ctl.stop.load(Ordering::SeqCst) {
            STOP_STOP => break End::Stopped,
            STOP_PAUSE => break End::Paused,
            _ => {}
        }
        sh.lock_core().wake_deferred(clock.monotonic());
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
                        Pop::Empty | Pop::LimitBlocked | Pop::NeedRefill => None,
                    }
                }
            };
            match popped {
                Some(item) => {
                    diag::inc(&diag::STATS.run_pops);
                    set.spawn(worker::process(Arc::clone(&sh), item));
                }
                None => break,
            }
        }
        // Ende? Außerdem: ist Auslagern oder Nachladen der Warteschlange fällig?
        {
            let mut core = sh.core.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(plan) = core.next_queue_job() {
                maint.spawn(crate::queue_maint::run(
                    Arc::clone(&sh),
                    plan,
                    Arc::clone(&queue_failures),
                ));
            }
            if core.active == 0 {
                if core.frontier.is_empty() && core.deferred.is_empty() {
                    break End::QueueEmpty;
                }
                if core.started >= core.max_urls {
                    break End::LimitReached;
                }
            }
        }
        let due = sh
            .lock_core()
            .next_due()
            .map(|due| due.saturating_sub(clock.monotonic()));
        tokio::select! {
            _ = tokio::time::sleep(due.unwrap_or(Duration::from_secs(3600))), if due.is_some() => {},
            Some((urls, reply)) = added.recv() => {
                let result = crate::lifecycle::admit_added(&mut sh.lock_core(), &urls, &sh);
                let _ = reply.send(result);
            }
            _ = ctl.notify.notified() => {}
            Some(joined) = set.join_next(), if !set.is_empty() => {
                if let Err(e) = joined {
                    if e.is_panic() {
                        sh.fail(EngineError::Join(format!("Worker abgestürzt: {e}")));
                    }
                }
            }
            _ = progress_tick.tick() => {
                publish(&sh, &progress_tx);
                let now = clock.monotonic();
                if now.saturating_sub(last_snapshot) >= Duration::from_secs(1) {
                    last_snapshot = now;
                    let snapshot = progress_tx.borrow().clone();
                    match crate::memory::tick(&crate::memory::MemoryInputs { progress: &snapshot, config: &sh.cfg }) {
                        crate::memory::MemoryAction::None => {},
                        crate::memory::MemoryAction::Throttle(n) => { ctl.threads.store(n.max(1), Ordering::SeqCst); },
                        crate::memory::MemoryAction::Pause(reason) => {
                            *ctl.reason.lock().unwrap_or_else(|p| p.into_inner()) = Some(("memory", reason));
                            ctl.stop.store(STOP_PAUSE, Ordering::SeqCst);
                        }
                    }
                    let mut batch = WriteBatch::default();
                    batch.meta.push(("progress_snapshot".into(), serde_json::to_string(&snapshot).map_err(|e| EngineError::Invalid(e.to_string()))?));
                    if sh.writer.send(batch).await.is_err() { sh.abort.store(true, Ordering::SeqCst); }
                }
            },
            _ = heartbeat.tick() => {
                let mut b = WriteBatch::default();
                b.meta.push(("heartbeat_at".to_string(), clock.now().to_db()));
                rule_events(&sh, &mut b);
                if sh.writer.send(b).await.is_err() {
                    sh.abort.store(true, Ordering::SeqCst);
                }
            }
            _ = disk_tick.tick() => {
                let check = crate::disk_guard::check(&file);
                if check.level == os_store::files::DiskLevel::Critical {
                    *ctl.reason.lock().unwrap_or_else(|p| p.into_inner()) = Some((
                        crate::disk_guard::PAUSE_REASON,
                        crate::disk_guard::pause_message(check.free_gib),
                    ));
                    ctl.stop.store(STOP_PAUSE, Ordering::SeqCst);
                }
            }
        }
    };
    drop(added);

    // laufende Worker zu Ende bringen
    while let Some(joined) = set.join_next().await {
        if let Err(e) = joined {
            if e.is_panic() {
                sh.fail(EngineError::Join(format!("Worker abgestürzt: {e}")));
            }
        }
    }
    // Auch ein beim Pausieren noch laufender Abruf kann eine Wiederholung zurückstellen.
    sh.lock_core().restore_deferred();

    // Die Hintergrundarbeit hält einen Verweis auf den gemeinsamen Zustand und damit den Writer offen: abwarten.
    while maint.join_next().await.is_some() {}

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
    tracing::info!(%crawl_id, state = state.as_str(), crawled, "Crawl beendet");

    if sh.cfg.advanced.crawl_fragments && end != End::Failed {
        sh.writer
            .flush()
            .await
            .map_err(|_| EngineError::Join("Fragment-Basis nicht geschrieben".into()))?;
        crate::fragments::materialize(&file, &sh.writer).await?;
    }

    let mut last = WriteBatch::default();
    last.meta
        .push(("state".to_string(), state.as_str().to_string()));
    last.meta.push((
        "end_reason".to_string(),
        reason.map(|r| r.as_str().to_string()).unwrap_or_default(),
    ));
    let at = clock.now().to_db();
    last.meta.push(("heartbeat_at".to_string(), at.clone()));
    // Der Prozess besitzt den Crawl nicht mehr (F-14 des M2-Konzepts); der Determinismus-Test bleibt gültig.
    last.meta.push(("owner_pid".to_string(), String::new()));
    if state == CrawlState::Paused {
        last.meta.push(("paused_at".to_string(), at));
        let reason_code = ctl.reason.lock().unwrap_or_else(|p| p.into_inner()).take();
        last.meta.push((
            "pause_reason".to_string(),
            reason_code
                .as_ref()
                .map(|(code, _)| (*code).to_string())
                .unwrap_or_default(),
        ));
        if let Some((code, message)) = reason_code {
            last.events.push(CrawlEvent {
                at: clock.now(),
                kind: "warn".to_string(),
                code: code.to_string(),
                message,
            });
        }
    } else {
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

    let summary = CrawlSummary {
        crawl_id,
        file,
        state,
        end_reason: reason,
        crawled,
    };
    // CRAWL-141: Hook nach regulärem Ende, Limit oder Stopp – nach dem Checkpoint, vor der Auflösung von
    // `wait()`; nie bei Pause oder Fehler.
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
