//! Unit test of the view-cache gate while quitting (`crate::quit`, CRAWL-126). Included from `src/quit.rs`; lives
//! here because only `pool.rs` and `write_queue.rs` in `src` may name store types (`runtime_lint`).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use super::*;
use crate::AppConfig;
use futures_util::FutureExt;
use os_store::view_cache::{self, Entry};

fn store(key: &'static str) -> impl FnOnce(&mut os_store::CrawlFile) -> Result<(), CmdError> {
    move |file| {
        let entry = Entry {
            step_stamp: "s".into(),
            built_seq: 1,
            data: vec![1],
        };
        view_cache::save(file, key, "p", &entry)
            .map(drop)
            .map_err(|e| CmdError::new(ErrorCode::Store, "err-store").with_detail(e.to_string()))
    }
}

/// The callers' pattern (views, sidebar): no submission without a ticket, the ticket checked on the writer thread.
async fn submit_cache(
    app: &App,
    key: &crate::pool::CrawlKey,
    f: impl FnOnce(&mut os_store::CrawlFile) -> Result<(), CmdError> + Send + 'static,
) -> Result<(), CmdError> {
    let Some(ticket) = cache_ticket(app) else {
        return Ok(());
    };
    app.write_queue()
        .submit_file(
            key,
            move |file| if ticket.open() { f(file) } else { Ok(()) },
        )
        .await
}

fn stored(path: &std::path::Path, key: &str) -> bool {
    let file = os_store::CrawlFile::open(path, os_store::OpenMode::ReadOnly).unwrap();
    view_cache::load(&file, key, "p").unwrap().is_some()
}

// spec: CRAWL-126
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_cache_writes_queued_before_quitting_run_and_later_ones_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gate.ospider");
    let file = os_store::CrawlFile::create(
        &path,
        &os_store::CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000126".parse().unwrap(),
            mode: os_model::CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "gate".into(),
            name: "gate".into(),
            config: os_model::CrawlConfig::default(),
            created_at: os_model::Timestamp::UNIX_EPOCH,
        },
        os_store::StorageProfile::Balanced,
    )
    .unwrap();
    file.set_meta("state", "Completed").unwrap();
    drop(file);
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    app.dispatch(
        CallCtx::for_window("main"),
        "crawl.open",
        serde_json::json!({"kind": "path", "path": path}),
    )
    .await
    .unwrap();
    let key = app.sessions().all()[0].key.clone();

    // Hold the writer thread so that the first write is still queued when quitting begins.
    let (release, held) = std::sync::mpsc::channel::<()>();
    let mut blocker = Box::pin(app.write_queue().submit_file(&key, move |_| {
        held.recv().ok();
        Ok(())
    }));
    assert!(
        (&mut blocker).now_or_never().is_none(),
        "the writer thread is held"
    );
    let mut queued = Box::pin(submit_cache(&app, &key, store("queued")));
    assert!(
        (&mut queued).now_or_never().is_none(),
        "queued behind the held job"
    );

    close_cache(&app);
    submit_cache(&app, &key, store("after-close"))
        .await
        .unwrap();
    release.send(()).unwrap();
    blocker.await.unwrap();
    queued.await.unwrap();
    assert!(
        stored(&path, "queued"),
        "a write queued before quitting runs"
    );
    assert!(
        !stored(&path, "after-close"),
        "no new write once quitting began"
    );

    // A submission that passed the check just before quitting began runs after the seal and is skipped too.
    let gate = app.ext::<CacheGate>();
    gate.closing.store(false, Ordering::SeqCst);
    gate.sealed.store(true, Ordering::SeqCst);
    submit_cache(&app, &key, store("raced")).await.unwrap();
    assert!(!stored(&path, "raced"));

    // A refused or failed quit opens the cache again.
    reopen_cache(&app);
    submit_cache(&app, &key, store("reopened")).await.unwrap();
    assert!(stored(&path, "reopened"));
}
