//! Analyse-Teilschritt `Pagination`: Paginierung: Schleifen, Sequenzfehler, nicht verlinkte Pagination-URLs (TECH-037, TECH-040, TECH-041).
//!
//! STUB(M2-F02): der Schritt ist in der Registry (`runner::default_registry`) angemeldet und liefert bis zur Umsetzung
//! durch M2-D05 `NotApplicable` – wie jeder Schritt ohne Umsetzung in M1.

use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;

// STUB(M2-D05)
/// Führt den Schritt aus.
pub fn run(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    Ok(StepState::NotApplicable)
}
