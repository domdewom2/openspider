//! Crawl-Analyse (CRAWL-141): Teilschritte nach dem Crawl. Der Runner (`runner`, `persist`) kommt von
//! P14, die Auslösung (`trigger`) von P19, die Schritte (`steps`) von P24/P25.

pub mod persist;
pub mod runner;
pub mod steps;
pub mod trigger;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use os_model::{AnalysisStep, StepState};

pub use persist::mark_stale;
pub use runner::run;
pub use trigger::{ensure_analysis, run_after_crawl};

/// Abbruchmarke; Klone teilen den Zustand.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// Neue, nicht abgebrochene Marke.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bricht ab (für alle Klone).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Wurde abgebrochen?
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Ergebnis eines Analyselaufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisOutcome {
    /// Zustand je Teilschritt.
    pub steps: Vec<(AnalysisStep, StepState)>,
    /// Alle anwendbaren Schritte sind fertig (nicht abgebrochen).
    pub complete: bool,
}
