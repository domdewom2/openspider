//! Auslösung der Crawl-Analyse durch den Aufrufer der Engine (CRAWL-141).
//!
//! STUB(M1-P19): `os-engine` ruft nur den Hook `EngineDeps.on_finished`; `os-cli` setzt dort
//! `run_after_crawl`. Die Engine hängt nie an `os-analyze`.

use std::path::Path;

use os_model::CrawlConfig;
use os_store::CrawlFile;

use super::{AnalysisOutcome, CancelToken};
use crate::error::AnalyzeError;

/// Liest die Konfiguration aus der Datei und startet die Analyse; `None`, wenn `analysis.auto_at_end` aus ist.
// STUB(M1-P19)
pub fn run_after_crawl(
    _file: &Path,
    _cancel: &CancelToken,
) -> Result<Option<AnalysisOutcome>, AnalyzeError> {
    Ok(None)
}

/// Führt ausstehende oder veraltete Schritte aus (vor analyseabhängigen Exporten).
// STUB(M1-P19)
pub fn ensure_analysis(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<AnalysisOutcome, AnalyzeError> {
    Ok(AnalysisOutcome {
        steps: Vec::new(),
        complete: true,
    })
}
