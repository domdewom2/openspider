//! Verwaltung der Crawl-Dateien im Crawl-Ordner (`<crawls>/<crawl_id>.ospider`).
//!
//! STUB(M1-P07)

use std::path::{Path, PathBuf};

use os_model::CrawlId;

use crate::error::StoreError;
use crate::meta::CrawlMeta;

/// Eintrag der Crawl-Liste.
#[derive(Debug, Clone)]
pub struct CrawlListing {
    /// Pfad der Datei.
    pub path: PathBuf,
    /// Metadaten (ohne Migration gelesen).
    pub meta: CrawlMeta,
    /// Anzahl der URLs in der Datei.
    pub url_count: u64,
}

/// Listet alle Crawl-Dateien im Ordner.
// STUB(M1-P07): leere Liste
pub fn list_crawls(_dir: &Path) -> Result<Vec<CrawlListing>, StoreError> {
    Ok(Vec::new())
}

/// Sucht die Datei zu einer Crawl-Kennung.
// STUB(M1-P07)
pub fn find(_dir: &Path, _id: &CrawlId) -> Option<PathBuf> {
    None
}
