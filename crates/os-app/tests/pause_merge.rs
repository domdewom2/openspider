//! Pause with a background WAL merge (CRAWL-126, OS-Festlegung vom 01.11.2026): "Paused" is acknowledged while a
//! reader still holds an older snapshot; resuming right afterwards cancels the merge, and quitting waits for it.
// The reader stands for another process (e.g. a CLI query) holding a snapshot, outside the app's ReadPool.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_store::open::wal_bytes;
use os_store::{CrawlFile, OpenMode};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// `crawl-site-100` with 30 ms per page: the crawl runs long enough to pause, resume and quit.
fn slow_site() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/crawl-site-100"),
        dir.path(),
    );
    let mut toml = String::new();
    for n in 0..100 {
        let path = if n == 0 {
            "/".to_string()
        } else {
            format!("/page-{n}.html")
        };
        toml.push_str(&format!("[[route]]\npath = \"{path}\"\ndelay_ms = 30\n\n"));
    }
    std::fs::write(dir.path().join("scenario.toml"), toml).unwrap();
    dir
}

/// A reader that holds a snapshot until it is dropped.
fn reader(path: &Path) -> CrawlFile {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    file.conn().execute_batch("BEGIN").unwrap();
    let _: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    file
}

async fn until_crawled(path: &Path, n: i64) {
    for _ in 0..400 {
        let done: i64 = CrawlFile::open(path, OpenMode::ReadOnly)
            .and_then(|f| {
                Ok(f.conn().query_row(
                    "SELECT COUNT(*) FROM urls WHERE state = 'done'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap_or(0);
        if done >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the crawl does not reach {n} URLs");
}

// spec: CRAWL-126
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_right_after_the_ack_cancels_the_merge_and_quit_waits_for_it() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let site_dir = slow_site();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let mut config = cfg();
    config.speed.threads = 1;
    let fetch = Arc::new(
        os_fetch::HttpFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(
                &config,
                &os_fetch::MemorySecretStore::new(),
                Arc::new(os_model::SystemClock),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let started = os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request(format!("http://127.0.0.1:{}/", site.port())),
        config,
        fetch,
    )
    .await
    .unwrap();
    let path = PathBuf::from(&started.file);
    until_crawled(&path, 3).await;

    // Pause while a reader holds an older snapshot: acknowledged, the WAL merge is still pending.
    let held = reader(&path);
    call(&app, "crawl.pause", json!(null)).await;
    assert!(wal_bytes(&path) > 0, "the merge waits for the reader");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let resumed = app.dispatch(ctx(), "crawl.resume", json!(null)).await;
    assert!(resumed.is_ok(), "resume 50 ms after the ack: {resumed:?}");
    drop(held);
    until_crawled(&path, 6).await;

    // Quit pauses, waits for the merge (the reader ends after 300 ms) and stays within 3 s.
    let held = reader(&path);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    let t = Instant::now();
    os_app::quit::confirm(&app, true).await.unwrap();
    let took = t.elapsed();
    release.join().unwrap();
    assert!(took <= Duration::from_secs(3), "quit took {took:?}");
    assert_eq!(wal_bytes(&path), 0, "no WAL content after quit");
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(file.meta().unwrap().state, os_model::CrawlState::Paused);
}

#[derive(Default)]
struct Desktop(std::sync::Mutex<Vec<PathBuf>>);
impl os_app::runtime::hooks::DesktopActions for Desktop {
    fn write_clipboard(&self, _: &str) -> Result<(), os_app::CmdError> {
        Ok(())
    }
    fn reveal_paths(&self, paths: &[PathBuf]) -> Result<(), os_app::CmdError> {
        *self.0.lock().unwrap() = paths.to_vec();
        Ok(())
    }
}

// spec: CRAWL-126
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reveal_waits_for_the_merge_only_within_its_budget() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let desktop = Arc::new(Desktop::default());
    app.set_desktop_actions(desktop.clone());
    let sink = Arc::new(os_app::events::CollectingSink::new());
    app.subscribe(window(), sink.clone());
    let site_dir = slow_site();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let mut config = cfg();
    config.speed.threads = 1;
    let fetch = Arc::new(
        os_fetch::HttpFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(
                &config,
                &os_fetch::MemorySecretStore::new(),
                Arc::new(os_model::SystemClock),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let started = os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request(format!("http://127.0.0.1:{}/", site.port())),
        config,
        fetch,
    )
    .await
    .unwrap();
    let path = PathBuf::from(&started.file);
    until_crawled(&path, 3).await;

    // A foreign snapshot (e.g. a CLI query) keeps the merge pending: the file is shown after the budget anyway.
    let held = reader(&path);
    call(&app, "crawl.pause", json!(null)).await;
    assert!(wal_bytes(&path) > 0);
    let t = Instant::now();
    call(&app, "crawls.reveal", json!({"ids":[started.crawl_id]})).await;
    let took = t.elapsed();
    assert!(took <= Duration::from_secs(3), "reveal took {took:?}");
    assert_eq!(
        desktop.0.lock().unwrap().as_slice(),
        std::slice::from_ref(&path)
    );
    let mut notices = app
        .assemble_tick()
        .map(|tick| tick.notices)
        .unwrap_or_default();
    for event in sink.take() {
        if let os_app::dto::UiEvent::Tick { tick } = event {
            notices.extend(tick.notices);
        }
    }
    assert!(
        notices.iter().any(|n| n.code == "crawl-merge-pending"),
        "{notices:?}"
    );

    // Once the snapshot ends, the merge completes and reveal shows a self-contained file.
    drop(held);
    let key = os_app::pool::CrawlKey(started.crawl_id.clone());
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0);
    let t = Instant::now();
    call(&app, "crawls.reveal", json!({"ids":[started.crawl_id]})).await;
    assert!(t.elapsed() < Duration::from_secs(1));
}

// spec: CRAWL-126
// spec: UI-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_from_paused_succeeds_while_the_merge_waits_for_a_reader() {
    // Windows CI (M2-FIX-STOP): "Stop" after "Pause" failed with "database is locked" while the background merge
    // held the writer lock during its bounded wait for a reader. The merge holds that lock for most of each attempt;
    // several crawls make a hit of that window certain in practice.
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let site_dir = slow_site();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    for round in 0..6 {
        let mut config = cfg();
        config.speed.threads = 1;
        let fetch = Arc::new(
            os_fetch::HttpFetcher::new(
                os_fetch::FetchConfig::from_crawl_config(
                    &config,
                    &os_fetch::MemorySecretStore::new(),
                    Arc::new(os_model::SystemClock),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let started = os_app::session::control::start_with_fetcher(
            &app,
            ctx(),
            request(format!("http://127.0.0.1:{}/", site.port())),
            config,
            fetch,
        )
        .await
        .unwrap();
        let path = PathBuf::from(&started.file);
        until_crawled(&path, 3).await;

        // A reader keeps an older snapshot: "Paused" is acknowledged and the merge keeps retrying in the background.
        let held = reader(&path);
        call(&app, "crawl.pause", json!(null)).await;
        assert!(
            wal_bytes(&path) > 0,
            "round {round}: the merge waits for the reader"
        );
        tokio::time::sleep(Duration::from_millis(20 * round)).await;
        let stopped = app.dispatch(ctx(), "crawl.stop", json!(null)).await;
        assert!(
            stopped.is_ok(),
            "round {round}: stop during the merge: {stopped:?}"
        );
        let state = {
            let check = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
            let state = check.meta().unwrap().state;
            let reason = check.get_meta("end_reason").unwrap();
            (state, reason)
        };
        assert_eq!(
            state,
            (os_model::CrawlState::Completed, Some("stopped".to_string())),
            "round {round}"
        );
        // The merge continues after the stop and also takes the stop commit back into the main file.
        drop(held);
        let key = os_app::pool::CrawlKey(started.crawl_id.clone());
        os_app::session::merge::wait_merged(&app, &key).await;
        assert_eq!(
            wal_bytes(&path),
            0,
            "round {round}: WAL empty after the merge"
        );
    }
}

/// Starts a one-thread crawl on the slow site and waits for three crawled URLs.
async fn started_crawl(app: &os_app::App, site: &os_testsite::TestSite) -> (PathBuf, String) {
    let mut config = cfg();
    config.speed.threads = 1;
    let fetch = Arc::new(
        os_fetch::HttpFetcher::new(
            os_fetch::FetchConfig::from_crawl_config(
                &config,
                &os_fetch::MemorySecretStore::new(),
                Arc::new(os_model::SystemClock),
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let started = os_app::session::control::start_with_fetcher(
        app,
        ctx(),
        request(format!("http://127.0.0.1:{}/", site.port())),
        config,
        fetch,
    )
    .await
    .unwrap();
    let path = PathBuf::from(&started.file);
    until_crawled(&path, 3).await;
    (path, started.crawl_id)
}

// spec: CRAWL-126
// spec: UI-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_from_paused_waits_for_the_writer_lock_and_merges_its_own_commit() {
    // Deterministic variant: another connection holds the writer lock for 200 ms, as the merge does while it waits
    // for readers. The stop commit waits for the lock (busy timeout) instead of failing with "database is locked".
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let site_dir = slow_site();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let (path, crawl_id) = started_crawl(&app, &site).await;
    let key = os_app::pool::CrawlKey(crawl_id);
    call(&app, "crawl.pause", json!(null)).await;
    os_app::session::merge::wait_merged(&app, &key).await;

    let writer = rusqlite::Connection::open(&path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        writer.execute_batch("COMMIT").unwrap();
    });
    let t = Instant::now();
    let stopped = app.dispatch(ctx(), "crawl.stop", json!(null)).await;
    let took = t.elapsed();
    release.join().unwrap();
    assert!(
        stopped.is_ok(),
        "stop while the writer lock is held: {stopped:?}"
    );
    assert!(
        took >= Duration::from_millis(150),
        "the stop commit waited for the lock ({took:?})"
    );
    let check = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(check.meta().unwrap().state, os_model::CrawlState::Completed);
    assert_eq!(
        check.get_meta("end_reason").unwrap().as_deref(),
        Some("stopped")
    );
    drop(check);

    // The merge of the pause had ended: the stop starts a new one, which empties the WAL without further action.
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0, "the stop commit is merged");
}
