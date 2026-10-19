//! Zustand für Fortsetzen und Absturzerkennung (CRAWL-125, CRAWL-128, OUT-230).
//!
//! * [`CrawlFile::effective_state`]: ein gespeichertes „Running“ gilt nur, solange der Heartbeat der Engine
//!   höchstens [`HEARTBEAT_TIMEOUT_MS`] alt ist; sonst ist der Crawl abgestürzt („Incomplete“).
//! * [`CrawlFile::resume_state`]: nächste freie Kennungen und die vollständige Warteschlange in
//!   Abrufreihenfolge.
//! * [`CrawlFile::for_each_url_key`] / [`CrawlFile::url_keys_page`]: alle `(id, url)` für das Seen-Set.

use os_model::{Clock, CrawlState, Timestamp, UrlId};
use rusqlite::OptionalExtension;

use crate::error::StoreError;
use crate::open::CrawlFile;

/// Ein Heartbeat, der älter ist als diese Zeit (Millisekunden), gilt als ausgeblieben.
pub const HEARTBEAT_TIMEOUT_MS: i64 = 10_000;

/// Wertet den gespeicherten Zustand anhand der letzten Aktivität aus, ohne Dateizugriff.
/// Aufrufer wählen Heartbeat, Startzeit oder Anlegezeit in dieser Reihenfolge.
pub fn effective_state_at(
    state: CrawlState,
    reference: Option<Timestamp>,
    now: Timestamp,
) -> CrawlState {
    if state == CrawlState::Running
        && !reference.is_some_and(|t| {
            now.unix_millis().saturating_sub(t.unix_millis()) <= HEARTBEAT_TIMEOUT_MS
        })
    {
        CrawlState::Incomplete
    } else {
        state
    }
}

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
    /// Tatsächlicher Zustand: „Running“ mit Heartbeat älter als 10 s gilt als „Incomplete“. Fehlt der
    /// Heartbeat (Crawl gerade angelegt), zählt `started_at`, danach `created_at`; ist nichts davon lesbar,
    /// gilt ein „Running“ als abgestürzt. Alle anderen Zustände bleiben unverändert.
    pub fn effective_state(&self, clock: &dyn Clock) -> Result<CrawlState, StoreError> {
        let meta = self.meta()?;
        if meta.state != CrawlState::Running {
            return Ok(meta.state);
        }
        let reference = match meta.heartbeat_at.or(meta.started_at) {
            Some(t) => Some(t),
            None => match self.get_meta("created_at")? {
                Some(v) => os_model::Timestamp::from_db(&v).ok(),
                None => None,
            },
        };
        Ok(effective_state_at(meta.state, reference, clock.now()))
    }

    /// Liest den Zustand zum Fortsetzen. `next_url_id` und `next_seq` sind mindestens so groß wie die
    /// größte vergebene Kennung bzw. Reihenfolgenummer in der Datei.
    pub fn resume_state(&self) -> Result<ResumeState, StoreError> {
        let conn = self.conn();
        let read = |key: &str| -> Result<i64, StoreError> {
            let value: Option<String> = conn
                .query_row("SELECT value FROM crawl_meta WHERE key = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()?;
            match value {
                None => Ok(0),
                Some(value) => value.parse().map_err(|_| StoreError::InvalidMeta {
                    key: key.to_string(),
                    value,
                }),
            }
        };
        let max_id: i64 =
            conn.query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
        let max_seq: i64 =
            conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM queue", [], |r| r.get(0))?;
        let next_url_id = read("next_url_id")?.max(max_id.saturating_add(1)).max(1);
        let next_seq = read("next_seq")?.max(max_seq.saturating_add(1)).max(1);

        let mut stmt = conn.prepare(
            "SELECT url_id, seq, depth, priority FROM queue ORDER BY priority, depth, seq",
        )?;
        let queue = stmt
            .query_map([], |r| {
                Ok(QueueEntry {
                    id: UrlId(r.get(0)?),
                    seq: u64::try_from(r.get::<_, i64>(1)?).unwrap_or(0),
                    depth: r
                        .get::<_, Option<i64>>(2)?
                        .and_then(|d| u32::try_from(d).ok()),
                    priority: u8::try_from(r.get::<_, i64>(3)?).unwrap_or(u8::MAX),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ResumeState {
            next_url_id,
            next_seq: u64::try_from(next_seq).unwrap_or(1),
            queue,
        })
    }

    /// Ruft `f` für jede URL der Datei mit Kennung und URL Encoded Address auf (aufsteigend nach Kennung),
    /// ohne die Liste zu materialisieren. Liefert die Anzahl der Aufrufe; ein Fehler von `f` bricht ab.
    pub fn for_each_url_key(
        &self,
        f: &mut dyn FnMut(UrlId, &str) -> Result<(), StoreError>,
    ) -> Result<u64, StoreError> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, url FROM urls ORDER BY id")?;
        let mut rows = stmt.query([])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            let url: String = row.get(1)?;
            f(UrlId(row.get(0)?), &url)?;
            count += 1;
        }
        Ok(count)
    }

    /// Eine Seite des Schlüsselverzeichnisses: höchstens `limit` Paare `(id, url)` mit Kennung `> after`.
    pub fn url_keys_page(
        &self,
        after: Option<UrlId>,
        limit: u32,
    ) -> Result<Vec<(UrlId, String)>, StoreError> {
        let mut stmt = self
            .conn()
            .prepare_cached("SELECT id, url FROM urls WHERE id > ?1 ORDER BY id LIMIT ?2")?;
        let rows = stmt
            .query_map(
                rusqlite::params![after.map_or(i64::MIN, |a| a.0), limit],
                |r| Ok((UrlId(r.get(0)?), r.get(1)?)),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
