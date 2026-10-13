//! Runner der Teilschritte (CRAWL-141): Schalter, Datengrundlage, Parallelität, Abbruch, Persistenz.
//!
//! Ablauf von [`run`]:
//! 1. Abgewählte Teilschritte (Schalter in `analysis.*` aus) → `Skipped`; Teilschritte ohne Datengrundlage
//!    (Bilder ohne JS-Rendering, Analytics/Search Console/Linkmetriken ohne Anbindung) → `NotApplicable`.
//! 2. Die übrigen Teilschritte sind voneinander unabhängig und laufen parallel, jeder auf einer eigenen
//!    Verbindung zur Crawl-Datei (SQLite erlaubt einen Schreiber zugleich; kurze Sperren löst `busy_timeout`).
//! 3. Jeder Schritt wird vor dem Start auf „ausstehend“ gesetzt und erst nach seinem Ende als `Done`
//!    gespeichert. Ein Abbruch lässt fertige Schritte gelten, abgebrochene bleiben ausstehend – es gibt nie
//!    Teilergebnisse als Endstand.
//! 4. Ergebnis, `analysis_state` und `analysis_at` stehen danach in der Crawl-Datei (`persist`).
//!
//! Ein Schritt meldet `Ok(StepState::Pending)`, wenn er wegen des Abbruchs nicht fertig wurde.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use os_model::{AnalysisStep, Clock, CrawlConfig, RenderingMode, StepState, SystemClock};
use os_store::{CrawlFile, OpenMode};

use super::persist::{self, RunState};
use super::{AnalysisOutcome, CancelToken};
use crate::error::AnalyzeError;
use crate::state::step_config_hash;

/// Signatur eines Teilschritts (`steps::<name>::run`).
pub type StepFn = fn(&CrawlFile, &CrawlConfig, &CancelToken) -> Result<StepState, AnalyzeError>;

/// Standardzuordnung Teilschritt → Funktion. Schritte ohne eigene Umsetzung sind in M1 `NotApplicable`.
pub fn default_registry(step: AnalysisStep) -> StepFn {
    match step {
        AnalysisStep::Canonicals => super::steps::canonicals::run,
        AnalysisStep::Sitemaps => super::steps::sitemaps::run,
        _ => not_applicable,
    }
}

fn not_applicable(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}

/// Optionen eines Laufs; [`RunOptions::default`] entspricht [`run`].
#[derive(Clone, Copy)]
pub struct RunOptions<'a> {
    /// Uhr für `finished_at` und `analysis_at`.
    pub clock: &'a dyn Clock,
    /// Nur diese Teilschritte ausführen (`None` = alle).
    pub only: Option<&'a [AnalysisStep]>,
    /// Zuordnung Teilschritt → Funktion (Tests setzen eigene Schritte ein).
    pub registry: fn(AnalysisStep) -> StepFn,
    /// Höchstzahl gleichzeitig laufender Teilschritte (`0` = Anzahl der CPU-Kerne).
    pub max_parallel: usize,
}

static SYSTEM_CLOCK: SystemClock = SystemClock;

impl Default for RunOptions<'_> {
    fn default() -> Self {
        RunOptions {
            clock: &SYSTEM_CLOCK,
            only: None,
            registry: default_registry,
            max_parallel: 0,
        }
    }
}

/// Führt die Teilschritte aus und meldet den Fortschritt je Schritt (0.0 beim Start, 1.0 am Ende).
pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    progress: &dyn Fn(AnalysisStep, f32),
) -> Result<AnalysisOutcome, AnalyzeError> {
    run_with(file, cfg, cancel, progress, &RunOptions::default())
}

/// Führt nur die genannten Teilschritte aus (z. B. die ausstehenden oder veralteten, `ensure_analysis`).
pub fn run_only(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    progress: &dyn Fn(AnalysisStep, f32),
    only: &[AnalysisStep],
) -> Result<AnalysisOutcome, AnalyzeError> {
    let opts = RunOptions {
        only: Some(only),
        ..RunOptions::default()
    };
    run_with(file, cfg, cancel, progress, &opts)
}

/// Vorab entscheidbarer Zustand eines Teilschritts: `Some` = nicht ausführen.
pub fn predetermined_state(step: AnalysisStep, cfg: &CrawlConfig) -> Option<StepState> {
    let js = cfg.rendering.mode == RenderingMode::JavaScript;
    // Bilder sind ohne JS-Rendering nicht wählbar (CRAWL-141): nicht anwendbar, auch bei gesetztem Schalter.
    if step == AnalysisStep::Images && !js {
        return Some(StepState::NotApplicable);
    }
    if !cfg.analysis.step_enabled(step) {
        return Some(StepState::Skipped);
    }
    match step {
        // Keine Anbindung in M1: keine Datengrundlage.
        AnalysisStep::Analytics | AnalysisStep::SearchConsole | AnalysisStep::LinkMetrics => {
            Some(StepState::NotApplicable)
        }
        _ => None,
    }
}

/// Nachricht eines Arbeiters an den Hauptthread.
enum Event {
    Started(AnalysisStep),
    Finished(AnalysisStep, Result<StepState, AnalyzeError>),
}

/// Wie [`run`], mit Optionen.
pub fn run_with(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    progress: &dyn Fn(AnalysisStep, f32),
    opts: &RunOptions<'_>,
) -> Result<AnalysisOutcome, AnalyzeError> {
    let in_scope: Vec<AnalysisStep> = AnalysisStep::ALL
        .iter()
        .copied()
        .filter(|s| opts.only.is_none_or(|only| only.contains(s)))
        .collect();

    persist::write_run_state(file, RunState::Running, None)?;

    let mut results: Vec<(AnalysisStep, StepState)> = Vec::new();
    let mut runnable: Vec<AnalysisStep> = Vec::new();
    for &step in &in_scope {
        match predetermined_state(step, cfg) {
            Some(state) => {
                persist::write_step(
                    file,
                    step,
                    state,
                    Some(&opts.clock.now()),
                    Some(&step_config_hash(step, cfg)),
                )?;
                persist::clear_stale(file, step)?;
                results.push((step, state));
                progress(step, 1.0);
            }
            None => {
                persist::begin_step(file, step)?;
                results.push((step, StepState::Pending));
                runnable.push(step);
            }
        }
    }

    let mut first_error: Option<AnalyzeError> = None;
    if !runnable.is_empty() && !cancel.is_cancelled() {
        let workers = match opts.max_parallel {
            0 => std::thread::available_parallelism().map_or(1, usize::from),
            n => n,
        }
        .clamp(1, runnable.len());
        let next = AtomicUsize::new(0);
        let (tx, rx) = mpsc::channel::<Event>();
        let path = file.path().to_path_buf();

        std::thread::scope(|scope| {
            for _ in 0..workers {
                let tx = tx.clone();
                let (next, runnable, path) = (&next, &runnable, &path);
                scope.spawn(move || {
                    // Eigene Verbindung je Arbeiter; scheitert das Öffnen, scheitern seine Schritte.
                    let conn =
                        CrawlFile::open(path, OpenMode::ReadWrite).map_err(|e| e.to_string());
                    loop {
                        if cancel.is_cancelled() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(&step) = runnable.get(i) else { break };
                        let _ = tx.send(Event::Started(step));
                        let outcome = match &conn {
                            Ok(f) => (opts.registry)(step)(f, cfg, cancel),
                            Err(msg) => Err(open_error(msg)),
                        };
                        let _ = tx.send(Event::Finished(step, outcome));
                    }
                });
            }
            drop(tx);
            // Hauptthread: Fortschritt melden und fertige Schritte sofort speichern.
            for event in rx {
                match event {
                    Event::Started(step) => progress(step, 0.0),
                    Event::Finished(step, outcome) => {
                        match settle(file, cfg, opts.clock, step, outcome) {
                            Ok(state) => {
                                set_result(&mut results, step, state);
                                if state != StepState::Pending {
                                    progress(step, 1.0);
                                }
                            }
                            Err(e) => {
                                first_error.get_or_insert(e);
                            }
                        }
                    }
                }
            }
        });
    }

    let complete = results.iter().all(|(_, s)| *s != StepState::Pending);
    let final_state = overall_state(file)?;
    persist::write_run_state(file, final_state, Some(&opts.clock.now()))?;

    if let Some(e) = first_error {
        return Err(e);
    }
    Ok(AnalysisOutcome {
        steps: results,
        complete,
    })
}

/// Fehler, wenn ein Arbeiter die Crawl-Datei nicht öffnen konnte.
fn open_error(msg: &str) -> AnalyzeError {
    AnalyzeError::Sql(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
        Some(format!(
            "Crawl-Datei für die Analyse nicht zu öffnen: {msg}"
        )),
    ))
}

fn set_result(results: &mut [(AnalysisStep, StepState)], step: AnalysisStep, state: StepState) {
    if let Some(slot) = results.iter_mut().find(|(s, _)| *s == step) {
        slot.1 = state;
    }
}

/// Speichert das Ergebnis eines Schritts und liefert dessen endgültigen Zustand.
fn settle(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    clock: &dyn Clock,
    step: AnalysisStep,
    outcome: Result<StepState, AnalyzeError>,
) -> Result<StepState, AnalyzeError> {
    let state = outcome?;
    match state {
        // Nicht fertig (abgebrochen): bleibt ausstehend.
        StepState::Pending => Ok(StepState::Pending),
        done => {
            persist::write_step(
                file,
                step,
                done,
                Some(&clock.now()),
                Some(&step_config_hash(step, cfg)),
            )?;
            if done != StepState::Pending {
                persist::clear_stale(file, step)?;
            }
            Ok(done)
        }
    }
}

/// `completed`, wenn jeder Teilschritt eine Entscheidung hat, sonst `incomplete`.
fn overall_state(file: &CrawlFile) -> Result<RunState, AnalyzeError> {
    let rows = persist::read_steps(file)?;
    let decided = AnalysisStep::ALL
        .iter()
        .all(|s| rows.get(s).is_some_and(|r| r.state != StepState::Pending));
    Ok(if decided {
        RunState::Completed
    } else {
        RunState::Incomplete
    })
}
