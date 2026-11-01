//! Lesen-dann-Schreiben-Transaktionen neben der WAL-Rückführung (CRAWL-126, OS-Festlegung vom 01.11.2026): Die
//! Rückführung hält die Schreibsperre bis zu 100 ms je Versuch, solange ein Leser einen älteren Stand hält. Eine
//! verzögerte Transaktion bekäme beim Hochstufen auf Schreiben sofort `SQLITE_BUSY` (der Busy-Timeout wirkt dort
//! nicht); mit `BEGIN IMMEDIATE` wartet sie im Busy-Timeout auf die Sperre.

mod store_common;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use os_store::open::{merge_wal, wal_bytes};
use os_store::queries::{api, detail_ext, serp::SerpEdit};
use os_store::{CrawlSink, StorageProfile};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use store_common::*;

/// So lange hält die zweite Verbindung die Schreibsperre.
const HOLD: Duration = Duration::from_millis(200);

/// Crawl-Datei mit zwei URLs und GA4-Fakten für URL 1.
fn crawl_with_api_values(dir: &Path) -> PathBuf {
    let path = new_crawl(dir, "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    drop(sink);
    writer(&path)
        .execute(
            "INSERT INTO api_values(url_id,source,metric,v_real,fetched_at) \
             VALUES(1,'ga4','sessions',3,'2026-11-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
    path
}

/// Schreibverbindung wie die Schreibwarteschlange der App (Busy-Timeout 5 s).
fn writer(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn
}

/// Eine zweite Verbindung nimmt die Schreibsperre (wie die WAL-Rückführung, während sie auf Leser wartet) und gibt
/// sie nach [`HOLD`] wieder frei; kehrt zurück, sobald die Sperre gehalten wird.
fn hold_write_lock(path: &Path) -> std::thread::JoinHandle<()> {
    let (tx, rx) = mpsc::channel();
    let path = path.to_path_buf();
    let handle = std::thread::spawn(move || {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        tx.send(()).unwrap();
        std::thread::sleep(HOLD);
        conn.execute_batch("ROLLBACK").unwrap();
    });
    rx.recv().unwrap();
    handle
}

/// Ein Leser, der einen Snapshot hält, bis er fallen gelassen wird.
fn reader(path: &Path) -> Connection {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.execute_batch("BEGIN").unwrap();
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    conn
}

fn detail(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT detail FROM api_status WHERE source='ga4'",
        [],
        |r| r.get::<_, Option<String>>(0),
    )
    .optional()
    .unwrap()
    .flatten()
}

// spec: CRAWL-126
// spec: INTEG-010
#[test]
fn invalidate_waits_for_a_held_write_lock_instead_of_failing_busy() {
    for remove in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = crawl_with_api_values(dir.path());
        let conn = writer(&path);
        let lock = hold_write_lock(&path);
        let t = Instant::now();
        let result = api::invalidate(&conn, "ga4", remove);
        let waited = t.elapsed();
        lock.join().unwrap();
        assert!(
            matches!(result, Ok(true)),
            "remove={remove}: {result:?} after {waited:?}"
        );
        assert!(
            waited >= HOLD / 2,
            "remove={remove}: waited only {waited:?} for the lock"
        );
        let values: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_values WHERE source='ga4'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        if remove {
            assert_eq!(values, 0);
            assert_eq!(detail(&conn), None);
        } else {
            assert_eq!(values, 1);
            assert_eq!(
                detail(&conn).as_deref(),
                Some("Daten stammen aus einer früheren Konfiguration")
            );
        }
    }
}

// spec: CRAWL-126
// spec: INTEG-010
#[test]
fn invalidate_joins_a_transaction_the_caller_already_opened() {
    let dir = tempfile::tempdir().unwrap();
    let path = crawl_with_api_values(dir.path());
    let conn = writer(&path);
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    conn.execute("UPDATE crawl_meta SET value='renamed' WHERE key='name'", [])
        .unwrap();
    assert!(api::invalidate(&conn, "ga4", false).unwrap());
    assert!(!conn.is_autocommit(), "the caller's transaction stays open");
    conn.execute_batch("ROLLBACK").unwrap();
    assert_eq!(detail(&conn), None, "rolled back together with the caller");
}

// spec: CRAWL-126
// spec: INTEG-010
#[test]
fn invalidate_succeeds_while_the_wal_merge_waits_for_a_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = crawl_with_api_values(dir.path());
    let conn = writer(&path);
    let held = reader(&path);
    // Frames nach dem Snapshot des Lesers: die Rückführung kann erst nach dessen Ende abschließen.
    conn.execute("UPDATE crawl_meta SET value='paused' WHERE key='name'", [])
        .unwrap();
    assert!(wal_bytes(&path) > 0);
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let merge = scope.spawn(|| merge_wal(&path, &cancel).unwrap());
        std::thread::sleep(Duration::from_millis(20));
        for attempt in 0..10 {
            let result = api::invalidate(&conn, "ga4", false);
            assert!(
                matches!(result, Ok(true)),
                "attempt {attempt} during the merge: {result:?}"
            );
            std::thread::sleep(Duration::from_millis(15));
        }
        assert!(!merge.is_finished(), "the merge waits for the reader");
        drop(held);
        assert!(merge.join().unwrap(), "merged after the reader ended");
    });
    assert_eq!(wal_bytes(&path), 0);
}

// spec: CRAWL-126
// spec: UI-037
#[test]
fn serp_edit_waits_for_a_held_write_lock_instead_of_failing_busy() {
    let dir = tempfile::tempdir().unwrap();
    let path = crawl_with_api_values(dir.path());
    let mut conn = writer(&path);
    let edit = SerpEdit {
        device: "desktop".into(),
        title: Some("Neuer Titel".into()),
        description: None,
        edited_at: "2026-11-01T00:00:00.000Z".into(),
    };
    let lock = hold_write_lock(&path);
    let t = Instant::now();
    let result = detail_ext::save_serp_edit(&mut conn, 1, Some(&edit));
    let waited = t.elapsed();
    lock.join().unwrap();
    assert!(matches!(result, Ok(true)), "{result:?} after {waited:?}");
    assert!(waited >= HOLD / 2, "waited only {waited:?} for the lock");
}
