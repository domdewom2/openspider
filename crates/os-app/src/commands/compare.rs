//! compare.* (OUT-140 ff., M2-C01): eine Vergleichssitzung je Fenster.
//!
//! `compare.open` öffnet beide Crawls (Kennung aus der Crawl-Verwaltung oder Pfad einer Crawl-Datei) und ordnet sie nach
//! Startzeitpunkt (der spätere ist Current, OUT-140); ältere Schemaversionen werden als Kopie im Cache-Ordner migriert.
//! `compare.swap` vertauscht die Rollen (OUT-141), `compare.testMapping` ist der Test-Reiter des URL-Mappings (OUT-146),
//! `compare.close` beendet die Sitzung und löscht migrierte Kopien.
//!
//! Die Vergleichssitzung hält zwei eigene, nur lesende Verbindungen (zwei Dateien gleichzeitig, das gibt der Lese-Pool
//! je Aufruf nicht her); jeder Zugriff läuft in `spawn_blocking`, nie auf einem Tokio-Worker (2.3 Regel 1). Die
//! Berechnung prüft das Token der Operation: der Ressourcenwächter (`os_engine::memory::watch_operation`) pausiert sie
//! bei Speicherknappheit, statt sie abzubrechen (CRAWL-129, K2-56).
//!
//! Für Overview, Issues, Views und Exporte im Vergleichsmodus stellen [`with_active`] und [`active_version`] die
//! Sitzung des Fensters bereit; die `compare`-Parameter von `catalog.*`, `sidebar.*` und `view.*` gehören deren
//! Besitzern (Schnittstellenbedarf im Bericht `docs/plan/M2-reports/c01.md`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::future::BoxFuture;
use os_analyze::compare::{CompareError, CompareNote, Comparison, CrawlSide, Side, UrlMapping};
use os_model::{ChangeElement, CrawlState, MappingRule};

use crate::dto::*;
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{internal, parse_args, store_error, to_json};
use crate::runtime::{App, CallCtx, WindowId};

/// Eine geöffnete Vergleichssitzung.
pub struct ActiveComparison {
    /// Der Vergleich.
    pub comparison: Comparison,
    /// Aktive Elemente der Change Detection (Kennungen laut TECH-300, M2-C02).
    pub elements: Vec<String>,
    /// Schwelle für Inhaltsänderungen in Prozent.
    pub content_threshold_pct: u8,
}

#[derive(Default)]
struct CompareState {
    sessions: Mutex<HashMap<WindowId, Arc<Mutex<ActiveComparison>>>>,
}

fn state(app: &App) -> Arc<CompareState> {
    app.ext::<CompareState>()
}

fn session(app: &App, window: &WindowId) -> Option<Arc<Mutex<ActiveComparison>>> {
    state(app)
        .sessions
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(window)
        .cloned()
}

fn no_comparison() -> CmdError {
    CmdError::new(ErrorCode::NotFound, "err-no-comparison")
}

/// Fehler des Vergleichs als Command-Fehler.
pub fn compare_error(e: CompareError) -> CmdError {
    match e {
        CompareError::Store(e) => store_error(&e),
        CompareError::Sql(e) => store_error(&os_store::StoreError::from(e)),
        CompareError::Cancelled => CmdError::new(ErrorCode::Cancelled, "err-cancelled"),
        CompareError::Mapping(e) => CmdError::invalid_input(
            "err-mapping-pattern",
            format!("mapping[{}].pattern", e.rule),
        )
        .with_arg("rule", e.rule)
        .with_arg("position", e.position)
        .with_detail(e.message),
        CompareError::View(e) => super::catalog::view_error(e),
        CompareError::Invalid(detail) => {
            CmdError::invalid_input("err-compare-input", "compare").with_detail(detail)
        }
    }
}

/// Führt `f` mit der Vergleichssitzung des Fensters in `spawn_blocking` aus (für Overview, Issues, Views, Exporte im
/// Vergleichsmodus). Ohne Sitzung: `NotFound`.
pub async fn with_active<T: Send + 'static>(
    app: &App,
    window: &WindowId,
    f: impl FnOnce(&mut ActiveComparison) -> Result<T, CmdError> + Send + 'static,
) -> Result<T, CmdError> {
    let active = session(app, window).ok_or_else(no_comparison)?;
    tokio::task::spawn_blocking(move || {
        let mut guard = active.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    })
    .await
    .map_err(|e| internal("compare", e.to_string()))?
}

/// Version der Klassifikation der Sitzung des Fensters (`None` ohne Vergleich).
pub fn active_version(app: &App, window: &WindowId) -> Option<u64> {
    session(app, window).map(|s| {
        s.lock()
            .unwrap_or_else(|p| p.into_inner())
            .comparison
            .version()
    })
}

/// Löst eine Auswahl auf: vorhandene Datei oder Kennung aus der Crawl-Verwaltung.
async fn resolve(app: &App, selection: &str, field: &'static str) -> Result<PathBuf, CmdError> {
    let path = Path::new(selection);
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let id: os_model::CrawlId = selection
        .parse()
        .map_err(|_| CmdError::invalid_input("err-crawl-id", field))?;
    let listing = super::crawls::admin(app, |m| m.list().map_err(|e| store_error(&e))).await?;
    listing
        .crawls
        .into_iter()
        .find(|l| l.meta.crawl_id == id)
        .map(|l| l.path)
        .ok_or_else(|| {
            CmdError::new(ErrorCode::NotFound, "err-crawl-not-found")
                .with_arg("id", selection)
                .with_arg("field", field)
        })
}

/// Angaben einer Seite für die Zusammenfassung (ohne Dateizugriff).
struct SideInfo {
    meta: os_store::CrawlMeta,
    source: PathBuf,
    urls: u64,
}

impl SideInfo {
    fn of(c: &Comparison, side: Side) -> SideInfo {
        let p = c.pairing();
        SideInfo {
            meta: c.side(side).meta.clone(),
            source: c.side(side).source.clone(),
            urls: match side {
                Side::Current => p.current.len(),
                Side::Previous => p.previous.len(),
            },
        }
    }

    fn summary(&self, now: os_model::Timestamp) -> CrawlSummaryDto {
        let m = &self.meta;
        let state = os_store::registry::effective_state(m, now);
        CrawlSummaryDto {
            crawl_id: m.crawl_id.to_string(),
            file: self.source.to_string_lossy().into_owned(),
            name: m.name.clone(),
            phase: crate::session::feed::phase(state),
            mode: crate::session::feed::mode(m.mode),
            started_at: m.started_at.unwrap_or_default().to_db(),
            finished_at: m.ended_at.map(|t| t.to_db()),
            // Gecrawlte URLs (Existenz im Sinne von OUT-142).
            urls: self.urls,
            file_bytes: std::fs::metadata(&self.source).map_or(0, |d| d.len()),
            schema_version: m.schema_version,
            owner: CrawlOwner {
                kind: OwnerKind::App,
                pid: None,
                live: state == CrawlState::Running,
                controllable: false,
            },
        }
    }
}

fn dto(active: &ActiveComparison, now: os_model::Timestamp) -> CompareSession {
    let c = &active.comparison;
    CompareSession {
        current: SideInfo::of(c, Side::Current).summary(now),
        previous: SideInfo::of(c, Side::Previous).summary(now),
        mapping: c
            .rules()
            .iter()
            .map(|r| MappingRuleDto {
                pattern: r.pattern.clone(),
                replacement: r.replacement.clone(),
            })
            .collect(),
        elements: active.elements.clone(),
        content_threshold_pct: active.content_threshold_pct,
        version: c.version(),
    }
}

fn notes_log(notes: &[CompareNote]) {
    for n in notes {
        match n {
            CompareNote::DifferentModes { current, previous } => tracing::info!(
                current = current.as_str(),
                previous = previous.as_str(),
                "Vergleich: Crawls mit unterschiedlichem Modus"
            ),
            CompareNote::Migrated { side, copy } => tracing::info!(
                side = side.as_str(),
                copy = %copy.display(),
                "Vergleich: Crawl-Datei als Kopie migriert"
            ),
            CompareNote::Collision(c) => tracing::warn!(
                current = %c.current_url,
                winner = %c.winner,
                losers = ?c.losers,
                "Vergleich: Kollision im URL-Mapping"
            ),
            CompareNote::MappingWarning(w) => {
                tracing::warn!(rule = w.rule, pattern = %w.pattern, "Vergleich: {}", w.message)
            }
        }
    }
}

fn rules(mapping: &[MappingRuleDto]) -> Vec<MappingRule> {
    mapping
        .iter()
        .map(|r| MappingRule {
            pattern: r.pattern.clone(),
            replacement: r.replacement.clone(),
        })
        .collect()
}

fn remove_copies(copies: &[PathBuf]) {
    for copy in copies {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = copy.as_os_str().to_owned();
            p.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(p));
        }
    }
}

/// Entfernt die Sitzung des Fensters; Verbindungen schließen und Kopien löschen außerhalb des Tokio-Workers.
async fn close_window(app: &App, window: &WindowId) -> Result<(), CmdError> {
    let old = state(app)
        .sessions
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(window);
    if let Some(old) = old {
        tokio::task::spawn_blocking(move || {
            // Hält ein laufender Aufruf die Sitzung noch, schließt er sie; die Kopien bleiben dann im Cache.
            if let Ok(m) = Arc::try_unwrap(old) {
                let active = m.into_inner().unwrap_or_else(|p| p.into_inner());
                let copies: Vec<PathBuf> = [Side::Current, Side::Previous]
                    .into_iter()
                    .filter_map(|s| active.comparison.side(s).migrated.clone())
                    .collect();
                drop(active);
                remove_copies(&copies);
            }
        })
        .await
        .map_err(|e| internal("compare.close", e.to_string()))?;
    }
    Ok(())
}

/// Wartet vor dem Start, solange der Ressourcenwächter Stufe 2 meldet (CRAWL-129): der Vergleich beginnt pausiert und
/// läuft weiter, sobald der Speicher wieder reicht; während der Berechnung pausiert `watch_operation` das Token.
async fn wait_for_memory(cancel: &os_model::CancelToken) -> Result<(), CmdError> {
    let mut warned = false;
    while os_engine::memory::sample().stage() == os_model::MemoryStage::Paused {
        if cancel.is_cancelled() {
            return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
        }
        if !warned {
            tracing::warn!("Wenig Arbeitsspeicher – Vergleichsberechnung pausiert");
            warned = true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

fn open<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CompareOpenArgs = parse_args(args)?;
        if args.content_threshold_pct.is_some_and(|p| p > 100) {
            return Err(CmdError::invalid_input(
                "err-compare-threshold",
                "contentThresholdPct",
            ));
        }
        for e in &args.elements {
            e.parse::<ChangeElement>().map_err(|_| {
                CmdError::invalid_input("err-compare-element", "elements")
                    .with_arg("element", e.as_str())
            })?;
        }
        let mapping = rules(&args.mapping);
        UrlMapping::compile(&mapping).map_err(|e| compare_error(e.into()))?;
        let current = resolve(app, &args.current, "current").await?;
        let previous = resolve(app, &args.previous, "previous").await?;
        let work = app.config().paths.cache.join("compare");
        std::fs::create_dir_all(&work).map_err(|e| {
            store_error(&os_store::StoreError::Io {
                path: work.clone(),
                source: e,
            })
        })?;
        let cancel = ctx.cancel.clone();
        wait_for_memory(&cancel).await?;
        let _guard = os_engine::memory::watch_operation(cancel.clone());
        let opened = tokio::task::spawn_blocking(move || -> Result<ActiveComparison, CmdError> {
            let a = CrawlSide::open(&current, &work).map_err(compare_error)?;
            let b = CrawlSide::open(&previous, &work).map_err(compare_error)?;
            let comparison = Comparison::open(a, b, &mapping, &cancel).map_err(compare_error)?;
            let defaults = &comparison.current().config.compare.change_detection;
            let elements = if args.elements.is_empty() {
                defaults
                    .elements
                    .iter()
                    .map(|e| e.as_str().to_string())
                    .collect()
            } else {
                args.elements.clone()
            };
            let content_threshold_pct = args
                .content_threshold_pct
                .unwrap_or(defaults.content_threshold_pct);
            notes_log(&comparison.notes());
            Ok(ActiveComparison {
                comparison,
                elements,
                content_threshold_pct,
            })
        })
        .await
        .map_err(|e| internal("compare.open", e.to_string()))??;
        let now = app.config().clock.now();
        let result = dto(&opened, now);
        close_window(app, &ctx.window).await?;
        state(app)
            .sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(ctx.window.clone(), Arc::new(Mutex::new(opened)));
        to_json(&result)
    })
}

fn swap<'a>(app: &'a App, ctx: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let now = app.config().clock.now();
        let session = with_active(app, &ctx.window, move |active| {
            active.comparison.swap();
            Ok(dto(active, now))
        })
        .await?;
        to_json(&session)
    })
}

fn test_mapping<'a>(_: &'a App, _: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: TestMappingArgs = parse_args(args)?;
        let mut output = args.input.clone();
        let mut steps = Vec::with_capacity(args.rules.len());
        for (index, r) in args.rules.iter().enumerate() {
            let rule = MappingRule {
                pattern: r.pattern.clone(),
                replacement: r.replacement.clone(),
            };
            match UrlMapping::compile(std::slice::from_ref(&rule)) {
                Ok(one) => {
                    let trace = one.trace(&output);
                    let step = trace.steps.into_iter().next().expect("eine Regel");
                    steps.push(RuleStep {
                        index: index as u32,
                        matched: step.matched,
                        before: step.before,
                        after: step.after.clone(),
                        error: step.error,
                    });
                    output = step.after;
                }
                Err(e) => steps.push(RuleStep {
                    index: index as u32,
                    matched: false,
                    before: output.clone(),
                    after: output.clone(),
                    error: Some(match e.position {
                        Some(p) => {
                            format!("invalid regular expression at position {p}: {}", e.message)
                        }
                        None => format!("invalid regular expression: {}", e.message),
                    }),
                }),
            }
        }
        to_json(&RuleTrace {
            output,
            included: None,
            steps,
        })
    })
}

fn close<'a>(app: &'a App, ctx: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        close_window(app, &ctx.window).await?;
        Ok(serde_json::Value::Null)
    })
}

/// Trägt die Commands dieser Domäne in die Registry ein (Namen und Typen unverändert laut Katalog).
pub fn register(r: &mut Registry) {
    for (name, args, ret, handler) in [
        (
            "compare.open",
            "CompareOpenArgs",
            "CompareSession",
            open as crate::registry::Handler,
        ),
        ("compare.swap", "void", "CompareSession", swap),
        (
            "compare.testMapping",
            "TestMappingArgs",
            "RuleTrace",
            test_mapping,
        ),
        ("compare.close", "void", "void", close),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            handler,
        );
    }
}
