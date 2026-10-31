//! Atomare, wiederholbare Inlink-Fakten für TECH-139/140/142.
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
    // Ein sequenzieller Graphdurchlauf vermeidet zufällige Tabellenzugriffe
    // über den Zielindex außerhalb des SQLite-Seitencaches. Fünf Zustandsbits
    // je interner URL ersetzen die vollständige Inlink-Gruppierung.
    const INDEXABLE: u8 = 1;
    const ELIGIBLE: u8 = 2;
    const FOLLOW: u8 = 4;
    const NOFOLLOW: u8 = 8;
    const INDEXABLE_SOURCE: u8 = 16;
    let mut pages = std::collections::HashMap::<i64, u8>::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(indexability,'') <> 'Non-Indexable',
            COALESCE(indexability = 'Indexable' AND discovered_via <> 'start',0)
            FROM urls WHERE is_internal = 1",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let flags = (u8::from(row.get::<_, bool>(1)?) * INDEXABLE)
                | (u8::from(row.get::<_, bool>(2)?) * ELIGIBLE);
            pages.insert(row.get(0)?, flags);
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT source_id,target_id,follow <> 0 FROM links NOT INDEXED
            WHERE link_type = 'Hyperlink' AND crawlable = 1 AND source_id <> target_id",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let source: i64 = row.get(0)?;
            let target: i64 = row.get(1)?;
            if let Some(&source_flags) = pages.get(&source) {
                if let Some(flags) = pages.get_mut(&target) {
                    *flags |= if row.get::<_, bool>(2)? {
                        FOLLOW
                    } else {
                        NOFOLLOW
                    };
                    if source_flags & INDEXABLE != 0 {
                        *flags |= INDEXABLE_SOURCE;
                    }
                }
            }
        }
    }
    struct Scratch<'a>(&'a rusqlite::Connection);
    impl Drop for Scratch<'_> {
        fn drop(&mut self) {
            let _ = self.0.execute_batch(
                "DROP TABLE IF EXISTS temp.d08_link_facts;
                DROP TABLE IF EXISTS temp.d08_old_link_facts;
                DROP TABLE IF EXISTS temp.d08_link_changes;",
            );
        }
    }
    let _scratch = Scratch(conn);
    conn.execute_batch(
        "CREATE TEMP TABLE d08_link_facts (url_id INTEGER, key TEXT, v_int INTEGER,
        PRIMARY KEY(url_id,key)) WITHOUT ROWID;",
    )?;
    let scratch_tx = conn.unchecked_transaction()?;
    {
        let mut insert = scratch_tx.prepare("INSERT INTO d08_link_facts VALUES(?1,?2,1)")?;
        for (id, flags) in pages {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            for (key, present) in [
                (
                    "link.mixed_inlinks",
                    flags & (FOLLOW | NOFOLLOW) == (FOLLOW | NOFOLLOW),
                ),
                (
                    "link.nofollow_only",
                    flags & (FOLLOW | NOFOLLOW) == NOFOLLOW,
                ),
                (
                    "link.non_indexable_only",
                    flags & (FOLLOW | NOFOLLOW) != 0
                        && flags & INDEXABLE_SOURCE == 0
                        && flags & ELIGIBLE != 0,
                ),
            ] {
                if present {
                    insert.execute(rusqlite::params![id, key])?;
                }
            }
        }
    }
    scratch_tx.commit()?;
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    // Numerische Fakten benötigen keinen Text-Overflow-Join. Der sequenzielle
    // Primärindex-Durchlauf vermeidet wiederholte Scans über change_seq.
    // Copy-Swap-Migrationen von url_ext_data müssen den Autoindex-Namen
    // sqlite_autoindex_url_ext_data_1 prüfen (K2-99).
    tx.execute_batch(
        "CREATE TEMP TABLE d08_old_link_facts(url_id INTEGER, key TEXT, v_int INTEGER)",
    )?;
    let have_old_keys: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM ext_keys WHERE key IN
        ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only'))",
        [],
        |r| r.get(0),
    )?;
    if have_old_keys {
        tx.execute(
            "INSERT INTO d08_old_link_facts SELECT e.url_id,k.key,e.v_int FROM url_ext_data e INDEXED BY sqlite_autoindex_url_ext_data_1 CROSS JOIN ext_keys k ON k.id=e.key_id WHERE e.key_id IN (SELECT id FROM ext_keys WHERE key IN ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only'))",
            [],
        )?;
    }
    tx.execute_batch("CREATE TEMP TABLE d08_link_changes(url_id INTEGER PRIMARY KEY);
        INSERT INTO d08_link_changes
        SELECT url_id FROM (SELECT * FROM d08_old_link_facts EXCEPT SELECT * FROM d08_link_facts)
        UNION SELECT url_id FROM (SELECT * FROM d08_link_facts EXCEPT SELECT * FROM d08_old_link_facts);")?;
    let changed: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM d08_link_changes)", [], |r| {
        r.get(0)
    })?;
    if changed {
        let seq = os_store::queries::change_seq(conn)? + 1;
        tx.execute(
            "UPDATE urls SET change_seq = ?1 WHERE id IN (SELECT url_id FROM d08_link_changes)",
            [seq],
        )?;
        {
            let mut changed = tx.prepare("SELECT url_id FROM d08_link_changes ORDER BY url_id")?;
            let mut rows = changed.query([])?;
            let mut delete = tx.prepare(
                "DELETE FROM url_ext WHERE url_id=?1 AND key IN
                ('link.mixed_inlinks','link.nofollow_only','link.non_indexable_only')",
            )?;
            while let Some(row) = rows.next()? {
                if cancel.is_cancelled() {
                    return Ok(StepState::Pending);
                }
                delete.execute([row.get::<_, i64>(0)?])?;
            }
        }
        tx.execute("INSERT INTO url_ext(url_id,key,v_int,change_seq) SELECT url_id,key,v_int,?1 FROM d08_link_facts WHERE url_id IN (SELECT url_id FROM d08_link_changes)", [seq])?;
        tx.execute(
            "INSERT OR REPLACE INTO crawl_meta(key,value) VALUES('change_seq',?1)",
            [seq.to_string()],
        )?;
    }
    tx.execute_batch(
        "DROP TABLE d08_old_link_facts; DROP TABLE d08_link_facts; DROP TABLE d08_link_changes;",
    )?;
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    tx.commit()?;
    Ok(StepState::Done)
}

/// Unterbricht lange SQLite-Anweisungen ebenso wie die Rust-Schleifen.
/// Nur der Interrupt-Handle wechselt den Thread; die Verbindung bleibt beim Besitzer.
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
