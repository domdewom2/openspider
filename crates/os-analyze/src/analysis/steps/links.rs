//! Atomic, repeatable incoming-link facts for TECH-139/140/142.
use crate::{analysis::CancelToken, error::AnalyzeError};
use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

pub fn run(
    file: &CrawlFile,
    _: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    with_sql_cancel(file.conn(), cancel, || run_inner(file, cancel))
}

fn run_inner(file: &CrawlFile, cancel: &CancelToken) -> Result<StepState, AnalyzeError> {
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let conn = file.conn();
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "DROP TABLE IF EXISTS temp.d08_inlinks;
        CREATE TEMP TABLE d08_inlinks AS
        SELECT l.target_id AS id, MAX(l.follow <> 0) AS yes, MAX(l.follow = 0) AS no,
            MAX(COALESCE(s.indexability, '') <> 'Non-Indexable') AS indexable
        FROM links l JOIN urls s ON s.id = l.source_id JOIN urls t ON t.id = l.target_id
        WHERE s.is_internal = 1 AND t.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1
            AND l.source_id <> l.target_id GROUP BY l.target_id;",
    )?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    tx.execute_batch("CREATE TEMP TABLE d08_link_facts (url_id INTEGER, key TEXT, v_int INTEGER, PRIMARY KEY(url_id,key)) WITHOUT ROWID;")?;
    for (key, condition) in [
        ("link.mixed_inlinks", "d.yes = 1 AND d.no = 1"),
        ("link.nofollow_only", "d.yes = 0 AND d.no = 1"),
        ("link.non_indexable_only", "d.indexable = 0 AND u.indexability = 'Indexable' AND u.address <> COALESCE((SELECT value FROM crawl_meta WHERE key = 'start_url'), '')"),
    ] {
        tx.execute(&format!("INSERT INTO d08_link_facts SELECT d.id, ?1, 1 FROM d08_inlinks d JOIN urls u ON u.id = d.id WHERE {condition}"), [key])?;
    }
    tx.execute_batch("CREATE TEMP TABLE d08_link_changes AS
        SELECT url_id FROM (SELECT url_id,key,v_int FROM url_ext WHERE key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only') EXCEPT SELECT * FROM d08_link_facts)
        UNION SELECT url_id FROM (SELECT * FROM d08_link_facts EXCEPT SELECT url_id,key,v_int FROM url_ext WHERE key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only'));")?;
    let changed: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM d08_link_changes)", [], |r| {
        r.get(0)
    })?;
    if changed {
        let seq = os_store::queries::change_seq(conn)? + 1;
        tx.execute(
            "UPDATE urls SET change_seq = ?1 WHERE id IN (SELECT url_id FROM d08_link_changes)",
            [seq],
        )?;
        tx.execute_batch("DELETE FROM url_ext WHERE key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only') AND url_id IN (SELECT url_id FROM d08_link_changes)")?;
        tx.execute("INSERT INTO url_ext(url_id,key,v_int,change_seq) SELECT url_id,key,v_int,?1 FROM d08_link_facts WHERE url_id IN (SELECT url_id FROM d08_link_changes)", [seq])?;
        tx.execute(
            "INSERT OR REPLACE INTO crawl_meta(key,value) VALUES('change_seq',?1)",
            [seq.to_string()],
        )?;
    }
    tx.execute_batch(
        "DROP TABLE d08_inlinks; DROP TABLE d08_link_facts; DROP TABLE d08_link_changes;",
    )?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    tx.commit()?;
    Ok(StepState::Done)
}

/// Interrupt long SQLite grouping/sorting statements as well as the Rust loops.
/// Only the interrupt handle crosses threads; the connection stays on its owner thread.
pub(super) fn with_sql_cancel(
    conn: &rusqlite::Connection,
    cancel: &CancelToken,
    work: impl FnOnce() -> Result<StepState, AnalyzeError>,
) -> Result<StepState, AnalyzeError> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let finished = AtomicBool::new(false);
    struct Finish<'a>(&'a AtomicBool);
    impl Drop for Finish<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    std::thread::scope(|scope| {
        let handle = conn.get_interrupt_handle();
        let done = &finished;
        scope.spawn(move || {
            while !done.load(Ordering::Acquire) {
                if cancel.is_cancelled() {
                    handle.interrupt();
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });
        let _finish = Finish(&finished);
        match work() {
            Err(AnalyzeError::Sql(rusqlite::Error::SqliteFailure(e, _)))
                if e.code == rusqlite::ErrorCode::OperationInterrupted && cancel.is_cancelled() =>
            {
                Ok(StepState::Pending)
            }
            result => result,
        }
    })
}
