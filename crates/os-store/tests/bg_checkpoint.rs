//! Optionaler Hintergrund-Checkpoint des Writers (K-76, M1-P32, `SqliteSink::with_background_checkpoint`):
//! `wal_autocheckpoint = 0` auf der Writer-Verbindung, ein Thread mit eigener Verbindung führt `PASSIVE`-Checkpoints aus,
//! der Writer wartet nie darauf. Abschluss und Pause führen weiter den kurzen `TRUNCATE`-Checkpoint aus; ein von außen
//! gesetzter `wal_autocheckpoint` bleibt unangetastet. Standard ist der Checkpoint im Commit (letzter Test).

mod store_common;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use os_store::sink::diag;
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

/// Die Messzähler sind prozessweit: alle Tests dieser Datei laufen nacheinander.
static DIAG: Mutex<()> = Mutex::new(());

fn wal_len(path: &std::path::Path) -> u64 {
    std::fs::metadata(format!("{}-wal", path.display())).map_or(0, |m| m.len())
}

fn counter(name: &str) -> u64 {
    diag::snapshot()
        .into_iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
        .unwrap()
}

/// Lässt die Datei um rund `mib` MiB WAL wachsen (eine Transaktion mit Füllzeilen).
fn grow_wal(sink: &os_store::SqliteSink, mib: usize) {
    let conn = sink.file().conn();
    conn.execute_batch("CREATE TABLE IF NOT EXISTS filler (a BLOB)")
        .unwrap();
    let tx = conn.unchecked_transaction().unwrap();
    for _ in 0..(mib * 256) {
        tx.execute("INSERT INTO filler VALUES (zeroblob(4000))", [])
            .unwrap();
    }
    tx.commit().unwrap();
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < Duration::from_secs(20), "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

// spec: CRAWL-125
#[test]
fn the_writer_connection_has_no_automatic_checkpoint_and_a_thread_starts_with_the_first_write() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_background_checkpoint()
        .unwrap();
    let auto: i64 = sink
        .file()
        .conn()
        .pragma_query_value(None, "wal_autocheckpoint", |r| r.get(0))
        .unwrap();
    assert_eq!(auto, 0, "kein Checkpoint im Commit des Writers");
    assert!(!sink.background_checkpoint_running());
    sink.write(batch_with_urls([(1, true)])).unwrap();
    assert!(sink.background_checkpoint_running());
    assert_eq!(
        sink.checkpoint_trigger_pages(),
        os_store::profile::WAL_CHECKPOINT_MIN_PAGES
    );
    assert!(sink.background_checkpoint_error().is_none());
}

// spec: PERF-003
#[test]
fn the_background_checkpoint_runs_without_errors_and_the_writer_never_waits_for_it() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "b.ospider", StorageProfile::Balanced);
    // Auslöser bei 2 000 Frames (8 MiB); insgesamt wandern rund 200 MiB durch das WAL.
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_background_checkpoint()
        .unwrap()
        .with_checkpoint_bounds(2_000, 2_000)
        .unwrap();
    diag::reset();
    for i in 0..12 {
        sink.write(batch_with_urls([(i64::from(i) + 1, true)]))
            .unwrap();
        grow_wal(&sink, 16);
        // dem Thread eine Prüfperiode (100 ms) lassen
        std::thread::sleep(Duration::from_millis(250));
    }
    // Ob das WAL dabei schrumpft, hängt vom Zeitpunkt ab (SQLite setzt es nur zurück, wenn der nächste Schreibvorgang nach
    // dem Ende eines Durchlaufs beginnt; auf Windows-Runnern blieb es bei 194 MiB): geprüft werden nur die Zähler.
    assert!(
        counter("bg_checkpoints") >= 3,
        "Durchläufe: {}",
        counter("bg_checkpoints")
    );
    assert_eq!(counter("bg_checkpoint_errors"), 0);
    assert_eq!(counter("wait_checkpoint_n"), 0, "der Writer wartet nie");
    assert_eq!(counter("wait_checkpoint_ns"), 0);
    assert_eq!(
        counter("checkpoints"),
        0,
        "kein ausdrücklicher Checkpoint während des Schreibens"
    );
    assert!(sink.background_checkpoint_error().is_none());
    // Abschluss: kurzer TRUNCATE, das WAL ist leer
    sink.finish().unwrap();
    assert_eq!(wal_len(&path), 0);
    assert!(
        !sink.background_checkpoint_running(),
        "finish() hält den Thread an"
    );
    assert!(CrawlFile::open(&path, OpenMode::ReadOnly)
        .unwrap()
        .integrity_check()
        .unwrap());
}

// spec: CRAWL-125
#[test]
fn pause_checkpoint_stops_the_thread_and_the_next_write_restarts_it() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "c.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_background_checkpoint()
        .unwrap();
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    assert!(sink.background_checkpoint_running());
    sink.checkpoint().unwrap();
    assert!(!sink.background_checkpoint_running());
    assert_eq!(wal_len(&path), 0, "TRUNCATE leert das WAL");
    sink.write(batch_with_urls([(3, true)])).unwrap();
    assert!(sink.background_checkpoint_running());
    // alle Zeilen sind da
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 3);
}

// spec: CRAWL-125
#[test]
fn an_externally_set_threshold_is_left_alone_and_no_thread_runs() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "d.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_background_checkpoint()
        .unwrap();
    sink.file()
        .conn()
        .pragma_update(None, "wal_autocheckpoint", 5_000)
        .unwrap();
    sink.write(batch_with_urls([(1, true)])).unwrap();
    sink.write(batch_with_urls([(2, true)])).unwrap();
    let auto: i64 = sink
        .file()
        .conn()
        .pragma_query_value(None, "wal_autocheckpoint", |r| r.get(0))
        .unwrap();
    assert_eq!(auto, 5_000, "der Sink stellt den Wert nicht um");
    assert!(!sink.background_checkpoint_running());
    // Der Abschluss-Checkpoint bleibt.
    sink.finish().unwrap();
    assert_eq!(wal_len(&path), 0);
}

// spec: CRAWL-125
#[test]
fn the_trigger_starts_at_256_mib_and_grows_with_the_file_within_its_bounds() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    use os_store::profile::WAL_CHECKPOINT_MIN_PAGES;
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "e.ospider", StorageProfile::Balanced);
    let mut sink0 = sink(&path, StorageProfile::Balanced);
    assert_eq!(sink0.checkpoint_trigger_pages(), WAL_CHECKPOINT_MIN_PAGES);
    assert_eq!(WAL_CHECKPOINT_MIN_PAGES * 4096, 256 * 1024 * 1024);
    sink0.write(batch_with_urls([(1, true)])).unwrap();
    assert_eq!(sink0.checkpoint_trigger_pages(), WAL_CHECKPOINT_MIN_PAGES);
    drop(sink0);

    // Mit kleinen Grenzen (2 000 bis 6 000 Seiten) lässt sich das Mitwachsen mit einer kleinen Datei prüfen.
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_background_checkpoint()
        .unwrap()
        .with_checkpoint_bounds(2_000, 6_000)
        .unwrap();
    assert_eq!(sink.checkpoint_trigger_pages(), 2_000);
    grow_wal(&sink, 16); // rund 4 000 Seiten
    sink.write(batch_with_urls([(2, true)])).unwrap();
    wait_until("Auslöser wächst mit der Datei", || {
        (4_000..=6_000).contains(&sink.checkpoint_trigger_pages())
    });
    // Über der Obergrenze bleibt er dort.
    grow_wal(&sink, 16);
    wait_until("Auslöser an der Obergrenze", || {
        sink.checkpoint_trigger_pages() == 6_000
    });
}

// spec: CRAWL-125
#[test]
fn by_default_sqlite_checkpoints_in_the_commit_and_no_thread_runs() {
    let _serial = DIAG.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "f.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let auto: i64 = sink
        .file()
        .conn()
        .pragma_query_value(None, "wal_autocheckpoint", |r| r.get(0))
        .unwrap();
    assert_eq!(auto, os_store::profile::WAL_CHECKPOINT_MIN_PAGES);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    assert!(!sink.background_checkpoint_running());
    sink.finish().unwrap();
    assert_eq!(wal_len(&path), 0);
}
