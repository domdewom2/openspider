//! Serialisierte Schreibwarteschlange der App (K-20): eigener Thread, eigene Verbindung, `busy_timeout` 5 s.
//!
//! STUB(M2-F03): Signatur aus `docs/plan/M2.md` 3.6; M2-F03 setzt die Warteschlange um.

use os_store::StoreError;

use crate::dto::CmdError;
use crate::pool::CrawlKey;

/// Die einzige Schreibstelle der App neben dem Engine-Writer.
#[derive(Debug, Default)]
pub struct WriteQueue;

impl WriteQueue {
    /// Führt `f` mit der Schreibverbindung des Crawls aus.
    pub async fn submit<T: Send + 'static>(
        &self,
        _crawl: CrawlKey,
        _f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, CmdError> {
        Err(CmdError::unsupported("WriteQueue::submit", "M2-F03"))
    }
}
