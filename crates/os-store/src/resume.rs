//! Zustand für Fortsetzen und Absturzerkennung.
//!
//! STUB(M1-P07)

use os_model::{Clock, CrawlState, UrlId};

use crate::error::StoreError;
use crate::open::CrawlFile;

/// Eintrag der Warteschlange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    /// URL.
    pub id: UrlId,
    /// Reihenfolge innerhalb der Tiefe.
    pub seq: u64,
    /// Crawl-Tiefe (bei Sitemap-only-URLs ohne Tiefe).
    pub depth: Option<u32>,
    /// 0 Link, 1 nur Sitemap.
    pub priority: u8,
}

/// Zustand zum Fortsetzen eines Crawls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResumeState {
    /// Nächste freie URL-Kennung.
    pub next_url_id: i64,
    /// Nächste freie Reihenfolgenummer der Warteschlange.
    pub next_seq: u64,
    /// Warteschlange in Abruf-Reihenfolge (Priorität, Tiefe, `seq`).
    pub queue: Vec<QueueEntry>,
}

impl CrawlFile {
    /// Tatsächlicher Zustand: „Running“ mit Heartbeat älter als 10 s gilt als „Incomplete“.
    // STUB(M1-P07): liefert den gespeicherten Zustand.
    pub fn effective_state(&self, _clock: &dyn Clock) -> Result<CrawlState, StoreError> {
        Ok(self.meta()?.state)
    }

    /// Liest den Zustand zum Fortsetzen.
    // STUB(M1-P07): nur Zähler aus `crawl_meta`, leere Warteschlange.
    pub fn resume_state(&self) -> Result<ResumeState, StoreError> {
        let read = |key: &str| -> Result<i64, StoreError> {
            let value: String = self.conn().query_row(
                "SELECT value FROM crawl_meta WHERE key = ?1",
                [key],
                |r| r.get(0),
            )?;
            value.parse().map_err(|_| StoreError::InvalidMeta {
                key: key.to_string(),
                value,
            })
        };
        Ok(ResumeState {
            next_url_id: read("next_url_id")?,
            next_seq: u64::try_from(read("next_seq")?).unwrap_or(0),
            queue: Vec::new(),
        })
    }
}
