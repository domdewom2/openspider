//! Bildanalyse (ONPAGE-178): globale Hintergrundklassifikation, atomar und wiederholbar.
use crate::{analysis::CancelToken, AnalyzeError};
use os_model::{CrawlConfig, RenderingMode, StepState};
use os_store::{queries, CrawlFile};

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
    let tx = file.conn().unchecked_transaction()?;
    // Nicht korreliert: alle img/srcset-Ziele einmal lesen, nicht je Hintergrundinstanz.
    tx.execute_batch("CREATE TEMP TABLE d03_background(id INTEGER PRIMARY KEY);
      INSERT INTO d03_background SELECT DISTINCT image_id FROM image_instances
        WHERE image_id IS NOT NULL AND (source='css' OR origin='Dynamically Loaded')
        AND image_id NOT IN (SELECT image_id FROM image_instances WHERE source IN ('img','srcset') AND image_id IS NOT NULL);
      CREATE TEMP TABLE d03_changed(id INTEGER PRIMARY KEY);
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
    tx.execute_batch("DROP TABLE d03_changed; DROP TABLE d03_background;")?;
    if cancel.checkpoint().is_err() {
        return Ok(StepState::Pending);
    }
    tx.commit()?;
    cancel.set_progress(1.0);
    Ok(StepState::Done)
}
