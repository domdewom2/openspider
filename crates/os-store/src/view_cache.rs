//! Disposable view blobs. Read-only crawls never write cache data.
use rusqlite::{params, OptionalExtension};

use crate::{CrawlFile, StoreError};

/// Versioned opaque cache entry; interpretation belongs to `os-analyze`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub step_stamp: String,
    pub built_seq: u64,
    pub data: Vec<u8>,
}

pub fn load(file: &CrawlFile, key: &str, params_hash: &str) -> Result<Option<Entry>, StoreError> {
    Ok(file.conn().query_row(
        "SELECT step_stamp, built_seq, data FROM view_cache WHERE key = ?1 AND params_hash = ?2",
        params![key, params_hash],
        |r| Ok(Entry { step_stamp: r.get(0)?, built_seq: r.get(1)?, data: r.get(2)? }),
    ).optional()?)
}

/// Returns false for a read-only connection. Other write errors remain visible to callers.
pub fn save(
    file: &CrawlFile,
    key: &str,
    params_hash: &str,
    entry: &Entry,
) -> Result<bool, StoreError> {
    if file.conn().is_readonly("main")? || file.get_meta("state")?.as_deref() == Some("Running") {
        return Ok(false);
    }
    let timeout: u64 = file
        .conn()
        .pragma_query_value(None, "busy_timeout", |r| r.get(0))?;
    file.conn().busy_timeout(std::time::Duration::ZERO)?;
    let result = save_now(file, key, params_hash, entry);
    file.conn()
        .busy_timeout(std::time::Duration::from_millis(timeout))?;
    result
}
fn save_now(
    file: &CrawlFile,
    key: &str,
    params_hash: &str,
    entry: &Entry,
) -> Result<bool, StoreError> {
    let tx = file.conn().unchecked_transaction()?;
    tx.execute(
        "DELETE FROM view_cache WHERE key=?1 AND params_hash<>?2",
        params![key, params_hash],
    )?;
    tx.execute(
        "INSERT INTO view_cache (key, params_hash, step_stamp, built_seq, data) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(key, params_hash) DO UPDATE SET step_stamp = excluded.step_stamp, built_seq = excluded.built_seq, data = excluded.data",
        params![key, params_hash, entry.step_stamp, entry.built_seq, entry.data],
    )?;
    tx.commit()?;
    Ok(true)
}

pub fn clear(file: &CrawlFile) -> Result<bool, StoreError> {
    if file.conn().is_readonly("main")? {
        return Ok(false);
    }
    file.conn().execute("DELETE FROM view_cache", [])?;
    Ok(true)
}
