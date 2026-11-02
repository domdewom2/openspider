//! API settings change during the background WAL merge after a pause (CRAWL-126, OS-Festlegung vom 01.11.2026;
//! INTEG-010). The merge holds the write lock up to 100 ms per attempt while a reader keeps an older snapshot; the
//! configuration and its invalidation note are written in one transaction that waits for the lock.
// The reader stands for another process (e.g. a CLI query) holding a snapshot, outside the app's ReadPool.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#[path = "cmd_api_support.rs"]
mod support;
use os_store::open::{merge_wal, wal_bytes};
use os_store::OpenMode;
use support::*;

/// A reader that holds a snapshot until it is dropped.
fn reader(path: &std::path::Path) -> rusqlite::Connection {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    conn.execute_batch("BEGIN").unwrap();
    let _: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    conn
}

// spec: CRAWL-126
// spec: INTEG-010
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keeping_data_after_a_settings_change_succeeds_during_the_wal_merge() {
    let h = Harness::new().await;
    h.connect_ga4().await;
    h.source(ApiKind::Ga4);
    h.op("api.fetch", json!({"kinds":["ga4"],"scope":{"kind":"all"}}))
        .await
        .unwrap();
    let path = h._dir.path().join("fixture.ospider");

    // A reader holds an older snapshot; frames after it keep the merge looping (as after a pause).
    let held = reader(&path);
    {
        let other = rusqlite::Connection::open(&path).unwrap();
        other.busy_timeout(Duration::from_secs(5)).unwrap();
        other
            .execute(
                "INSERT OR REPLACE INTO crawl_meta(key,value) VALUES('fix_tx_probe','1')",
                [],
            )
            .unwrap();
    }
    assert!(wal_bytes(&path) > 0);
    let cancel = Arc::new(AtomicBool::new(false));
    let merge = std::thread::spawn({
        let (path, cancel) = (path.clone(), cancel.clone());
        move || merge_wal(&path, &cancel).unwrap()
    });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut settings = h
        .call("api.getSettings", json!({"kind":"ga4"}))
        .await
        .unwrap();
    let metrics = ["sessions", "screenPageViews"];
    for change in 0..8 {
        settings["values"]["metrics"] = json!([metrics[change % 2]]);
        let keep = json!({"kind":"ga4","settings":settings,"decision":"keep"});
        let result = h.call("api.setSettings", keep).await;
        assert!(
            result.as_ref().is_ok_and(|r| r["dataStale"] == true),
            "change {change} during the merge: {result:?}"
        );
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    assert!(!merge.is_finished(), "the merge waits for the reader");
    drop(held);
    assert!(merge.join().unwrap(), "merged after the reader ended");

    let saved = h
        .call("api.getSettings", json!({"kind":"ga4"}))
        .await
        .unwrap();
    assert_eq!(saved["values"]["metrics"], json!(["screenPageViews"]));
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let detail: Option<String> = file
        .conn()
        .query_row(
            "SELECT detail FROM api_status WHERE source='ga4'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        detail.as_deref(),
        Some("Daten stammen aus einer früheren Konfiguration")
    );
    h.server.stop().await;
}
