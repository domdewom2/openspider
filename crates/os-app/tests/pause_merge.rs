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
