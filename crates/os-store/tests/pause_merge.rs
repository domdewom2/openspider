//! Abschluss-Checkpoint mit begrenzter Wartezeit und Hintergrund-Rückführung des WAL (CRAWL-126, OS-Festlegung vom
//! 01.11.2026): Hält ein Leser einen älteren Stand, kehrt der Checkpoint der Senke nach kurzer Zeit zurück; nach dem
//! Ende des Lesers führt `merge_wal` das WAL vollständig zurück.

mod store_common;

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use os_store::open::{merge_wal, wal_bytes};
use os_store::{CrawlSink, StorageProfile};
use rusqlite::{Connection, OpenFlags};
use store_common::*;

fn reader(path: &std::path::Path) -> Connection {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.execute_batch("BEGIN").unwrap();
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    conn
}

// spec: CRAWL-126
#[test]
fn a_reader_holding_a_snapshot_delays_the_final_checkpoint_only_briefly() {
    for profile in [StorageProfile::Balanced, StorageProfile::MaxSpeed] {
        let dir = tempfile::tempdir().unwrap();
        let path = new_crawl(dir.path(), "a.ospider", profile);
        let mut sink = sink(&path, profile);
        sink.write(batch_with_urls([(1, true)])).unwrap();
        let held = reader(&path);
        sink.durable().unwrap();
        sink.write(batch_with_urls([(2, true), (3, false)]))
            .unwrap();
        let t = Instant::now();
        sink.checkpoint().unwrap();
        let waited = t.elapsed();
        assert!(
            waited <= Duration::from_millis(300),
            "{profile:?}: checkpoint waited {waited:?} for a reader"
        );
        assert!(wal_bytes(&path) > 0, "{profile:?}: frames stay in the WAL");

        // the merge does not succeed while the reader holds its snapshot
        let cancel = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let merge = scope.spawn(|| merge_wal(&path, &cancel).unwrap());
            std::thread::sleep(Duration::from_millis(250));
            assert!(
                !merge.is_finished(),
                "{profile:?}: merge waits for the reader"
            );
            drop(held);
            assert!(
                merge.join().unwrap(),
                "{profile:?}: merged after the reader ended"
            );
        });
        assert_eq!(
            wal_bytes(&path),
            0,
            "{profile:?}: WAL is empty after the merge"
        );
        let count: i64 = sink
            .file()
            .conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }
}

// spec: CRAWL-126
#[test]
fn a_cancelled_merge_returns_without_merging() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let held = reader(&path);
    sink.write(batch_with_urls([(2, true)])).unwrap();
    sink.checkpoint().unwrap();
    let cancel = AtomicBool::new(true);
    let t = Instant::now();
    assert!(!merge_wal(&path, &cancel).unwrap());
    assert!(t.elapsed() < Duration::from_millis(300));
    assert!(wal_bytes(&path) > 0);
    drop(held);
}
