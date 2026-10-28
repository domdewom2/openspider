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
    // Der teure Graphdurchlauf schreibt nur TEMP. Er darf die MAIN-Schreibsperre
    // weder für andere Analysatoren noch für deren Fortschrittszustände festhalten.
    struct Scratch<'a>(&'a rusqlite::Connection);
    impl Drop for Scratch<'_> {
        fn drop(&mut self) {
            let _ = self.0.execute_batch("DROP TABLE IF EXISTS temp.d08_internal; DROP TABLE IF EXISTS temp.d08_inlinks; DROP TABLE IF EXISTS temp.d08_link_facts; DROP TABLE IF EXISTS temp.d08_link_changes;");
        }
    }
    let _scratch = Scratch(conn);
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.d08_internal; DROP TABLE IF EXISTS temp.d08_inlinks;
        DROP TABLE IF EXISTS temp.d08_link_facts; DROP TABLE IF EXISTS temp.d08_link_changes;
        CREATE TEMP TABLE d08_internal(id INTEGER PRIMARY KEY, indexable INTEGER NOT NULL);
        INSERT INTO d08_internal SELECT id, COALESCE(indexability,'') <> 'Non-Indexable'
            FROM urls WHERE is_internal = 1;
        CREATE TEMP TABLE d08_inlinks AS
        SELECT l.target_id AS id, MAX(l.follow <> 0) AS yes, MAX(l.follow = 0) AS no,
            MAX(s.indexable) AS indexable
        FROM links l JOIN d08_internal s ON s.id = l.source_id JOIN d08_internal t ON t.id = l.target_id
        WHERE l.link_type = 'Hyperlink' AND l.crawlable = 1
            AND l.source_id <> l.target_id GROUP BY l.target_id;",
    )?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    conn.execute_batch("CREATE TEMP TABLE d08_link_facts (url_id INTEGER, key TEXT, v_int INTEGER, PRIMARY KEY(url_id,key)) WITHOUT ROWID;")?;
    for (key, condition) in [
        ("link.mixed_inlinks", "d.yes = 1 AND d.no = 1"),
        ("link.nofollow_only", "d.yes = 0 AND d.no = 1"),
        (
            "link.non_indexable_only",
            "d.indexable = 0 AND u.indexability = 'Indexable' AND u.discovered_via <> 'start'",
        ),
    ] {
        conn.execute(&format!("INSERT INTO d08_link_facts SELECT d.id, ?1, 1 FROM d08_inlinks d JOIN urls u ON u.id = d.id WHERE {condition}"), [key])?;
    }
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
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
    let handle = conn.get_interrupt_handle();
    let _registration = cancel.on_cancel(move || handle.interrupt());
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    match work() {
        Err(AnalyzeError::Sql(rusqlite::Error::SqliteFailure(e, _)))
            if e.code == rusqlite::ErrorCode::OperationInterrupted && cancel.is_cancelled() =>
        {
            Ok(StepState::Pending)
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: PERF-007
    #[test]
    fn cancel_hook_interrupts_an_active_sql_statement_and_releases_connection() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let cancel = CancelToken::new();
        let (started, ready) = std::sync::mpsc::channel();
        conn.create_scalar_function(
            "d08_started",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                started.send(()).unwrap();
                Ok(1)
            },
        )
        .unwrap();
        let result = std::thread::scope(|scope| {
            let worker_cancel = &cancel;
            scope.spawn(move || {
                ready
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                worker_cancel.cancel();
            });
            with_sql_cancel(&conn, &cancel, || {
                conn.query_row(
                    "WITH RECURSIVE numbers(x) AS (SELECT d08_started()
                    UNION ALL SELECT x+1 FROM numbers WHERE x<100000000)
                    SELECT sum(x) FROM numbers",
                    [],
                    |r| r.get::<_, i64>(0),
                )?;
                Ok(StepState::Done)
            })
        })
        .unwrap();
        assert_eq!(result, StepState::Pending);
        assert_eq!(
            conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
