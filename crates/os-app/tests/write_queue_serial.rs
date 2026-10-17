//! Schreibwarteschlange (M2-F03, K-20): ein Thread, Aufträge nacheinander, `busy_timeout` 5 s, Fehlerabbildung.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "runtime_common.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use os_app::pool::CrawlKey;
use os_app::ErrorCode;
use os_store::StoreError;

fn key(s: &str) -> CrawlKey {
    CrawlKey(s.to_string())
}

// spec: UI-140
#[tokio::test]
async fn jobs_run_strictly_one_after_the_other_in_submission_order() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let running = Arc::new(AtomicUsize::new(0));
    let overlap = Arc::new(AtomicUsize::new(0));
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut futs = Vec::new();
    for i in 0..8usize {
        let (running, overlap, order) = (
            Arc::clone(&running),
            Arc::clone(&overlap),
            Arc::clone(&order),
        );
        // die Aufträge werden der Reihe nach eingereiht (submit reiht sofort beim ersten Poll ein)
        let fut = app.write_queue().submit(key("a"), move |conn| {
            if running.fetch_add(1, Ordering::SeqCst) != 0 {
                overlap.fetch_add(1, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(5));
            conn.execute(
                "INSERT INTO crawl_meta (key, value) VALUES (?1, ?2)",
                [format!("job{i}"), i.to_string()],
            )?;
            order.lock().unwrap().push(i);
            running.fetch_sub(1, Ordering::SeqCst);
            Ok(i)
        });
        // der erste Poll reiht den Auftrag ein; so liegen alle acht gleichzeitig in der Warteschlange
        let mut fut = Box::pin(fut);
        let _ = futures_util::poll!(fut.as_mut());
        futs.push(fut);
    }
    let mut results = Vec::new();
    for f in futs {
        results.push(f.await.unwrap());
    }
    assert_eq!(results, (0..8).collect::<Vec<_>>());
    assert_eq!(overlap.load(Ordering::SeqCst), 0, "keine Überlappung");
    assert_eq!(*order.lock().unwrap(), (0..8).collect::<Vec<_>>());
}

// spec: UI-140
#[tokio::test]
async fn the_write_connection_has_a_busy_timeout_of_five_seconds_and_the_collation() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let (timeout_ms, sorted): (i64, String) = app
        .write_queue()
        .submit(key("a"), |conn| {
            let t = conn.query_row("PRAGMA busy_timeout", [], |r| r.get::<_, i64>(0))?;
            let s = conn.query_row(
                "SELECT group_concat(v, ',') FROM (SELECT column1 AS v FROM (VALUES ('b'),('a')) ORDER BY v COLLATE OS_TEXT)",
                [],
                |r| r.get::<_, String>(0),
            )?;
            Ok((t, s))
        })
        .await
        .unwrap();
    assert_eq!(timeout_ms, 5000);
    assert_eq!(sorted, "a,b");
    assert_eq!(os_app::write_queue::BUSY_TIMEOUT, Duration::from_secs(5));
}

// spec: UI-140
#[tokio::test]
async fn a_job_waits_for_a_lock_held_by_the_engine_writer_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let path = make_crawl(dir.path(), "a.ospider");
    app.register_crawl(key("a"), &path);

    // der „Engine-Writer“ hält die Schreibsperre 400 ms
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let holder_path = path.clone();
    let holder = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(&holder_path).unwrap();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        locked_tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        conn.execute_batch("COMMIT").unwrap();
    });
    locked_rx.recv().unwrap();

    let started = Instant::now();
    app.write_queue()
        .submit(key("a"), |conn| {
            conn.execute(
                "INSERT INTO crawl_meta (key, value) VALUES ('queued', '1')",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("wartet auf die Sperre statt SQLITE_BUSY");
    assert!(
        started.elapsed() >= Duration::from_millis(250),
        "{:?}",
        started.elapsed()
    );
    holder.join().unwrap();

    let v: String = app
        .read_pool()
        .run(key("a"), |f, _c| {
            Ok(f.conn().query_row(
                "SELECT value FROM crawl_meta WHERE key = 'queued'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(v, "1");
}

// spec: UI-140
#[tokio::test]
async fn errors_are_mapped_and_a_panic_does_not_kill_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let q = app.write_queue();

    let e = q
        .submit(key("a"), |conn| {
            conn.execute("INSERT INTO no_such_table VALUES (1)", [])?;
            Ok(())
        })
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Store);

    let e = q
        .submit(key("a"), |_conn| -> Result<(), StoreError> {
            Err(StoreError::InvalidInput("Segment fehlt".into()))
        })
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);

    let e = q
        .submit(key("a"), |_conn| -> Result<(), StoreError> {
            panic!("boom")
        })
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Internal);

    let n = q.submit(key("a"), |_c| Ok(7)).await.unwrap();
    assert_eq!(n, 7);

    let e = q.submit(key("unbekannt"), |_c| Ok(())).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::NoCrawl);
}

// spec: UI-140
#[tokio::test]
async fn files_with_another_schema_or_foreign_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let path = make_crawl(dir.path(), "a.ospider");
    // ältere Schemaversion vortäuschen: die Engine migriert, die Warteschlange nie
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    app.register_crawl(key("old"), &path);
    let e = app
        .write_queue()
        .submit(key("old"), |_c| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Store);
    assert_eq!(e.message_key, "err-crawl-migration-required");

    let foreign = dir.path().join("f.ospider");
    {
        let conn = rusqlite::Connection::open(&foreign).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
    }
    app.register_crawl(key("foreign"), &foreign);
    let e = app
        .write_queue()
        .submit(key("foreign"), |_c| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(e.message_key, "err-not-a-crawl-file");
}

// spec: UI-140
#[tokio::test]
async fn flush_waits_for_queued_jobs_and_drop_stops_the_thread() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let done = Arc::new(AtomicUsize::new(0));
    let d2 = Arc::clone(&done);
    let pending = app.write_queue().submit(key("a"), move |_c| {
        std::thread::sleep(Duration::from_millis(80));
        d2.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let mut pending = Box::pin(pending);
    // einreihen, ohne zu warten
    let _ = futures_util::poll!(pending.as_mut());
    app.write_queue().flush().await;
    assert_eq!(done.load(Ordering::SeqCst), 1);
    pending.await.unwrap();

    let queue = os_app::write_queue::WriteQueue::new();
    queue.shutdown();
    let e = queue.submit(key("a"), |_c| Ok(())).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Internal);
}
