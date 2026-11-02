//! Abschließender Checkpoint der Crawl-Analyse neben einem Leser, der einen älteren Stand hält (CRAWL-141
//! Ablaufpunkt 7, OS-Festlegung vom 01.11.2026): Er wartet höchstens 100 ms (Wanduhr) auf den Leser; die Analyse ist
//! mit ihrem letzten dauerhaften Commit abgeschlossen, die verbleibenden Frames führt die App wie nach einer Pause im
//! Hintergrund zurück (CRAWL-126).
#[path = "aggregates_fixture.rs"]
mod fixture;

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use os_analyze::analysis::runner::{self, RunOptions, StepFn};
use os_analyze::analysis::CancelToken;
use os_model::{AnalysisStep, CrawlConfig, StepState};
use os_store::open::{merge_wal, wal_bytes};
use rusqlite::{Connection, OpenFlags};

/// Ein Teilschritt ohne eigene Arbeit: gemessen wird der Rahmen des Laufs mit seinem abschließenden Checkpoint.
fn quick_step(_: AnalysisStep) -> StepFn {
    |_, _, _| Ok(StepState::Done)
}

// spec: CRAWL-141
// spec: CRAWL-126
#[test]
fn final_checkpoint_waits_at_most_briefly_for_a_held_reader() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::create(dir.path());
    fixture::insert(
        &file,
        1,
        "https://site.test/",
        Some(0),
        true,
        true,
        Some(200),
        Some(10),
    );
    file.conn().busy_timeout(Duration::from_secs(5)).unwrap();
    assert!(wal_bytes(file.path()) > 0);

    // Ein fremder Leser (etwa eine CLI-Abfrage) hält einen Stand aus dem WAL.
    let reader =
        Connection::open_with_flags(file.path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    let _: i64 = reader
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();

    let t = Instant::now();
    let outcome = runner::run_with(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
        &|_, _| {},
        &RunOptions {
            only: Some(&[AnalysisStep::Links]),
            registry: quick_step,
            max_parallel: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let took = t.elapsed();
    assert!(outcome.complete);
    // Vorher wartete der Checkpoint mit einem Busy-Timeout von 5 s auf den Leser.
    assert!(
        took < Duration::from_secs(2),
        "the analysis waited {took:?} for the reader"
    );
    assert!(
        wal_bytes(file.path()) > 0,
        "the reader keeps the analysis frames in the WAL"
    );
    let timeout: i64 = file
        .conn()
        .pragma_query_value(None, "busy_timeout", |r| r.get(0))
        .unwrap();
    assert_eq!(timeout, 5_000, "the connection's busy timeout is restored");
    let state: String = file
        .conn()
        .query_row(
            "SELECT value FROM crawl_meta WHERE key='analysis_state'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // Nur ein Teilschritt lief: Der Gesamtzustand ist „unvollständig“, aber nicht mehr „läuft“.
    assert_ne!(state, "running", "the final run state is committed");
    let links: String = file
        .conn()
        .query_row(
            "SELECT state FROM analysis_steps WHERE step='links'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(links, "done", "the step result is committed");

    // Nach dem Ende des Lesers führt die Hintergrund-Rückführung den Rest zurück.
    drop(reader);
    assert!(merge_wal(file.path(), &AtomicBool::new(false)).unwrap());
    assert_eq!(wal_bytes(file.path()), 0);
}
