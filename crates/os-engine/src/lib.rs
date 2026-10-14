//! `os-engine` – Frontier, Scheduler, Worker-Pool, Pause/Resume und Crawl-Modi
//! (Spider, Liste, SERP, Vergleich). Orchestriert fetch, parse, store und optional render.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Die Engine hängt **nicht** an `os-analyze` (M1-Plan 3.2): Nach dem
//! Crawl-Ende ruft sie nur den Hook [`EngineDeps::on_finished`]. Kern: P15; Link-Entscheidung, Sitemap-Ingest
//! und robots-Glue: P15b; Lebenszyklus: P19; Testhelfer (Feature `testing`): P15.

pub mod crawl;
pub mod decide;
pub mod diag;
pub mod disk_guard;
pub mod error;
pub mod frontier;
pub mod lifecycle;
pub mod list_input;
pub mod progress;
pub mod redirect;
pub mod rescan;
pub mod resume;
pub mod robots_glue;
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
pub use types::{CrawlRequest, CrawlSource, CrawlSummary, EngineDeps, FinishedHook, SinkFactory};
