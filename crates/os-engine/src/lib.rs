//! `os-engine` – Frontier, Scheduler, Worker-Pool, Pause/Resume und Crawl-Modi
//! (Spider, Liste, SERP, Vergleich). Orchestriert fetch, parse, store und optional render.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Die Engine hängt **nicht** an `os-analyze` (M1-Plan 3.2): Nach dem
//! Crawl-Ende ruft sie nur den Hook [`EngineDeps::on_finished`]. Kern: P15; Link-Entscheidung, Sitemap-Ingest
//! und robots-Glue: P15b; Lebenszyklus: P19; Testhelfer (Feature `testing`): P15.
//!
//! Ab M2 (M2-F02): Rendering-Hook ([`Renderer`], `EngineDeps.renderer`, `render_glue`), [`add_urls`] und `memory`
//! (Ressourcenschutz, CRAWL-129) als Stubs der Pakete E02/E05.

pub mod crawl;
pub mod decide;
pub mod diag;
pub mod disk_guard;
pub mod error;
pub mod frontier;
pub mod lifecycle;
pub mod list_input;
pub mod memory;
pub mod progress;
pub mod queue;
pub(crate) mod queue_disk;
pub(crate) mod queue_maint;
pub mod redirect;
pub mod render_glue;
pub mod rescan;
pub mod resume;
pub mod robots_glue;
pub mod seen;
pub mod sitemaps;
pub mod types;
pub mod worker;
pub mod writer_thread;

#[cfg(feature = "testing")]
pub mod testing;

pub use crawl::{start_crawl, CrawlHandle};
pub use error::EngineError;
pub use lifecycle::resume_crawl;
pub use rescan::{recrawl_urls, remove_urls};
pub use render_glue::Renderer;
pub use types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps, FinishedHook, SinkFactory};

/// Ergebnis von [`add_urls`]: was aus den übergebenen Adressen geworden ist (CRAWL-131).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AddUrlsSummary {
    /// Neu eingereihte URLs.
    pub added: u64,
    /// Bereits bekannte URLs (Duplikate).
    pub duplicates: u64,
    /// Ungültige Adressen.
    pub invalid: u64,
    /// Durch Include/Exclude oder den Umfang ausgeschlossene URLs.
    pub excluded: u64,
}

/// Fügt einem laufenden oder pausierten Crawl URLs hinzu (CRAWL-131 [P1], Command `crawl.addUrls`).
///
/// STUB(M2-E02): liefert bis zur Umsetzung `EngineError::NotImplemented`.
pub fn add_urls(_handle: &CrawlHandle, _urls: &[String]) -> Result<AddUrlsSummary, EngineError> {
    Err(EngineError::NotImplemented("add_urls (M2-E02)"))
}
