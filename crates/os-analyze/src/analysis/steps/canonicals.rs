//! Analyse-Teilschritt: Kanonisierungen (TECH-018: Non-Indexable Canonical).
//!
//! STUB(M1-P24)

use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;

/// Führt den Schritt aus.
// STUB(M1-P24): Stub meldet `NotApplicable`.
pub fn run(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}
