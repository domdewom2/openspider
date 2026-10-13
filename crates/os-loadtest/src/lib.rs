//! `os-loadtest` – Lasttest-Runner nach PERF-009.
//!
//! Gerüst: Der Runner startet die synthetische Site eines Szenarios aus `os-testsite`,
//! misst den Testserver (Durchsatz, Latenz, Determinismus) und schreibt
//! `target/loadtest/report.json`. Sobald `os-engine` das Trait [`CrawlerUnderTest`]
//! implementiert, kommen die Crawler-Messwerte (RSS, URLs/s, ...) in dieselben Berichte.

pub mod crawler;
pub mod report;
pub mod runner;
pub mod scenario;

pub use crawler::{CrawlJob, CrawlOutcome, CrawlerError, CrawlerUnderTest, NoCrawler};
pub use report::Report;
pub use scenario::{ScenarioDef, Size, SCENARIOS};
