//! Analyse-Teilschritt: Kanonisierungen (TECH-018: Non-Indexable Canonical).
//!
//! Eine Seite bekommt die Markierung `non_indexable_canonical` (Tabelle `url_flags`), wenn ihre maßgebliche
//! Canonical (TECH-017) auf eine andere URL zeigt (`Canonicalised`, TECH-014) und das Ziel nach TECH-001
//! `Non-Indexable` ist: robots-gesperrt, ohne Antwort, 3xx, 4xx, 5xx, `noindex` oder selbst kanonisiert.
//! Externe Ziele sind nur nach ihrem Status bewertet, denn für sie gibt es keine Seitenfakten.
//!
//! Ein Ziel ohne Bewertung (nicht gecrawlt: ausgeschlossen, außerhalb des Umfangs oder Canonicals-Crawlen aus)
//! hat keine `indexability`; die Seite wird dann nicht markiert, weil der Status unbekannt ist. Ohne
//! gespeicherte Canonicals (`spider.canonicals.store` aus) gibt es keine Canonical-Zeilen und damit keine
//! Markierung.
//!
//! Der Schritt ersetzt die Markierungen vollständig und ist damit wiederholbar: derselbe Crawl liefert bei
//! jedem Lauf dieselbe Menge. Geänderte Zeilen erhalten eine neue `change_seq`, damit der Änderungsfeed
//! (M1.md 3.9) die neue Zugehörigkeit zum Filter meldet.

use std::collections::BTreeSet;

use os_model::{CrawlConfig, StepState};
use os_store::{queries, CrawlFile};
use rusqlite::{params, Connection};

use crate::analysis::CancelToken;
use crate::error::AnalyzeError;
use crate::tabs::canonicals::FLAG_NON_INDEXABLE_CANONICAL;

/// Seiten, deren Canonical auf ein nicht indexierbares, bewertetes Ziel zeigt.
const SELECT_FLAGGED: &str = "SELECT u.id FROM urls u \
     JOIN urls t ON t.id = u.canonical_target_id \
     WHERE u.is_internal = 1 AND u.status_code BETWEEN 200 AND 299 \
       AND u.canonical_count >= 1 AND u.canonical_self = 0 \
       AND t.indexability = 'Non-Indexable' \
     ORDER BY u.id";

/// Führt den Schritt aus.
pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    let conn = file.conn();
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match apply(conn, cfg, cancel) {
        Ok(state) => {
            conn.execute_batch(if state == StepState::Done {
                "COMMIT"
            } else {
                "ROLLBACK"
            })?;
            Ok(state)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn collect(
    conn: &Connection,
    sql: &str,
    cancel: &CancelToken,
) -> Result<Option<BTreeSet<i64>>, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut ids = BTreeSet::new();
    while let Some(row) = rows.next()? {
        if cancel.checkpoint().is_err() {
            return Ok(None);
        }
        ids.insert(row.get(0)?);
    }
    Ok(Some(ids))
}

/// Nur interne Hyperlinks zählen; der einmalige Linkscan funktioniert auch ohne links_target-Index.
const SELECT_UNLINKED: &str = "SELECT u.id FROM urls u JOIN urls t ON t.id = u.canonical_target_id
    WHERE u.is_internal = 1 AND u.status_code BETWEEN 200 AND 299
      AND u.canonical_count >= 1 AND u.canonical_self = 0 AND t.indexability IS NOT NULL
      AND t.id NOT IN (SELECT l.target_id FROM links l JOIN urls s ON s.id = l.source_id
        WHERE s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.target_id IS NOT NULL)
    ORDER BY u.id";

fn apply(
    conn: &Connection,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    let mut changed = BTreeSet::new();
    for (flag, selection, enabled) in [
        (
            FLAG_NON_INDEXABLE_CANONICAL,
            SELECT_FLAGGED,
            cfg.spider.canonicals.store,
        ),
        (
            "unlinked_canonical",
            SELECT_UNLINKED,
            cfg.spider.canonicals.store,
        ),
    ] {
        let Some(wanted) = collect(
            conn,
            if enabled {
                selection
            } else {
                "SELECT id FROM urls WHERE 0"
            },
            cancel,
        )?
        else {
            return Ok(StepState::Pending);
        };
        let Some(existing) = collect(
            conn,
            &format!("SELECT url_id FROM url_flags WHERE flag = '{flag}'"),
            cancel,
        )?
        else {
            return Ok(StepState::Pending);
        };
        let mut delete =
            conn.prepare_cached("DELETE FROM url_flags WHERE url_id = ?1 AND flag = ?2")?;
        let mut insert = conn.prepare_cached(
            "INSERT OR REPLACE INTO url_flags (url_id, flag, value) VALUES (?1, ?2, NULL)",
        )?;
        for id in existing.difference(&wanted) {
            if cancel.checkpoint().is_err() {
                return Ok(StepState::Pending);
            }
            delete.execute(params![id, flag])?;
            changed.insert(*id);
        }
        for id in wanted.difference(&existing) {
            if cancel.checkpoint().is_err() {
                return Ok(StepState::Pending);
            }
            insert.execute(params![id, flag])?;
            changed.insert(*id);
        }
    }
    if !changed.is_empty() {
        let seq = queries::change_seq(conn)? + 1;
        let mut stamp = conn.prepare_cached("UPDATE urls SET change_seq = ?1 WHERE id = ?2")?;
        for id in changed {
            if cancel.checkpoint().is_err() {
                return Ok(StepState::Pending);
            }
            stamp.execute(params![seq as i64, id])?;
        }
        conn.execute(
            "INSERT OR REPLACE INTO crawl_meta (key, value) VALUES ('change_seq', ?1)",
            params![seq.to_string()],
        )?;
    }
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    Ok(StepState::Done)
}
