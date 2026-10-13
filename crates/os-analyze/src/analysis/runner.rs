//! Runner der Teilschritte (CRAWL-141): Schalter, Abhängigkeiten, Abbruch, Fortschritt.
//!
//! STUB(M1-P14)

use os_model::{AnalysisStep, CrawlConfig};
use os_store::CrawlFile;

use super::{AnalysisOutcome, CancelToken};
use crate::error::AnalyzeError;

/// Führt die Teilschritte aus und meldet den Fortschritt je Schritt (0.0–1.0).
// STUB(M1-P14): kein Schritt ausgeführt.
pub fn run(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
    _progress: &dyn Fn(AnalysisStep, f32),
) -> Result<AnalysisOutcome, AnalyzeError> {
    Ok(AnalysisOutcome {
        steps: Vec::new(),
        complete: true,
    })
}
