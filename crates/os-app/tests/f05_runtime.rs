//! Infrastruktur (M2-F05b); Fachprüfungen folgen in den angegebenen Besitzerpaketen.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use futures_util::future::BoxFuture;
use os_app::{
    pool::CrawlKey,
    runtime::{hooks::CompletionBarrier, lifecycle_bus::CrawlEvent, sessions::SessionInfo},
    App, AppConfig, WindowId,
};
use os_model::{CrawlConfig, CrawlState};
use std::sync::{Arc, Mutex};

// spec: UI-140
#[test]
fn sessions_extensions_and_window_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let w = WindowId("main".into());
    let key = CrawlKey("fixture".into());
    let path = common::make_crawl(dir.path(), "session.ospider");
    app.sessions().set_current(
        w.clone(),
        SessionInfo {
            key: key.clone(),
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );
    let cfg = Arc::new(CrawlConfig::default());
    app.sessions().update_config(&key, cfg.clone());
    app.sessions()
        .rename_path(&key, dir.path().join("renamed.ospider"));
    let session = app.sessions().current(&w).unwrap();
    assert!(Arc::ptr_eq(&session.config, &cfg));
    assert_eq!(session.path, dir.path().join("renamed.ospider"));
    assert_eq!(app.sessions().all().len(), 1);
    assert!(Arc::ptr_eq(
        &app.ext::<Mutex<Vec<u32>>>(),
        &app.ext::<Mutex<Vec<u32>>>()
    ));
    let other = common::new_app(&dir.path().join("other"));
    assert!(!Arc::ptr_eq(
        &app.ext::<Mutex<Vec<u32>>>(),
        &other.ext::<Mutex<Vec<u32>>>()
    ));
    let called = Arc::new(Mutex::new(Vec::new()));
    let record = called.clone();
    app.on_window_closed(Box::new(move |app, window| {
        assert!(app.sessions().current(window).is_some());
        record.lock().unwrap().push(window.clone());
    }));
    app.window_closed(&w);
    assert_eq!(*called.lock().unwrap(), vec![w.clone()]);
    assert!(app.sessions().current(&w).is_none());
}
struct Barrier {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}
impl CompletionBarrier for Barrier {
    fn wait(&self, _: CrawlKey) -> BoxFuture<'static, ()> {
        let entered = self.entered.clone();
        let release = self.release.clone();
        Box::pin(async move {
            entered.notify_one();
            release.notified().await;
        })
    }
}
// spec: CRAWL-141
#[tokio::test]
async fn lifecycle_order_and_completion_barrier() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let order = Arc::new(Mutex::new(Vec::new()));
    for n in 0..3 {
        let order = order.clone();
        app.crawl_events().subscribe(Box::new(move |app, event| {
            assert!(matches!(event, CrawlEvent::Completed(_)));
            // Hörer dürfen das Register erneut betreten.
            app.crawl_events().subscribe(Box::new(|_, _| {}));
            order.lock().unwrap().push(n);
        }));
    }
    let key = CrawlKey("fixture".into());
    app.crawl_events()
        .emit(&app, CrawlEvent::Completed(key.clone()));
    assert_eq!(*order.lock().unwrap(), vec![0, 1, 2]);
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    app.crawl_events().add_barrier(
        "analysis",
        Arc::new(Barrier {
            entered: entered.clone(),
            release: release.clone(),
        }),
    );
    let waiter = tokio::spawn(async move {
        app.crawl_events().wait_barriers(&key).await;
    });
    entered.notified().await;
    assert!(!waiter.is_finished());
    release.notify_one();
    waiter.await.unwrap();
}
// spec: OUT-225
#[test]
fn app_config_uses_mutable_memory_secrets() {
    use os_fetch::{MemorySecretStore, SecretStoreMut};
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets)).unwrap();
    // SecretStoreMut is the public AppConfig contract, never a keyring in this test.
    fn mutable(_: &Arc<dyn SecretStoreMut>) {}
    mutable(&app.config().secrets);
}
// spec: UI-140
#[tokio::test]
async fn file_admin_and_sql_jobs_share_one_order() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let key = CrawlKey("fixture".into());
    let path = common::make_crawl(dir.path(), "write.ospider");
    app.register_crawl(key.clone(), path);
    app.write_queue()
        .submit(key.clone(), |conn| {
            conn.execute_batch("CREATE TABLE f05_order (n INTEGER)")?;
            Ok(())
        })
        .await
        .unwrap();
    app.write_queue()
        .submit_file(&key, |file| {
            file.conn()
                .execute("INSERT INTO f05_order VALUES (1)", [])
                .unwrap();
            Ok(())
        })
        .await
        .unwrap();
    let (thread_a, value) = app
        .write_queue()
        .submit_file(&key, |file| {
            let n: i64 = file
                .conn()
                .query_row("SELECT n FROM f05_order", [], |r| r.get(0))
                .unwrap();
            Ok((std::thread::current().id(), n))
        })
        .await
        .unwrap();
    let thread_b = app
        .write_queue()
        .submit_admin(|| Ok(std::thread::current().id()))
        .await
        .unwrap();
    assert_eq!(value, 1);
    assert_eq!(thread_a, thread_b);
}
// spec: UI-140
#[tokio::test]
async fn exclusive_waits_for_readers_and_blocks_new_readers() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let key = CrawlKey("fixture".into());
    let path = common::make_crawl(dir.path(), "read.ospider");
    app.register_crawl(key.clone(), path);
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let pool = app.read_pool().clone();
    let read_key = key.clone();
    let reader = tokio::spawn(async move {
        pool.run(read_key, move |_, _| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        })
        .await
        .unwrap();
    });
    entered_rx.await.unwrap();
    let pool = app.read_pool().clone();
    let exclusive_key = key.clone();
    let exclusive = tokio::spawn(async move { pool.exclusive(&exclusive_key).await });
    tokio::task::yield_now().await;
    assert!(!exclusive.is_finished());
    release_tx.send(()).unwrap();
    reader.await.unwrap();
    let guard = exclusive.await.unwrap();
    assert_eq!(app.read_pool().idle_connections(&key), 0);
    let pool = app.read_pool().clone();
    let reader = tokio::spawn(async move { pool.run(key, |_, _| Ok(42)).await.unwrap() });
    tokio::task::yield_now().await;
    assert!(!reader.is_finished());
    drop(guard);
    assert_eq!(reader.await.unwrap(), 42);
}
// spec: OUT-184
#[tokio::test]
async fn prepare_open_checks_current_and_migrates_old_copy() {
    use os_store::schema;
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let path = common::make_crawl(dir.path(), "open.ospider");
    let key = CrawlKey("fixture".into());
    let report = app
        .read_pool()
        .prepare_open(key.clone(), path.clone())
        .await
        .unwrap();
    assert!(!report.migrated);
    assert_eq!(report.path, path);
    let latest = schema::latest_version(schema::MIGRATIONS);
    // Reale v3-Fixture aus den unveränderten Migrationen, unabhängig vom aktuellen Schema.
    let old = dir.path().join("v3.ospider");
    let mut conn = rusqlite::Connection::open(&old).unwrap();
    conn.pragma_update(None, "application_id", schema::APPLICATION_ID)
        .unwrap();
    schema::migrate(&mut conn, 0, &schema::MIGRATIONS[..3]).unwrap();
    drop(conn);
    let before = std::fs::read(&old).unwrap();
    let report = app
        .read_pool()
        .prepare_open(CrawlKey("v3".into()), old.clone())
        .await
        .unwrap();
    assert_eq!(report.migrated, latest > 3);
    assert_eq!(std::fs::read(&old).unwrap(), before);
    if latest > 3 {
        assert_ne!(report.path, old);
    }
}

// Infrastruktur (M2-F05b); Taktpolitik wird durch M2-K04 ausgefüllt.
// spec: UI-143
#[test]
fn tick_policy_can_defer_without_losing_pending_notices() {
    use os_app::{
        dto::Notice,
        events::{EventSink, SinkClosed},
        runtime::hooks::TickPolicy,
    };
    struct Policy;
    impl TickPolicy for Policy {
        fn due_windows(&self, _: &App) -> Vec<WindowId> {
            Vec::new()
        }
    }
    struct Sink;
    impl EventSink for Sink {
        fn send(&self, _: os_app::dto::UiEvent) -> Result<(), SinkClosed> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    app.subscribe(WindowId("main".into()), Arc::new(Sink));
    app.set_tick_policy(Arc::new(Policy));
    app.push_notice(Notice {
        level: os_app::dto::NoticeLevel::Info,
        code: "fixture".into(),
        args: Default::default(),
    });
    assert_eq!(app.flush_tick(), 0);
    assert!(app
        .assemble_tick()
        .unwrap()
        .notices
        .iter()
        .any(|n| n.code == "fixture"));
}
