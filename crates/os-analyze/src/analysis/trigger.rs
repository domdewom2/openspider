//! Auslösung der Crawl-Analyse durch den Aufrufer der Engine (CRAWL-141).
//!
//! `os-engine` kennt `os-analyze` nicht; sie ruft nur den Hook `EngineDeps.on_finished` – nach regulärem Ende,
//! Limit oder Stopp, nach dem abschließenden Checkpoint und vor der Auflösung von `wait()`, nie bei Pause oder
//! Absturz. `os-cli` und die App setzen dort [`run_after_crawl`] ein.
//!
//! * [`run_after_crawl`] liest die Konfiguration aus der Crawl-Datei und startet die Analyse, wenn
//!   `analysis.auto_at_end` an ist (OS-Default: an); sonst läuft nichts, und die analyseabhängigen Filter
//!   zeigen „Crawl-Analyse erforderlich“.
//! * [`run_manual`] ist der manuelle Start („Crawl-Analyse › Starten“). Während einer Pause (und nach einer
//!   Unterbrechung) sind die Ergebnisse möglicherweise unvollständig: Der Analysezustand steht danach auf
//!   „unvollständig“, nicht auf „abgeschlossen“.
//! * [`ensure_analysis`] führt ausstehende oder veraltete Teilschritte aus (CLI vor analyseabhängigen Exporten,
//!   „Exporte sollen nie stillschweigend leer sein“); ist alles aktuell, passiert nichts.

use std::path::Path;

use os_model::{AnalysisStep, CrawlConfig, CrawlState, StepState};
use os_store::{CrawlFile, OpenMode};

use super::persist::{self, RunState};
use super::runner::{self, predetermined_state};
use super::{AnalysisOutcome, CancelToken};
use crate::error::AnalyzeError;
use crate::state::{step_config_hash, AnalysisSnapshot};

/// Liest die Konfiguration aus der Datei und startet die Analyse; `None`, wenn `analysis.auto_at_end` aus ist.
pub fn run_after_crawl(
    file: &Path,
    cancel: &CancelToken,
) -> Result<Option<AnalysisOutcome>, AnalyzeError> {
    let crawl = CrawlFile::open(file, OpenMode::ReadWrite)?;
    let cfg = crawl.config()?;
    if !cfg.analysis.auto_at_end {
        return Ok(None);
    }
    run_manual(&crawl, &cfg, cancel, &|_, _| {}).map(Some)
}

/// Manueller Start: alle Teilschritte. Läuft der Crawl nicht regulär zu Ende (Pause, Unterbrechung, noch
/// laufend), gilt das Ergebnis als unvollständig (CRAWL-141 Punkt 2).
pub fn run_manual(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
    progress: &dyn Fn(AnalysisStep, f32),
) -> Result<AnalysisOutcome, AnalyzeError> {
    let outcome = runner::run(file, cfg, cancel, progress)?;
    mark_incomplete_if_crawl_unfinished(file)?;
    Ok(outcome)
}

fn mark_incomplete_if_crawl_unfinished(file: &CrawlFile) -> Result<(), AnalyzeError> {
    let state = file.meta()?.state;
    if state != CrawlState::Completed && persist::read_run_state(file)? == RunState::Completed {
        let at = file.get_meta("analysis_at")?;
        persist::write_run_state(file, RunState::Incomplete, None)?;
        if let Some(at) = at {
            file.set_meta("analysis_at", &at)?;
        }
    }
    Ok(())
}

/// Führt ausstehende oder veraltete Schritte aus (vor analyseabhängigen Exporten).
pub fn ensure_analysis(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<AnalysisOutcome, AnalyzeError> {
    let snapshot = AnalysisSnapshot::load(file)?;
    let stale = persist::read_stale(file)?;
    let mut todo: Vec<AnalysisStep> = Vec::new();
    let mut current: Vec<(AnalysisStep, StepState)> = Vec::new();
    for step in AnalysisStep::ALL.iter().copied() {
        let record = snapshot.record(step);
        let up_to_date = match record {
            None => false,
            Some(rec) if rec.state == StepState::Pending || stale.contains(step) => false,
            // Entscheidungen der Konfiguration (Schalter aus, ohne Datengrundlage) gelten, solange sie sich nicht ändern
            Some(rec) => match predetermined_state(step, cfg) {
                Some(state) => rec.state == state,
                // Fertig oder vom Schritt selbst als nicht anwendbar bewertet: aktuell, wenn die für den Schritt
                // relevante Konfiguration dieselbe ist
                None => {
                    matches!(rec.state, StepState::Done | StepState::NotApplicable)
                        && rec.config_hash.as_deref() == Some(step_config_hash(step, cfg).as_str())
                }
            },
        };
        match (up_to_date, record) {
            (true, Some(rec)) => current.push((step, rec.state)),
            _ => todo.push(step),
        }
    }
    if todo.is_empty() {
        return Ok(AnalysisOutcome {
            steps: current,
            complete: true,
        });
    }
    let outcome = runner::run_only(file, cfg, cancel, &|_, _| {}, &todo)?;
    mark_incomplete_if_crawl_unfinished(file)?;
    let mut steps = current;
    steps.extend(outcome.steps);
    steps.sort_by_key(|(s, _)| *s);
    Ok(AnalysisOutcome {
        steps,
        complete: outcome.complete,
    })
}
