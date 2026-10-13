//! Lebenszyklus: Pause, Fortsetzen, Absturzerholung.
//!
//! STUB(M1-P19)

use std::path::PathBuf;
use std::sync::Arc;

use os_fetch::Fetch;
use os_model::CrawlConfig;

use crate::crawl::CrawlHandle;
use crate::error::EngineError;
use crate::types::EngineDeps;

/// Setzt einen pausierten oder abgebrochenen Crawl fort (optional mit geänderter Konfiguration).
// STUB(M1-P19)
pub async fn resume_crawl<F: Fetch>(
    _fetcher: Arc<F>,
    _file: PathBuf,
    _config: Option<CrawlConfig>,
    _deps: EngineDeps,
) -> Result<CrawlHandle, EngineError> {
    Err(EngineError::NotImplemented("resume_crawl (M1-P19)"))
}
