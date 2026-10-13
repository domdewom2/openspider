//! `SqliteSink`: schreibt `WriteBatch`es in die Crawl-Datei (eine Transaktion je Batch).
//!
//! STUB(M1-P07)

use std::path::Path;

use os_model::WriteBatch;

use crate::error::StoreError;
use crate::open::{CrawlFile, OpenMode};
use crate::profile::StorageProfile;
use crate::sink::CrawlSink;

/// Standard-Senke der Engine.
pub struct SqliteSink {
    #[allow(dead_code)]
    file: CrawlFile,
}

impl SqliteSink {
    /// Öffnet die Crawl-Datei zum Schreiben.
    // STUB(M1-P07): öffnet die Datei; P07 ergänzt vorbereitete Statements und Profil-PRAGMAs.
    pub fn open(path: &Path, _profile: StorageProfile) -> Result<Self, StoreError> {
        Ok(SqliteSink {
            file: CrawlFile::open(path, OpenMode::ReadWrite)?,
        })
    }
}

impl CrawlSink for SqliteSink {
    // STUB(M1-P07)
    fn write(&mut self, _batch: WriteBatch) -> Result<(), StoreError> {
        Err(StoreError::NotImplemented("SqliteSink::write (M1-P07)"))
    }

    // STUB(M1-P07)
    fn checkpoint(&mut self) -> Result<(), StoreError> {
        Ok(())
    }
}
