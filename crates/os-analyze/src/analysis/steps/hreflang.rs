//! Hreflang post-crawl analysis: unlinked alternate targets (TECH-075).
//! Atomically replaces flags and stamps changed target/source URLs for view caches.

use std::collections::BTreeSet;

use os_model::{CrawlConfig, StepState};
use os_store::{queries, CrawlFile};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;
use crate::tabs::hreflang::FLAG_UNLINKED;

/// Internal hyperlink counters include every internal source, including nofollow and self links.
/// External alternates always count as unlinked (TECH-075). One scan, independent of links_target.
const SELECT_FLAGGED: &str =
    "SELECT DISTINCT q.id FROM hreflang h JOIN urls q ON q.id = h.target_id \
    LEFT JOIN url_counters c ON c.url_id = q.id \
    WHERE h.url_id <> q.id AND (q.is_internal = 0 OR COALESCE(c.inlinks, 0) = 0) ORDER BY q.id";

/// Führt den Schritt aus.
pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let conn = file.conn();
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let state = apply(&tx, cfg, cancel)?;
    if state == StepState::Pending {
        tx.rollback()?;
    } else {
        tx.commit()?;
    }
    Ok(state)
}

fn collect(conn: &Connection, sql: &str) -> Result<BTreeSet<i64>, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    rows.collect()
}

fn apply(
    conn: &Connection,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    let wanted = if cfg.spider.hreflang.store {
        collect(conn, SELECT_FLAGGED)?
    } else {
        BTreeSet::new()
    };
    let existing = collect(
        conn,
        &format!("SELECT url_id FROM url_flags WHERE flag = '{FLAG_UNLINKED}'"),
    )?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }

    let removed: Vec<i64> = existing.difference(&wanted).copied().collect();
    let added: Vec<i64> = wanted.difference(&existing).copied().collect();
    if removed.is_empty() && added.is_empty() {
        return Ok(StepState::Done);
    }

    {
        let mut delete =
            conn.prepare_cached("DELETE FROM url_flags WHERE url_id = ?1 AND flag = ?2")?;
        for id in &removed {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            delete.execute(params![id, FLAG_UNLINKED])?;
        }
        let mut insert = conn.prepare_cached(
            "INSERT OR REPLACE INTO url_flags (url_id, flag, value) VALUES (?1, ?2, NULL)",
        )?;
        for id in &added {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            insert.execute(params![id, FLAG_UNLINKED])?;
        }
    }

    // Änderungsfeed: jede geänderte Zeile bekommt die nächste Sequenznummer.
    let seq = queries::change_seq(conn)? + 1;
    {
        let mut stamp = conn.prepare_cached("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
        for id in removed.iter().chain(&added) {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            stamp.execute(params![seq as i64, id])?;
            conn.execute("UPDATE urls SET change_seq = ?1 WHERE id IN (SELECT url_id FROM hreflang WHERE target_id = ?2)", params![seq as i64, id])?;
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO crawl_meta (key, value) VALUES ('change_seq', ?1)",
        params![seq.to_string()],
    )?;
    Ok(StepState::Done)
}
