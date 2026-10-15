//! `os-loadtest` – Lasttest-Runner nach PERF-009.
//!
//! Der Runner startet die synthetische Site eines Szenarios aus `os-testsite`, misst den Testserver (Durchsatz,
//! Latenz, Determinismus), lässt den Crawler ([`engine::EngineCrawler`], os-engine im Kindprozess) gegen die Site
//! laufen und schreibt `target/loadtest/report.json`. PERF-001 (Speicherobergrenze), PERF-003 (Gegendruck) und
//! PERF-005 (Absturzsicherheit) werden gegen die Kriterien der Spec geprüft; ein verletztes Kriterium lässt den
//! Lauf scheitern. Kleine Varianten (10 000 Seiten) laufen in jeder CI (`loadtest-small`), die großen per
//! `os-loadtest run --size large`.

pub mod child;
pub mod crawler;
pub mod diag;
pub mod engine;
pub mod proc;
pub mod report;
pub mod rss;
pub mod runner;
pub mod scenario;
pub mod sink;
pub mod throughput;
pub mod timers;

pub use crawler::{CrawlJob, CrawlOutcome, CrawlerError, CrawlerUnderTest, NoCrawler};
pub use engine::EngineCrawler;
pub use report::Report;
pub use scenario::{ScenarioDef, Size, SCENARIOS};
