//! Analyse-Teilschritt `Links`: Links: Follow-/Nofollow-Inlinks und nur nicht indexierbare Inlinks (TECH-139 ff.).
//!
//! STUB(M2-F02): der Schritt ist in der Registry (`runner::default_registry`) angemeldet und liefert bis zur Umsetzung
//! durch M2-D08 `NotApplicable` – wie jeder Schritt ohne Umsetzung in M1.

use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;

// STUB(M2-D08)
/// Führt den Schritt aus.
pub fn run(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}
