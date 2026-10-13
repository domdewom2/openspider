//! `crawl_meta`: Metadaten einer Crawl-Datei.

use std::collections::HashMap;
use std::str::FromStr;

use os_model::{CrawlId, CrawlMode, CrawlState, EndReason, Timestamp};

use crate::error::StoreError;
use crate::open::CrawlFile;
use crate::schema;

/// Gelesene Metadaten (`crawl_meta`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlMeta {
    /// Stabile Kennung (UUID v7).
    pub crawl_id: CrawlId,
    /// Modus.
    pub mode: CrawlMode,
    /// Start-URL (Spider-Modus).
    pub start_url: Option<String>,
    /// Projektname.
    pub project: String,
    /// Crawl-Name.
    pub name: String,
    /// Gespeicherter Zustand. Ob ein „Running“ noch lebt, entscheidet `effective_state` (M1-P07).
    pub state: CrawlState,
    /// Grund des Endes.
    pub end_reason: Option<EndReason>,
    /// Startzeitpunkt.
    pub started_at: Option<Timestamp>,
    /// Endzeitpunkt.
    pub ended_at: Option<Timestamp>,
    /// Letzter Heartbeat der Engine.
    pub heartbeat_at: Option<Timestamp>,
    /// Schemaversion der Datei (`PRAGMA user_version`).
    pub schema_version: u32,
}

fn parse<T: FromStr<Err = os_model::ModelError>>(
    map: &HashMap<String, String>,
    key: &str,
) -> Result<T, StoreError> {
    let value = map
        .get(key)
        .ok_or_else(|| StoreError::MissingMeta(key.to_string()))?;
    value.parse::<T>().map_err(|_| StoreError::InvalidMeta {
        key: key.to_string(),
        value: value.clone(),
    })
}

fn parse_opt<T: FromStr<Err = os_model::ModelError>>(
    map: &HashMap<String, String>,
    key: &str,
) -> Result<Option<T>, StoreError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => v
            .parse::<T>()
            .map(Some)
            .map_err(|_| StoreError::InvalidMeta {
                key: key.to_string(),
                value: v.clone(),
            }),
    }
}

fn parse_ts(map: &HashMap<String, String>, key: &str) -> Result<Option<Timestamp>, StoreError> {
    match map.get(key) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => Timestamp::from_db(v)
            .map(Some)
            .map_err(|_| StoreError::InvalidMeta {
                key: key.to_string(),
                value: v.clone(),
            }),
    }
}

impl CrawlFile {
    /// Liest alle Metadaten.
    pub fn meta(&self) -> Result<CrawlMeta, StoreError> {
        let mut stmt = self.conn().prepare("SELECT key, value FROM crawl_meta")?;
        let map: HashMap<String, String> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<_, _>>()?;
        let get = |key: &str| -> Result<String, StoreError> {
            map.get(key)
                .cloned()
                .ok_or_else(|| StoreError::MissingMeta(key.to_string()))
        };
        Ok(CrawlMeta {
            crawl_id: parse(&map, "crawl_id")?,
            mode: parse(&map, "mode")?,
            start_url: map.get("start_url").filter(|v| !v.is_empty()).cloned(),
            project: get("project")?,
            name: get("name")?,
            state: parse(&map, "state")?,
            end_reason: parse_opt(&map, "end_reason")?,
            started_at: parse_ts(&map, "started_at")?,
            ended_at: parse_ts(&map, "ended_at")?,
            heartbeat_at: parse_ts(&map, "heartbeat_at")?,
            schema_version: schema::user_version(self.conn())?,
        })
    }

    /// Liest einen einzelnen `crawl_meta`-Schlüssel.
    pub fn get_meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        let mut stmt = self
            .conn()
            .prepare_cached("SELECT value FROM crawl_meta WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Setzt einen `crawl_meta`-Schlüssel (Upsert).
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO crawl_meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}
