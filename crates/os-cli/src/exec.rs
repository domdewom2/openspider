//! Ablauf eines Laufs: Crawl (neu, fortgesetzt oder geladen) → Crawl-Analyse → Exporte → Speichern.
//!
//! Phasen für `--json-progress`: `validate` (in `lib.rs`), `crawl`, `analysis`, `export`, `done`.
//! Reihenfolge der Schlusszeile: Sie ist immer die letzte Zeile auf stderr und nennt Ergebnis und
//! Exit-Code.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use os_analyze::analysis::persist::{self, RunState};
use os_analyze::analysis::{ensure_analysis, trigger};
use os_analyze::views::ViewQuery;
use os_engine::types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps, FinishedHook};
use os_engine::{disk_guard, resume_crawl, start_crawl, CrawlHandle, EngineError};
use os_export::catalog::bulk_issues;
use os_export::{export_bulk, export_tab, save_report, ExportFileInfo, Format};
use os_fetch::secrets::KeyringSecretStore;
use os_fetch::{FetchConfig, HttpFetcher, SecretStore};
use os_model::{
    Clock, CrawlConfig, CrawlId, CrawlState, IdSource, Progress, RandomIds, SystemClock, Timestamp,
};
use os_store::{queries, registry, CrawlFile, OpenMode, StorageProfile};
use tokio::time::{interval_at, Instant, MissedTickBehavior};

use crate::exit::{CliError, Exit};
use crate::plan::{Plan, Source};
use crate::registry::Resolved;
use crate::reporter::Reporter;
use crate::signals::Signals;

/// Ergebnis eines Laufs.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Exit-Code.
    pub exit: Exit,
    /// Kurzfassung für die Schlusszeile (ohne Exit-Code).
    pub summary: String,
    /// Crawl-Datei des Laufs, sobald es eine gibt (für die Laufhistorie, OUT-169).
    pub crawl_path: Option<PathBuf>,
    /// Crawl-ID des Laufs.
    pub crawl_id: Option<String>,
    /// Anzahl gecrawlter URLs.
    pub crawled: Option<u64>,
}

impl Outcome {
    /// Ergebnis ohne Angaben zum Crawl.
    pub fn new(exit: Exit, summary: impl Into<String>) -> Self {
        Outcome {
            exit,
            summary: summary.into(),
            crawl_path: None,
            crawl_id: None,
            crawled: None,
        }
    }

    fn with_crawl(mut self, path: &Path, crawled: Option<u64>) -> Self {
        self.crawl_id = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        self.crawl_path = Some(path.to_path_buf());
        self.crawled = crawled;
        self
    }
}

/// `IdSource`, die genau die vorab gewählte Crawl-ID liefert: Die Datei im Crawl-Ordner heißt
/// `<crawl_id>.ospider` und muss vor dem Start feststehen.
struct FixedId(CrawlId);

impl IdSource for FixedId {
    fn new_crawl_id(&self, _now: Timestamp) -> CrawlId {
        self.0
    }
}

/// Stand nach dem Crawl-Teil.
struct Crawled {
    path: PathBuf,
    crawled: u64,
}

/// Führt den Plan aus; jeder Fehler wird zu einem Ergebnis mit Exit-Code.
pub async fn execute(plan: Plan, rep: Arc<Reporter>, signals: Signals) -> Outcome {
    match run(&plan, &rep, &signals).await {
        Ok(outcome) => outcome,
        Err(e) => {
            rep.error(&e.message);
            Outcome::new(e.exit, e.headline().to_string())
        }
    }
}

async fn run(plan: &Plan, rep: &Arc<Reporter>, signals: &Signals) -> Result<Outcome, CliError> {
    if signals.is_interrupted() {
        return Ok(Outcome::new(
            Exit::Interrupted,
            "Unterbrochen vor dem Start des Crawls",
        ));
    }

    // INTEG-017 / OUT-201: credentials are validated against the host's store before any crawl.
    if plan.resume || !matches!(plan.source, Source::Load { .. }) {
        preflight_apis(plan, rep)?;
    }

    let crawled = match (&plan.source, plan.resume) {
        (Source::Load { path }, false) => open_only(path, rep)?,
        _ => {
            let result = crawl_phase(plan, rep, signals).await?;
            match result {
                CrawlEnd::Done(c) => c,
                CrawlEnd::Interrupted { id } => {
                    return Ok(Outcome::new(
                        Exit::Interrupted,
                        format!(
                            "Unterbrochen – der Crawl ist pausiert und gespeichert (ID {id}); \
                             fortsetzen mit: openspider --load-crawl {id} --resume"
                        ),
                    )
                    .with_crawl(&registry::path_for(&plan.paths.crawls, &id), None));
                }
            }
        }
    };

    // Start-URL erreichbar? (nur neue Spider-Crawls)
    if let (Source::Spider { start_url }, false) = (&plan.source, plan.resume) {
        if let Some(problem) = start_url_problem(&crawled.path, start_url) {
            rep.warn(format!(
                "Start-URL {} nicht erreichbar: {problem}",
                os_fetch::redact::url(start_url)
            ));
            if plan.fail_on_start_error {
                return Err(CliError::crawl(format!(
                    "Start-URL nicht erreichbar ({problem}) – Abbruch wegen --fail-on-start-error"
                )));
            }
        }
    }

    let mut failures = 0u32;
    let mut files = 0usize;
    if plan.output.is_some() {
        match export_phase(plan, rep, signals, &crawled) {
            ExportEnd::Done {
                failures: f,
                files: n,
            } => {
                failures = f;
                files = n;
            }
            ExportEnd::Interrupted => {
                return Ok(Outcome::new(
                    Exit::Interrupted,
                    "Unterbrochen – die Exporte sind unvollständig",
                )
                .with_crawl(&crawled.path, Some(crawled.crawled)));
            }
        }
    }

    if plan.no_store {
        discard(&crawled.path, rep);
    }

    rep.phase("done");
    let warnings = rep.warnings();
    let base = format!(
        "{} URLs gecrawlt, {} Exportdateien, {} Warnungen",
        crawled.crawled, files, warnings
    );
    // Nach `--no-store` gibt es die Datei nicht mehr; ihre Kennung bleibt für die Historie.
    let crawl_info = |outcome: Outcome| outcome.with_crawl(&crawled.path, Some(crawled.crawled));
    if failures > 0 {
        return Ok(crawl_info(Outcome::new(
            Exit::ExportFailed,
            format!("Crawl erfolgreich, aber {failures} Export(e) fehlgeschlagen ({base})"),
        )));
    }
    if plan.strict && warnings > 0 {
        return Ok(crawl_info(Outcome::new(
            Exit::ExportFailed,
            format!("Warnungen mit --strict als Fehler gewertet ({base})"),
        )));
    }
    Ok(crawl_info(Outcome::new(
        Exit::Ok,
        format!("Fertig: {base}"),
    )))
}

// ---------------------------------------------------------------------------------------------------
// Laden

/// `--load-crawl` ohne `--resume`: nichts wird gecrawlt; ein unvollständiger Crawl erhält eine Warnung.
fn open_only(path: &Path, rep: &Reporter) -> Result<Crawled, CliError> {
    let file = CrawlFile::open(path, OpenMode::ReadWrite)
        .map_err(|e| CliError::usage(format!("Crawl-Datei nicht ladbar: {e}")))?;
    let meta = file.meta().map_err(store_err)?;
    let state = file
        .effective_state(&SystemClock)
        .map_err(store_err)
        .unwrap_or(meta.state);
    if state != CrawlState::Completed {
        rep.warn(format!(
            "crawl incomplete – der Crawl hat den Zustand '{}'; die Exporte zeigen den gespeicherten Stand \
             (fortsetzen mit --resume)",
            state.as_str()
        ));
    }
    let crawled = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE state = 'done' AND is_internal = 1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0);
    Ok(Crawled {
        path: path.to_path_buf(),
        crawled: u64::try_from(crawled).unwrap_or(0),
    })
}

fn store_err(e: os_store::StoreError) -> CliError {
    CliError::internal(format!("Crawl-Datei: {e}"))
}

// ---------------------------------------------------------------------------------------------------
// Crawl

enum CrawlEnd {
    Done(Crawled),
    Interrupted { id: CrawlId },
}

fn build_fetcher(
    cfg: &CrawlConfig,
    secrets: &dyn SecretStore,
    clock: Arc<dyn Clock>,
) -> Result<Arc<HttpFetcher>, CliError> {
    let fetch_cfg = FetchConfig::from_crawl_config(cfg, secrets, clock)
        .map_err(|e| CliError::usage(e.to_string()))?;
    HttpFetcher::new(fetch_cfg)
        .map(Arc::new)
        .map_err(|e| CliError::usage(e.to_string()))
}

fn deps(id: Option<CrawlId>, hook: FinishedHook) -> EngineDeps {
    let ids: Arc<dyn IdSource> = match id {
        Some(id) => Arc::new(FixedId(id)),
        None => Arc::new(RandomIds),
    };
    EngineDeps {
        clock: Arc::new(SystemClock),
        ids,
        profile: StorageProfile::Balanced,
        secrets: Arc::new(KeyringSecretStore::new()),
        sink_factory: None,
        on_finished: Some(hook),
        owner_kind: "cli",
        renderer: None,
    }
}

fn api_secrets(plan: &Plan) -> Arc<dyn SecretStore> {
    plan.api_secrets
        .as_ref()
        .map(|s| s.0.clone())
        .unwrap_or_else(|| os_fetch::secret_store_from_env())
}
fn deps_with_apis(plan: &Plan, id: Option<CrawlId>, hook: FinishedHook) -> EngineDeps {
    let mut value = deps(id, hook);
    value.secrets = api_secrets(plan);
    value
}
fn preflight_apis(plan: &Plan, rep: &Reporter) -> Result<(), CliError> {
    let ctx = os_integrations::ApiCtx {
        cfg: plan.config.clone(),
        secrets: os_integrations::crawl_glue::environment_view_for(api_secrets(plan), &plan.config),
        network: Default::default(),
        allow_api_override: false,
    };
    for warning in os_integrations::crawl_glue::preflight(&ctx, plan.require_apis)
        .map_err(|e| CliError::usage(e.to_string()))?
    {
        rep.warn(warning);
    }
    Ok(())
}

/// Hook am Crawl-Ende (CRAWL-141): Die Crawl-Analyse läuft sofort danach, vor der Auflösung von `wait()`.
fn analysis_hook(rep: &Arc<Reporter>, signals: &Signals) -> FinishedHook {
    let rep = Arc::clone(rep);
    let cancel = signals.cancel_token();
    Arc::new(move |summary: &CrawlSummary| {
        rep.phase("analysis");
        match trigger::run_after_crawl(&summary.file, &cancel) {
            Ok(Some(outcome)) if !outcome.complete && !cancel.is_cancelled() => {
                rep.warn("Crawl-Analyse unvollständig – analyseabhängige Filter können leer sein");
            }
            Ok(_) => {}
            Err(e) => rep.warn(format!("Crawl-Analyse fehlgeschlagen: {e}")),
        }
    })
}

fn engine_error(e: EngineError, resuming: bool) -> CliError {
    match e {
        EngineError::Invalid(msg) if resuming && msg.contains("läuft noch") => CliError::internal(
            format!(
                "{msg}. Ein abgestürzter Crawl lässt sich erst mehr als 10 Sekunden nach dem letzten \
                 Heartbeat fortsetzen."
            ),
        ),
        EngineError::Invalid(msg) => CliError::usage(msg),
        EngineError::Join(msg) => CliError::internal(format!("interner Fehler: {msg}")),
        other => CliError::crawl(format!("Der Crawl konnte nicht durchgeführt werden: {other}")),
    }
}

async fn crawl_phase(
    plan: &Plan,
    rep: &Arc<Reporter>,
    signals: &Signals,
) -> Result<CrawlEnd, CliError> {
    plan.paths.create_all().map_err(|e| {
        CliError::crawl(format!(
            "Crawl-Datei nicht schreibbar – Datenordner nicht anlegbar: {e}"
        ))
    })?;
    let hook = analysis_hook(rep, signals);

    let (handle, path, id) = match &plan.source {
        Source::Load { path } => {
            // --resume
            crate::listing::validate_resume(path)?;
            let file = CrawlFile::open(path, OpenMode::ReadOnly)
                .or_else(|_| CrawlFile::open(path, OpenMode::ReadWrite))
                .map_err(|e| CliError::usage(format!("Crawl-Datei nicht ladbar: {e}")))?;
            let meta = file.meta().map_err(store_err)?;
            if matches!(meta.state, CrawlState::Failed | CrawlState::Completed) {
                let queued: i64 = file
                    .conn()
                    .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
                    .map_err(|e| CliError::crawl(e.to_string()))?;
                if meta.state == CrawlState::Failed || queued == 0 {
                    return Err(CliError::crawl("Der Crawl kann nicht fortgesetzt werden"));
                }
            }
            let stored = file.config().map_err(store_err)?;
            drop(file);
            let cfg = plan.resume_config.clone().unwrap_or(stored);
            let fetcher = build_fetcher(&cfg, &KeyringSecretStore::new(), Arc::new(SystemClock))?;
            warn_disk(path, rep);
            rep.phase("crawl");
            let handle = resume_crawl(
                fetcher,
                path.clone(),
                plan.resume_config.clone(),
                deps_with_apis(plan, None, hook),
            )
            .await
            .map_err(|e| engine_error(e, true))?;
            (handle, path.clone(), meta.crawl_id)
        }
        Source::Spider { .. } | Source::List { .. } => {
            let id = RandomIds.new_crawl_id(SystemClock.now());
            let path = registry::path_for(&plan.paths.crawls, &id);
            let source = match &plan.source {
                Source::Spider { start_url } => CrawlSource::Spider {
                    start_url: start_url.clone(),
                },
                Source::List { items, label } => CrawlSource::List {
                    items: items.clone(),
                    label: label.clone(),
                },
                Source::Load { .. } => unreachable!("oben behandelt"),
            };
            let fetcher = build_fetcher(
                &plan.config,
                &KeyringSecretStore::new(),
                Arc::new(SystemClock),
            )?;
            warn_disk(&path, rep);
            rep.phase("crawl");
            let request = CrawlRequest {
                config: plan.config.clone(),
                source,
                file: path.clone(),
                project: plan.project.clone(),
                name: plan.name.clone(),
            };
            let handle = start_crawl(fetcher, request, deps_with_apis(plan, Some(id), hook))
                .await
                .map_err(|e| engine_error(e, false))?;
            (handle, path, id)
        }
    };

    let _segment_observer = if plan.config.segments.list.is_empty() {
        None
    } else {
        Some(
            os_analyze::segments::observe(path.clone(), signals.cancel_token())
                .map_err(|error| CliError::internal(format!("Segmentbeobachter: {error}")))?,
        )
    };
    let json_errors = rep.json();
    let summary = monitor(handle, signals, rep, &path, json_errors)
        .await
        .map_err(|e| engine_error(e, plan.resume))?;

    match summary.state {
        CrawlState::Paused if signals.is_interrupted() => return Ok(CrawlEnd::Interrupted { id }),
        CrawlState::Paused => {
            return Err(CliError::crawl(format!(
                "Der Crawl wurde automatisch pausiert (Speicherplatz knapp) und lässt sich nach \
                 dem Freigeben von Platz mit `openspider --load-crawl {id} --resume` fortsetzen"
            )));
        }
        CrawlState::Failed => {
            return Err(CliError::crawl(
                "Der Crawl ist fehlgeschlagen und wurde nicht abgeschlossen",
            ));
        }
        _ => {}
    }
    if signals.is_interrupted() {
        // Signal während der letzten Schritte: Analyse abgebrochen, keine Exporte.
        return Ok(CrawlEnd::Interrupted { id });
    }
    if summary.end_reason == Some(os_model::EndReason::LimitReached) {
        rep.info(format!(
            "Hinweis: URL-Limit erreicht; weiter crawlen mit `openspider --load-crawl {id} --resume --max-urls <n>`"
        ));
    }
    Ok(CrawlEnd::Done(Crawled {
        path,
        crawled: summary.crawled,
    }))
}

fn warn_disk(path: &Path, rep: &Reporter) {
    if let Some(w) = disk_guard::start_warning(path) {
        rep.warn(w);
    }
}

fn is_terminal(p: &Progress) -> bool {
    p.state != CrawlState::Running
}

/// Überwacht den Crawl: Fortschrittsausgabe, erstes Signal → Pause. Wartet danach auf das Ende (inkl.
/// Crawl-Analyse im Abschluss-Hook).
async fn monitor(
    handle: CrawlHandle,
    signals: &Signals,
    rep: &Reporter,
    path: &Path,
    count_errors: bool,
) -> Result<CrawlSummary, EngineError> {
    let mut progress = handle.watch();
    let mut first = signals.first_signal();
    let period = Duration::from_secs(if rep.json() { 1 } else { 5 });
    let mut tick = interval_at(Instant::now() + period, period);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut error_reader: Option<CrawlFile> = None;
    let mut paused_by_signal = false;

    if signals.is_interrupted() {
        handle.pause();
        paused_by_signal = true;
    }
    if !is_terminal(&progress.borrow()) {
        loop {
            tokio::select! {
                changed = first.changed(), if !paused_by_signal => {
                    if changed.is_ok() && *first.borrow() {
                        handle.pause();
                        paused_by_signal = true;
                    }
                }
                _ = tick.tick() => {
                    let snapshot = handle.progress();
                    let errors = count_errors.then(|| error_count(&mut error_reader, path));
                    rep.progress("crawl", &snapshot, errors.flatten());
                }
                changed = progress.changed() => {
                    if changed.is_err() || is_terminal(&progress.borrow()) {
                        break;
                    }
                }
            }
        }
    }
    drop(error_reader);
    handle.wait().await
}

/// Anzahl der URLs mit Fehlerstatus (0 oder ≥ 400) für `--json-progress`.
fn error_count(reader: &mut Option<CrawlFile>, path: &Path) -> Option<u64> {
    if reader.is_none() {
        *reader = CrawlFile::open(path, OpenMode::ReadOnly).ok();
    }
    let file = reader.as_ref()?;
    let n: i64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE status_code = 0 OR status_code >= 400",
            [],
            |r| r.get(0),
        )
        .ok()?;
    u64::try_from(n).ok()
}

/// Meldet, wenn die Start-URL keine Antwort lieferte (Statuscode 0).
fn start_url_problem(path: &Path, start_url: &str) -> Option<String> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).ok()?;
    let row = queries::url_by_address(file.conn(), start_url).ok()??;
    (row.status_code == Some(0)).then(|| row.status.unwrap_or_else(|| "keine Antwort".to_string()))
}

// ---------------------------------------------------------------------------------------------------
// Exporte

enum ExportEnd {
    Done { failures: u32, files: usize },
    Interrupted,
}

fn describe(item: &Resolved) -> String {
    match item {
        Resolved::Tab { tab, filter } => format!("{tab}:{filter}"),
        Resolved::Bulk(n) | Resolved::Report(n) => n.clone(),
        Resolved::Issues => "Issues:All Issues".to_string(),
    }
}

fn report_file(rep: &Reporter, info: &ExportFileInfo) {
    rep.note_export(&info.path);
    rep.info(format!(
        "Export: {} ({} Zeilen)",
        info.path.display(),
        info.rows
    ));
    for w in &info.warnings {
        rep.warn(w);
    }
}

fn export_phase(plan: &Plan, rep: &Reporter, signals: &Signals, crawled: &Crawled) -> ExportEnd {
    let Some(out) = plan.output.as_deref() else {
        return ExportEnd::Done {
            failures: 0,
            files: 0,
        };
    };
    rep.phase("export");
    let file = match CrawlFile::open(&crawled.path, OpenMode::ReadWrite) {
        Ok(f) => f,
        Err(e) => {
            rep.error(format!("Crawl-Datei für die Exporte nicht lesbar: {e}"));
            return ExportEnd::Done {
                failures: 1,
                files: 0,
            };
        }
    };
    let cfg = match file.config() {
        Ok(c) => c,
        Err(e) => {
            rep.error(format!("Konfiguration der Crawl-Datei nicht lesbar: {e}"));
            return ExportEnd::Done {
                failures: 1,
                files: 0,
            };
        }
    };

    let mut failures = 0u32;
    let mut files = 0usize;

    if !plan.exports.is_empty() {
        let pending = !matches!(persist::read_run_state(&file), Ok(RunState::Completed));
        if pending {
            rep.info(
                "Hinweis: Die Crawl-Analyse ist nicht aktuell – sie wird vor den Exporten ausgeführt, \
                 damit analyseabhängige Filter nicht leer bleiben",
            );
        }
        rep.phase("analysis");
        match ensure_analysis(&file, &cfg, &signals.cancel_token()) {
            Ok(outcome) if !outcome.complete && !signals.is_interrupted() => {
                rep.warn("Crawl-Analyse unvollständig – analyseabhängige Filter können leer sein");
            }
            Ok(_) => {}
            Err(e) => rep.warn(format!("Crawl-Analyse fehlgeschlagen: {e}")),
        }
        rep.phase("export");
    }

    // CRAWL-134: loading without --resume has no observer; analysis can change membership.
    if let Err(error) = os_analyze::segments::refresh_if_stale(&file, &cfg, &signals.cancel_token())
    {
        rep.error(format!("Segmentzuordnung fehlgeschlagen: {error}"));
        return ExportEnd::Done {
            failures: 1,
            files: 0,
        };
    }

    // Initialize configured segments once; subsequent exports only consume the saved assignment.
    if !cfg.segments.list.is_empty() && matches!(file.get_meta("segments.definitions"), Ok(None)) {
        if let Err(error) = os_analyze::segments::reassign(&file, &cfg, &signals.cancel_token()) {
            rep.error(format!("Segmentzuordnung fehlgeschlagen: {error}"));
            return ExportEnd::Done {
                failures: 1,
                files: 0,
            };
        }
    }

    if let (Some(batch), None) = (plan.batch, plan.segment.as_deref()) {
        if !plan.exports.is_empty() {
            match run_batch(&file, &cfg, plan, batch, out, signals, rep) {
                Ok(n) => files += n,
                Err(BatchEnd::Interrupted) => return ExportEnd::Interrupted,
                Err(BatchEnd::Failed(message)) => {
                    failures += 1;
                    rep.error(format!("Exporte fehlgeschlagen: {message}"));
                }
            }
        }
    }
    let one_by_one: &[Resolved] = if plan.batch.is_some() && plan.segment.is_none() {
        &[]
    } else {
        &plan.exports
    };
    for item in one_by_one {
        if signals.is_interrupted() {
            return ExportEnd::Interrupted;
        }
        if let Some(segment) = plan.segment.as_deref() {
            match run_segment_export(&file, &cfg, item, out, plan.format, segment, rep) {
                Ok(n) => files += n,
                Err(message) => {
                    failures += 1;
                    rep.error(format!(
                        "Export '{}' fehlgeschlagen: {message}",
                        describe(item)
                    ));
                }
            }
            continue;
        }
        match run_export(&file, &cfg, item, out, plan.format, rep) {
            Ok(n) => files += n,
            Err(message) => {
                failures += 1;
                rep.error(format!(
                    "Export '{}' fehlgeschlagen: {message}",
                    describe(item)
                ));
            }
        }
    }

    if plan.save_crawl {
        if signals.is_interrupted() {
            return ExportEnd::Interrupted;
        }
        let dest = out.join("crawl.ospider");
        if dest.exists() {
            let _ = std::fs::remove_file(&dest);
        }
        match file.save_as(&dest) {
            Ok(()) => {
                files += 1;
                rep.info(format!("Crawl gespeichert: {}", dest.display()));
            }
            Err(e) => {
                failures += 1;
                rep.error(format!("Crawl speichern fehlgeschlagen: {e}"));
            }
        }
    }
    ExportEnd::Done { failures, files }
}

fn run_export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    item: &Resolved,
    out: &Path,
    format: Format,
    rep: &Reporter,
) -> Result<usize, String> {
    match item {
        Resolved::Tab { tab, filter } => {
            let query = ViewQuery::new(tab.clone(), filter.clone());
            let info = export_tab(file, cfg, &query, out, format).map_err(|e| e.to_string())?;
            report_file(rep, &info);
            Ok(1)
        }
        Resolved::Bulk(name) => {
            let infos = export_bulk(file, cfg, name, out, format).map_err(|e| e.to_string())?;
            infos.iter().for_each(|i| report_file(rep, i));
            Ok(infos.len())
        }
        Resolved::Issues => {
            let infos = bulk_issues::export(file, cfg, out, format).map_err(|e| e.to_string())?;
            infos.iter().for_each(|i| report_file(rep, i));
            Ok(infos.len())
        }
        Resolved::Report(name) => {
            let result = save_report(file, cfg, name, out, format).map_err(|e| e.to_string())?;
            result.files.iter().for_each(|i| report_file(rep, i));
            for w in &result.warnings {
                rep.warn(w);
            }
            Ok(result.files.len())
        }
    }
}

enum BatchEnd {
    Interrupted,
    Failed(String),
}

/// Geplante Aufgaben: alle Tabellen in einem Stapel gegen denselben Lesestand (OUT-007, OUT-012).
fn run_batch(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    plan: &Plan,
    batch: crate::plan::BatchSettings,
    out: &Path,
    signals: &Signals,
    rep: &Reporter,
) -> Result<usize, BatchEnd> {
    let selections: Vec<os_export::types::ExportSelection> = plan
        .exports
        .iter()
        .map(|item| {
            let (kind, name) = match item {
                Resolved::Tab { tab, filter } => ("tab", format!("{tab}:{filter}")),
                Resolved::Bulk(name) => ("bulk", name.clone()),
                Resolved::Issues => ("bulk", os_export::catalog::bulk_issues::BULK_NAME.into()),
                Resolved::Report(name) => ("report", name.clone()),
            };
            os_export::types::ExportSelection {
                kind: kind.into(),
                name,
            }
        })
        .collect();
    let token = signals.cancel_token();
    let crawl_name = file
        .meta()
        .map(|m| m.name)
        .ok()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "crawl".to_string());
    let mut ctx = os_export::types::ExportCtx {
        cancel: &token,
        progress: &|_| {},
        skip_empty: batch.skip_empty,
        workbook: None,
        skipped: Vec::new(),
    };
    let options = os_export::pipeline::BatchOptions {
        format: plan.format,
        directory: out,
        crawl_name: &crawl_name,
        single_workbook: batch.single_workbook,
    };
    match os_export::pipeline::export_batch(file, cfg, &selections, &options, &mut ctx) {
        Ok(result) => {
            for info in &result.files {
                report_file(rep, info);
            }
            for warning in &result.warnings {
                rep.warn(warning);
            }
            for name in &result.skipped {
                rep.warn(format!("Export übersprungen: {name}"));
            }
            Ok(result.files.len())
        }
        Err(_) if signals.is_interrupted() => Err(BatchEnd::Interrupted),
        Err(e) => Err(BatchEnd::Failed(e.to_string())),
    }
}

/// `--no-store`: löscht die Crawl-Datei samt `-wal`/`-shm`.
fn discard(path: &Path, rep: &Reporter) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let candidate = PathBuf::from(name);
        if candidate.exists() {
            if let Err(e) = std::fs::remove_file(&candidate) {
                rep.warn(format!(
                    "Crawl-Datei {} nicht löschbar: {e}",
                    candidate.display()
                ));
            }
        }
    }
}

/// Segment-aware companion to the unfiltered export dispatcher (OUT-011).
fn run_segment_export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    item: &Resolved,
    out: &Path,
    format: Format,
    segment: &str,
    rep: &Reporter,
) -> Result<usize, String> {
    let (kind, name) = match item {
        Resolved::Tab { tab, filter } => ("tab", format!("{tab}:{filter}")),
        Resolved::Bulk(name) => ("bulk", name.clone()),
        Resolved::Issues => ("bulk", os_export::catalog::bulk_issues::BULK_NAME.into()),
        Resolved::Report(name) => ("report", name.clone()),
    };
    let selection = os_export::types::ExportSelection {
        kind: kind.into(),
        name,
    };
    let files = os_export::segment::export_all(file, cfg, &selection, Some(segment), out, format)
        .map_err(|e| e.to_string())?;
    files.iter().for_each(|info| report_file(rep, info));
    Ok(files.len())
}
