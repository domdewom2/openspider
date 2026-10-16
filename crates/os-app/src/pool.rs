//! Lese-Pool: alle Datenbankzugriffe der App laufen hier oder in `spawn_blocking`, nie auf dem UI-Thread oder einem
//! Tokio-Worker (`docs/plan/M2.md` 3.6, M2-ui-concept 2.3 Regel 1).
//!
//! STUB(M2-F03): Signatur aus 3.6; M2-F03 setzt den Pool um.

use os_analyze::analysis::CancelToken;
use os_store::{CrawlFile, StoreError};

use crate::dto::CmdError;

/// Schlüssel eines geöffneten Crawls (Kennung als String, UUID v7).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrawlKey(pub String);

/// Pool schreibgeschützter Verbindungen je Crawl.
#[derive(Debug, Default)]
pub struct ReadPool;

impl ReadPool {
    /// Führt `f` auf einer Leseverbindung des Crawls aus (Thread-Pool, abbrechbar über die Marke).
    pub async fn run<T: Send + 'static>(
        &self,
        _crawl: CrawlKey,
        _f: impl FnOnce(&CrawlFile, &CancelToken) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, CmdError> {
        Err(CmdError::unsupported("ReadPool::run", "M2-F03"))
    }
}
