//! Lese-Pool (M2-F03): Verbindungen je Crawl, Kollation je Verbindung, Abbruch über SQLite-Interrupt (UI-141),
//! Fehlerabbildung. Die Pool-Arbeit läuft nie auf dem Tokio-Worker (`spawn_blocking`).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "runtime_common.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use os_analyze::analysis::CancelToken;
use os_app::pool::CrawlKey;
use os_app::ErrorCode;
use os_store::StoreError;

fn key(s: &str) -> CrawlKey {
    CrawlKey(s.to_string())
}

// spec: UI-140
#[tokio::test]
async fn run_reads_from_a_registered_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let path = make_crawl(dir.path(), "a.ospider");
    app.register_crawl(key("a"), &path);

    let name: String = app
        .read_pool()
        .run(key("a"), |file, _cancel| {
            Ok(file.conn().query_row(
                "SELECT value FROM crawl_meta WHERE key = 'name'",
                [],
                |r| r.get::<_, String>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(name, "Testcrawl");
}

// spec: UI-140
#[tokio::test]
async fn unknown_crawl_is_no_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let err = app
        .read_pool()
        .run(key("nope"), |_f, _c| Ok(1))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NoCrawl);
    assert_eq!(err.args["crawl"], "nope");
}

// spec: UI-140
#[tokio::test]
async fn pool_work_runs_on_a_blocking_thread_not_a_runtime_worker() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let caller = std::thread::current().id();
    let worker = app
        .read_pool()
        .run(key("a"), |_f, _c| Ok(std::thread::current().id()))
        .await
        .unwrap();
    assert_ne!(worker, caller);
}

// spec: UI-140
#[tokio::test]
async fn every_connection_has_the_os_text_collation() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    // mehrere gleichzeitige Arbeiten erzwingen mehrere Verbindungen; jede muss COLLATE OS_TEXT kennen
    let gate = Arc::new(std::sync::Barrier::new(3));
    let mut joins = Vec::new();
    for _ in 0..3 {
        let pool = app.read_pool().clone();
        let gate = Arc::clone(&gate);
        joins.push(tokio::spawn(async move {
            pool.run(key("a"), move |file, _c| {
                gate.wait();
                let order: String = file.conn().query_row(
                    "SELECT group_concat(v, ',') FROM (SELECT column1 AS v FROM (VALUES ('b'),('B'),('ä'),('a')) \
                     ORDER BY v COLLATE OS_TEXT, v)",
                    [],
                    |r| r.get(0),
                )?;
                Ok(order)
            })
            .await
        }));
    }
    for j in joins {
        // Reihenfolge nach OS_TEXT: a, ä, B, b (Groß/Klein gleichrangig, bytewise zweiter Schlüssel)
        assert_eq!(j.await.unwrap().unwrap(), "a,ä,B,b");
    }
    assert_eq!(app.read_pool().idle_connections(&key("a")), 3);
}

// spec: UI-140
#[tokio::test]
async fn idle_connections_are_reused_and_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = os_app::AppConfig::new(dir.path());
    config.read_connections = 2;
    let app = os_app::App::new(config).unwrap();
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut joins = Vec::new();
    for _ in 0..6 {
        let pool = app.read_pool().clone();
        let (running, peak) = (Arc::clone(&running), Arc::clone(&peak));
        joins.push(tokio::spawn(async move {
            pool.run(key("a"), move |_f, _c| {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(30));
                running.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            })
            .await
        }));
    }
    for j in joins {
        j.await.unwrap().unwrap();
    }
    assert!(peak.load(Ordering::SeqCst) <= 2, "höchstens 2 gleichzeitig");
    assert!(app.read_pool().idle_connections(&key("a")) <= 2);
}

// spec: UI-141
#[tokio::test]
async fn cancelling_interrupts_a_running_statement_within_100_ms() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let mut samples = Vec::new();
    for _ in 0..5 {
        let token = CancelToken::new();
        let pool = app.read_pool().clone();
        let t2 = token.clone();
        let job = tokio::spawn(async move {
            pool.run_with(key("a"), t2, |file, _c| {
                file.conn()
                    .query_row(SLOW_SQL, [], |r| r.get::<_, i64>(0))
                    .map_err(StoreError::from)
            })
            .await
        });
        // die Anweisung läuft schon
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!job.is_finished(), "die Anweisung muss noch laufen");
        let started = Instant::now();
        token.cancel();
        app.interrupt_hub().poke();
        let err = job.await.unwrap().unwrap_err();
        samples.push(started.elapsed());
        assert_eq!(err.code, ErrorCode::Cancelled);
    }
    assert_cancel_latency(samples);
    // die Verbindung ist danach weiter benutzbar
    let one: i64 = app
        .read_pool()
        .run(key("a"), |f, _c| {
            Ok(f.conn().query_row("SELECT 1", [], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(one, 1);
    assert_eq!(app.interrupt_hub().active(), 0);
}

// spec: UI-141
#[tokio::test]
async fn a_token_cancelled_before_the_start_never_runs_the_work() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    let token = CancelToken::new();
    token.cancel();
    let ran = Arc::new(AtomicUsize::new(0));
    let r2 = Arc::clone(&ran);
    let err = app
        .read_pool()
        .run_with(key("a"), token, move |_f, _c| {
            r2.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled);
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

// spec: UI-140
#[tokio::test]
async fn errors_of_the_work_are_mapped_and_panics_are_contained() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));

    let sql = app
        .read_pool()
        .run(key("a"), |f, _c| {
            f.conn()
                .query_row("SELECT * FROM table_that_does_not_exist", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(StoreError::from)
        })
        .await
        .unwrap_err();
    assert_eq!(sql.code, ErrorCode::Store);

    let invalid = app
        .read_pool()
        .run(key("a"), |_f, _c| -> Result<(), StoreError> {
            Err(StoreError::InvalidInput("Spalte x".into()))
        })
        .await
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::InvalidInput);

    let panicked = app
        .read_pool()
        .run(key("a"), |_f, _c| -> Result<(), StoreError> {
            panic!("kaputt")
        })
        .await
        .unwrap_err();
    assert_eq!(panicked.code, ErrorCode::Internal);
    assert_eq!(panicked.detail.as_deref(), Some("kaputt"));

    // der Pool arbeitet weiter
    app.read_pool()
        .run(key("a"), |_f, _c| Ok(()))
        .await
        .unwrap();
}

// spec: UI-140
#[tokio::test]
async fn a_missing_or_foreign_file_is_a_store_error() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("gone"), dir.path().join("gone.ospider"));
    let e = app
        .read_pool()
        .run(key("gone"), |_f, _c| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);

    let foreign = dir.path().join("x.ospider");
    std::fs::write(&foreign, b"das ist keine SQLite-Datei, nur Text").unwrap();
    app.register_crawl(key("foreign"), &foreign);
    let e = app
        .read_pool()
        .run(key("foreign"), |_f, _c| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Store);
    assert_eq!(e.message_key, "err-not-a-crawl-file");
}

// spec: UI-140
#[tokio::test]
async fn unregistering_removes_the_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.register_crawl(key("a"), make_crawl(dir.path(), "a.ospider"));
    assert!(app.read_pool().is_registered(&key("a")));
    app.unregister_crawl(&key("a"));
    assert!(!app.read_pool().is_registered(&key("a")));
    let e = app
        .read_pool()
        .run(key("a"), |_f, _c| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NoCrawl);
}
