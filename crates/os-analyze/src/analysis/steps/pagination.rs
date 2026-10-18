//! Nachlauf der Paginierung: unverlinkte Ziele, gerichtete Zyklen und Rückverweise.

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;
use crate::tabs::pagination::{FLAG_LOOP, FLAG_SEQUENCE, FLAG_UNLINKED};
use os_model::{CrawlConfig, StepState};
use os_store::{queries, CrawlFile};
use rusqlite::{params, Connection};
use std::collections::{BTreeSet, HashMap, HashSet};

fn collect(conn: &Connection, sql: &str) -> Result<BTreeSet<i64>, rusqlite::Error> {
    conn.prepare(sql)?.query_map([], |r| r.get(0))?.collect()
}

// Ein ausgehender Pfeil je Knoten: linearer Besuch, nur die Zyklusmitglieder markieren.
fn cycles(edges: &HashMap<i64, i64>, cancel: &CancelToken) -> BTreeSet<i64> {
    let mut done = HashSet::new();
    let mut result = BTreeSet::new();
    for &start in edges.keys() {
        if cancel.is_cancelled() {
            break;
        }
        let mut path = Vec::new();
        let mut positions = HashMap::new();
        let mut node = start;
        while !done.contains(&node) {
            if cancel.is_cancelled() {
                return result;
            }
            if let Some(&index) = positions.get(&node) {
                result.extend(&path[index..]);
                break;
            }
            positions.insert(node, path.len());
            path.push(node);
            match edges.get(&node) {
                Some(&target) => node = target,
                None => break,
            }
        }
        done.extend(path);
    }
    result
}

/// Ersetzt eigene Markierungen atomar; Abbruch veröffentlicht keine Teilergebnisse.
pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let conn = file.conn();
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match apply(conn, cfg, cancel) {
        Ok(StepState::Pending) => {
            conn.execute_batch("ROLLBACK")?;
            Ok(StepState::Pending)
        }
        Ok(state) => {
            conn.execute_batch("COMMIT")?;
            Ok(state)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn apply(
    conn: &Connection,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    let mut unlinked = BTreeSet::new();
    let mut loops = BTreeSet::new();
    let mut sequence = BTreeSet::new();
    if cfg.spider.pagination.store {
        // Ein Lauf über Hyperlinks; funktioniert auch ohne links_target-Index.
        unlinked = collect(
            conn,
            "SELECT DISTINCT p.url_id FROM pagination p JOIN urls t ON t.id = p.target_id \
            WHERE t.state = 'done' AND p.target_id NOT IN \
            (SELECT l.target_id FROM links l JOIN urls s ON s.id = l.source_id \
             WHERE s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.target_id IS NOT NULL)",
        )?;
        sequence = collect(conn, "SELECT DISTINCT p.url_id FROM pagination p JOIN urls t ON t.id = p.target_id \
            WHERE t.state = 'done' AND t.status_code = 200 AND t.content_class = 'HTML' \
            AND NOT EXISTS (SELECT 1 FROM pagination r WHERE r.url_id = t.id AND r.target_id = p.url_id \
                AND r.rel = CASE p.rel WHEN 'next' THEN 'prev' ELSE 'next' END)")?;
        for rel in ["next", "prev"] {
            let mut edges = HashMap::new();
            let mut stmt = conn.prepare(
                "WITH first_ref AS (SELECT p.*, ROW_NUMBER() OVER \
                (PARTITION BY p.url_id ORDER BY p.position, p.source) AS ordinal \
                FROM pagination p WHERE p.rel = ?1) \
                SELECT p.url_id, p.target_id FROM first_ref p \
                JOIN urls s ON s.id = p.url_id JOIN urls t ON t.id = p.target_id \
                WHERE p.ordinal = 1 AND s.state = 'done' AND t.state = 'done'",
            )?;
            let rows = stmt.query_map([rel], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            for row in rows {
                let (source, target) = row?;
                edges.entry(source).or_insert(target);
                if cancel.is_cancelled() {
                    return Ok(StepState::Pending);
                }
            }
            loops.extend(cycles(&edges, cancel));
        }
    }
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let mut changed = BTreeSet::new();
    for (flag, wanted) in [
        (FLAG_UNLINKED, unlinked),
        (FLAG_LOOP, loops),
        (FLAG_SEQUENCE, sequence),
    ] {
        let existing = collect(
            conn,
            &format!("SELECT url_id FROM url_flags WHERE flag = '{flag}'"),
        )?;
        for id in existing.difference(&wanted) {
            conn.execute(
                "DELETE FROM url_flags WHERE url_id = ?1 AND flag = ?2",
                params![id, flag],
            )?;
            changed.insert(*id);
        }
        for id in wanted.difference(&existing) {
            conn.execute(
                "INSERT INTO url_flags (url_id, flag, value) VALUES (?1, ?2, NULL)",
                params![id, flag],
            )?;
            changed.insert(*id);
        }
    }
    if !changed.is_empty() {
        let seq = queries::change_seq(conn)? + 1;
        for id in changed {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            conn.execute(
                "UPDATE urls SET change_seq = ?1 WHERE id = ?2",
                params![seq as i64, id],
            )?;
        }
        conn.execute(
            "INSERT OR REPLACE INTO crawl_meta (key, value) VALUES ('change_seq', ?1)",
            [seq.to_string()],
        )?;
    }
    if cancel.is_cancelled() {
        Ok(StepState::Pending)
    } else {
        Ok(StepState::Done)
    }
}
