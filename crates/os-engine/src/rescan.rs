//! „Neu crawlen“ und „Entfernen“ einzelner URLs (CRAWL-131).
//!
//! STUB(M1-P19)

use std::path::Path;
use std::sync::Arc;

use os_fetch::Fetch;
use os_model::UrlId;

use crate::error::EngineError;
use crate::types::EngineDeps;

/// Ruft die URLs erneut ab und ersetzt ihre Daten (`PageWrite.replace`).
// STUB(M1-P19)
pub async fn recrawl_urls<F: Fetch>(
    _fetcher: Arc<F>,
    _file: &Path,
    _urls: &[UrlId],
    _deps: EngineDeps,
) -> Result<(), EngineError> {
    Err(EngineError::NotImplemented("recrawl_urls (M1-P19)"))
}

/// Entfernt URLs aus der Crawl-Datei (Inlinks bleiben als Links ohne Status).
// STUB(M1-P19)
pub fn remove_urls(_file: &Path, _urls: &[UrlId]) -> Result<(), EngineError> {
    Err(EngineError::NotImplemented("remove_urls (M1-P19)"))
}
