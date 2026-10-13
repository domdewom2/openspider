//! Dateiverwaltung einer Crawl-Datei: Kopie, Integritätsprüfung, Reparatur.
//!
//! STUB(M1-P07)

use std::path::Path;

use crate::error::StoreError;
use crate::open::CrawlFile;

impl CrawlFile {
    /// Speichert eine eigenständige Kopie (ohne `-wal`/`-shm`).
    // STUB(M1-P07)
    pub fn save_as(&self, _dest: &Path) -> Result<(), StoreError> {
        Err(StoreError::NotImplemented("CrawlFile::save_as (M1-P07)"))
    }

    /// `PRAGMA integrity_check`: `true` bei `ok`.
    // STUB(M1-P07)
    pub fn integrity_check(&self) -> Result<bool, StoreError> {
        let result: String = self
            .conn()
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(result == "ok")
    }
}
