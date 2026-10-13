//! Schnittstelle zum getesteten Crawler.
//!
//! `os-engine` implementiert [`CrawlerUnderTest`], sobald der Crawler existiert. Bis dahin
//! liefert [`NoCrawler`] immer [`CrawlerError::NotImplemented`]; der Runner misst dann nur
//! den Testserver.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::scenario::Size;

/// Auftrag an den Crawler.
#[derive(Debug, Clone)]
pub struct CrawlJob {
    /// Name des Lastszenarios (z. B. `perf-001`).
    pub scenario: String,
    /// Größe des Laufs.
    pub size: Size,
    /// Start-URL der synthetischen Site (Startseite).
    pub start_url: String,
    /// Erwartete Anzahl erreichbarer URLs.
    pub expected_urls: u64,
}

/// Messwerte eines Crawl-Laufs.
#[derive(Debug, Clone, Default)]
pub struct CrawlOutcome {
    /// Anzahl abgeschlossener URLs.
    pub urls_crawled: u64,
    /// Gesamtdauer des Crawls.
    pub duration: Duration,
    /// Spitzen-RSS des Kernprozesses in Bytes (PERF-001/003).
    pub peak_rss_bytes: Option<u64>,
    /// Weitere szenariospezifische Messwerte; landen mit Präfix `crawler_` im Bericht.
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Fehler eines Crawl-Laufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlerError {
    /// Es ist (noch) kein Crawler angebunden.
    NotImplemented,
    /// Der Lauf ist fehlgeschlagen.
    Failed(String),
}

/// Boxed Future, damit das Trait objektsicher bleibt.
pub type CrawlFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CrawlOutcome, CrawlerError>> + Send + 'a>>;

/// Der Crawler, der gegen die Testsite läuft (später `os-engine`).
pub trait CrawlerUnderTest: Send + Sync {
    /// Anzeigename im Bericht.
    fn name(&self) -> &str;

    /// Crawlt die Site vollständig und liefert die Messwerte.
    fn crawl(&self, job: CrawlJob) -> CrawlFuture<'_>;
}

/// Platzhalter, solange der Crawler nicht existiert.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCrawler;

impl CrawlerUnderTest for NoCrawler {
    fn name(&self) -> &str {
        "none"
    }

    fn crawl(&self, _job: CrawlJob) -> CrawlFuture<'_> {
        Box::pin(async { Err(CrawlerError::NotImplemented) })
    }
}
