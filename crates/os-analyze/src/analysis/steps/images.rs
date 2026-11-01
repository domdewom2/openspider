//! Bildanalyse (ONPAGE-178): globale Hintergrundklassifikation, atomar und wiederholbar.
use crate::{analysis::CancelToken, AnalyzeError};
use os_model::{CrawlConfig, RenderingMode, StepState};
use os_store::{queries, CrawlFile};
use rusqlite::{Transaction, TransactionBehavior};

/// Entfernt die TEMP-Tabellen des Schritts auch bei Abbruch oder Fehler (nach dem Rollback der Transaktion).
struct Scratch<'a>(&'a rusqlite::Connection);
impl Drop for Scratch<'_> {
    fn drop(&mut self) {
        let _ = self.0.execute_batch(
            "DROP TABLE IF EXISTS temp.d03_changed; DROP TABLE IF EXISTS temp.d03_background;",
        );
    }
}

pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    if cfg.rendering.mode != RenderingMode::JavaScript && !cfg.images.parse_inline_css_backgrounds {
        return Ok(StepState::NotApplicable);
    }
    let conn = file.conn();
    let _scratch = Scratch(conn);
    // Sammeln ohne Schreibsperre: `image_instances` sind Crawl-Daten, die kein Analyseschritt ändert. Die
    // Leseanweisung gibt ihren Snapshot danach wieder frei (Autocommit).
    // Nicht korreliert: alle img/srcset-Ziele einmal lesen, nicht je Hintergrundinstanz.
    conn.execute_batch("DROP TABLE IF EXISTS temp.d03_background;
      DROP TABLE IF EXISTS temp.d03_changed;
      CREATE TEMP TABLE d03_background(id INTEGER PRIMARY KEY);
      INSERT INTO d03_background SELECT DISTINCT image_id FROM image_instances
        WHERE image_id IS NOT NULL AND (source='css' OR origin='Dynamically Loaded')
        AND image_id NOT IN (SELECT image_id FROM image_instances WHERE source IN ('img','srcset') AND image_id IS NOT NULL);")?;
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    // Vergleich und Veröffentlichung unter BEGIN IMMEDIATE: Eine verzögerte Transaktion, die erst `url_flags` liest
    // und dann schreibt, scheitert beim Hochstufen sofort mit SQLITE_BUSY (Busy-Timeout wirkt dort nicht), solange
    // die WAL-Rückführung nach Pause/Ende (CRAWL-126) die Schreibsperre hält, und mit SQLITE_BUSY_SNAPSHOT, wenn ein
    // paralleler Teilschritt inzwischen veröffentlicht hat (CRAWL-141).
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    tx.execute_batch("CREATE TEMP TABLE d03_changed(id INTEGER PRIMARY KEY);
      INSERT INTO d03_changed SELECT id FROM d03_background WHERE id NOT IN (SELECT url_id FROM url_flags WHERE flag='image_background')
        UNION SELECT url_id FROM url_flags WHERE flag='image_background' AND url_id NOT IN (SELECT id FROM d03_background);")?;
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    let changed: bool =
        tx.query_row("SELECT EXISTS(SELECT 1 FROM d03_changed)", [], |r| r.get(0))?;
    if changed {
        tx.execute_batch("DELETE FROM url_flags WHERE flag='image_background' AND url_id IN (SELECT id FROM d03_changed);
          INSERT INTO url_flags(url_id,flag) SELECT id,'image_background' FROM d03_background WHERE id IN (SELECT id FROM d03_changed);")?;
        let seq = queries::change_seq(&tx)? + 1;
        tx.execute(
            "UPDATE urls SET change_seq=?1 WHERE id IN (SELECT id FROM d03_changed)",
            [seq],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO crawl_meta(key,value) VALUES ('change_seq',?1)",
            [seq.to_string()],
        )?;
    }
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    tx.commit()?;
    cancel.set_progress(1.0);
    Ok(StepState::Done)
}
