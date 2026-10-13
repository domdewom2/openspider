//! Persistenz der Teilschritte in `analysis_steps` und `crawl_meta`.
//!
//! STUB(M1-P14)

use os_model::AnalysisStep;
use os_store::CrawlFile;

use crate::error::AnalyzeError;

/// Markiert Teilschritte als veraltet (z. B. nach „Neu crawlen“).
// STUB(M1-P14)
pub fn mark_stale(_file: &CrawlFile, _steps: &[AnalysisStep]) -> Result<(), AnalyzeError> {
    Ok(())
}
