//! Crawl-Kern: Start eines Crawls und das Handle zur Steuerung (Datenfluss M1-Plan 3.7).
//!
//! STUB(M1-P15): P15 ersetzt `start_crawl` und die Handle-Internas; die Signaturen bleiben. `pause` setzt
//! P19 um.

use std::sync::Arc;

use os_fetch::Fetch;
use os_model::{CrawlId, Progress};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::error::EngineError;
use crate::types::{CrawlRequest, CrawlSummary, EngineDeps};

/// Steuerung eines laufenden Crawls.
pub struct CrawlHandle {
    crawl_id: CrawlId,
    progress: watch::Receiver<Progress>,
    task: JoinHandle<Result<CrawlSummary, EngineError>>,
}

impl CrawlHandle {
    /// Kennung des Crawls.
    pub fn crawl_id(&self) -> CrawlId {
        self.crawl_id
    }

    /// Aktueller Fortschritt.
    pub fn progress(&self) -> Progress {
        self.progress.borrow().clone()
    }

    /// Beobachter des Fortschritts.
    pub fn watch(&self) -> watch::Receiver<Progress> {
        self.progress.clone()
    }

    /// Pausiert den Crawl (P19).
    // STUB(M1-P19)
    pub fn pause(&self) {}

    /// Beendet den Crawl; die Warteschlange bleibt erhalten.
    // STUB(M1-P15)
    pub fn stop(&self) {}

    /// Ändert die Zahl paralleler Threads (wirkt binnen 2 s).
    // STUB(M1-P15)
    pub fn set_threads(&self, _n: u32) {}

    /// Wartet auf das Ende und liefert die Zusammenfassung.
    pub async fn wait(self) -> Result<CrawlSummary, EngineError> {
        self.task
            .await
            .map_err(|e| EngineError::Join(e.to_string()))?
    }
}

/// Startet einen neuen Crawl.
// STUB(M1-P15)
pub async fn start_crawl<F: Fetch>(
    _fetcher: Arc<F>,
    _req: CrawlRequest,
    _deps: EngineDeps,
) -> Result<CrawlHandle, EngineError> {
    Err(EngineError::NotImplemented("start_crawl (M1-P15)"))
}
