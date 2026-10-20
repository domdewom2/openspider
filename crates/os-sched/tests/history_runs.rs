//! Laufhistorie (OUT-169), Überlappung (OUT-167) und Abbruchmarke (OUT-171).

use os_model::Timestamp;
use os_sched::history::{HistoryStore, RunRecord, RunResult, KEEP_RUNS, STALE_AFTER_SECS};

fn at(utc: &str) -> Timestamp {
    Timestamp::from_db(utc).unwrap()
}

fn record(run: &str, task: &str, started: &str) -> RunRecord {
    RunRecord::start(run, task, "Nightly", at(started))
}

fn finished(run: &str, task: &str, started: &str, result: RunResult) -> RunRecord {
    let mut r = record(run, task, started);
    r.finish(result, Some(0), at(started));
    r
}

// spec: OUT-169
#[test]
fn runs_are_listed_newest_first_with_all_columns() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path());
    let mut a = finished(
        "run-a",
        "nightly",
        "2026-10-18T00:30:00.000Z",
        RunResult::Completed,
    );
    a.crawl_id = Some("crawl-1".into());
    a.urls_crawled = Some(1234);
    a.output_folder = Some("out/2026-10-18_02-30-00".into());
    a.exports = vec!["out/internal_all.csv".into()];
    a.log_path = Some("logs/task-nightly-20261018-003000.log".into());
    store.write(&a).unwrap();
    store
        .write(&finished(
            "run-b",
            "nightly",
            "2026-10-19T00:30:00.000Z",
            RunResult::CompletedWithWarnings,
        ))
        .unwrap();
    store
        .write(&finished(
            "run-c",
            "other",
            "2026-10-20T00:30:00.000Z",
            RunResult::Failed,
        ))
        .unwrap();
    let ids: Vec<String> = store
        .list(Some("nightly"), 0, 10)
        .into_iter()
        .map(|r| r.run_id)
        .collect();
    assert_eq!(ids, ["run-b", "run-a"]);
    let all: Vec<String> = store
        .list(None, 0, 10)
        .into_iter()
        .map(|r| r.run_id)
        .collect();
    assert_eq!(all, ["run-c", "run-b", "run-a"]);
    assert_eq!(store.list(None, 1, 1)[0].run_id, "run-b");
    assert_eq!(store.get("nightly", "run-a").unwrap(), a);
    assert_eq!(
        store.find("run-a").unwrap().crawl_id.as_deref(),
        Some("crawl-1")
    );
    assert_eq!(store.last("nightly").unwrap().run_id, "run-b");
    assert!(store.find("../x").is_none());
    assert_eq!(
        RunResult::CompletedWithWarnings.label(),
        "Completed with warnings"
    );
}

// spec: OUT-169
#[test]
fn only_the_last_100_runs_per_task_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path());
    for i in 0..(KEEP_RUNS + 7) {
        let day = 1 + (i / 24) % 28;
        let started = format!("2026-09-{day:02}T{:02}:00:00.000Z", i % 24);
        // Eindeutige, nach Zeit sortierbare Kennungen.
        let mut r = finished(
            &format!("run-{i:03}"),
            "nightly",
            &started,
            RunResult::Completed,
        );
        r.started_at = format!(
            "2026-{:02}-{:02}T{:02}:00:00.000Z",
            1 + i / 600,
            1 + (i / 24) % 28,
            i % 24
        );
        store.write(&r).unwrap();
    }
    // Ein laufender Lauf wird nie verworfen.
    let live = record("run-live", "nightly", "2026-01-01T00:00:00.000Z");
    store.write(&live).unwrap();
    let removed = store.prune("nightly", KEEP_RUNS);
    assert_eq!(removed, 7);
    let left = store.list(Some("nightly"), 0, 1000);
    assert_eq!(left.len(), KEEP_RUNS + 1);
    assert!(left.iter().any(|r| r.run_id == "run-live"));
    // Die ältesten sind verworfen.
    assert!(store.get("nightly", "run-000").is_none());
    assert!(store
        .get("nightly", &format!("run-{:03}", KEEP_RUNS + 6))
        .is_some());
}

// spec: OUT-169
#[test]
fn a_run_that_ended_without_finishing_becomes_failed_with_unexpected_termination() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path());
    store
        .write(&record("run-dead", "nightly", "2026-10-20T00:00:00.000Z"))
        .unwrap();
    let mut alive = record("run-alive", "nightly", "2026-10-20T00:00:00.000Z");
    alive.heartbeat_at = "2026-10-20T00:05:00.000Z".into();
    store.write(&alive).unwrap();
    let now = at("2026-10-20T00:05:10.000Z");
    assert!(store.get("nightly", "run-alive").unwrap().is_alive(now));
    assert!(!store.get("nightly", "run-dead").unwrap().is_alive(now));
    let fixed = store.reconcile(now);
    assert_eq!(fixed.len(), 1);
    let dead = store.get("nightly", "run-dead").unwrap();
    assert_eq!(dead.result, RunResult::Failed);
    assert_eq!(dead.message.as_deref(), Some("unexpected termination"));
    assert!(dead.ended_at.is_some());
    // Der Lebende bleibt, ein zweiter Abgleich ändert nichts.
    assert_eq!(
        store.get("nightly", "run-alive").unwrap().result,
        RunResult::Running
    );
    assert!(store.reconcile(now).is_empty());
    assert!(STALE_AFTER_SECS >= 30);
}

// spec: OUT-167
#[test]
fn active_runs_of_a_task_are_found_for_the_overlap_check() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path());
    let now = at("2026-10-20T00:01:00.000Z");
    let mut running = record("run-1", "nightly", "2026-10-20T00:00:30.000Z");
    running.heartbeat_at = "2026-10-20T00:01:00.000Z".into();
    store.write(&running).unwrap();
    store
        .write(&finished(
            "run-0",
            "nightly",
            "2026-10-19T00:00:00.000Z",
            RunResult::Completed,
        ))
        .unwrap();
    let mut other = record("run-2", "other", "2026-10-20T00:00:30.000Z");
    other.heartbeat_at = "2026-10-20T00:01:00.000Z".into();
    store.write(&other).unwrap();
    let active = store.active("nightly", now);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].run_id, "run-1");
    // Verschiedene Aufgaben stören sich nicht: jede hat ihre eigene Datei.
    assert_eq!(store.active("other", now).len(), 1);
}

// spec: OUT-171
#[test]
fn a_cancel_request_is_a_marker_next_to_the_run_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path());
    store
        .write(&record("run-1", "nightly", "2026-10-20T00:00:00.000Z"))
        .unwrap();
    assert!(!store.cancel_requested("nightly", "run-1"));
    store.request_cancel("nightly", "run-1").unwrap();
    assert!(store.cancel_requested("nightly", "run-1"));
    assert!(!store.cancel_requested("nightly", "run-2"));
    store.clear_cancel("nightly", "run-1");
    assert!(!store.cancel_requested("nightly", "run-1"));
    assert!(store.request_cancel("nightly", "../x").is_err());
}

// spec: OUT-169
#[test]
fn unsafe_names_never_leave_the_history_folder() {
    let dir = tempfile::tempdir().unwrap();
    let store = HistoryStore::new(dir.path().join("history"));
    let mut bad = record("run-1", "../escape", "2026-10-20T00:00:00.000Z");
    assert!(store.write(&bad).is_err());
    bad.task_id = "nightly".into();
    bad.run_id = "../escape".into();
    assert!(store.write(&bad).is_err());
    assert!(!dir.path().join("escape").exists());
    store.delete_task("nightly");
}
