//! Analyse-Teilschritt: Sitemaps und Orphan-URLs (TECH-212 bis TECH-215).
//!
//! STUB(M1-P25)

use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;

/// Führt den Schritt aus.
// STUB(M1-P25): Stub meldet `NotApplicable`.
pub fn run(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}
