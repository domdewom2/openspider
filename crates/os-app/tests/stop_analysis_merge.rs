//! WAL merge around stop, crawl analysis and quit (CRAWL-126, CRAWL-141; OS-Festlegungen vom 01.11.2026):
//! - "Stop" from "Paused" while a reader holds an older snapshot, followed by the automatic analysis: the analysis
//!   completes, and once the reader ends the WAL is empty without any user action.
//! - The analysis' final checkpoint waits only briefly for readers; the frames it leaves are merged in the background.
//! - Quitting first runs the app's own pending writes and then merges, so the WAL is empty afterwards.
// The reader stands for another process (e.g. a CLI query) holding a snapshot, outside the app's ReadPool.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::dto::{OpState, OpStatus};
use os_app::pool::CrawlKey;
use os_engine::testing::{SimFetcher, SimSite};
use os_store::open::wal_bytes;
use os_store::{CrawlFile, OpenMode};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Start page with an inline CSS background (images step, ONPAGE-178; the plain link lets the crawl fetch the image)
/// and 20 slow pages.
fn site() -> SimSite {
    let links: String = (1..=20)
        .map(|n| format!("<a href=\"/p{n}\">Seite {n}</a> "))
        .collect();
    let mut site = SimSite::new()
        .delay(Duration::from_millis(40))
        .page(
            "https://fixture.test/",
            &format!(
                "<html><head><title>Start</title></head><body>\
                 <div style=\"background-image:url(/hero.png)\">Hero</div>\
                 <a href=\"/hero.png\">Bild</a> {links}</body></html>"
            ),
        )
        .asset("https://fixture.test/hero.png", "image/png", "png");
    for n in 1..=20 {
        site = site.page(
            &format!("https://fixture.test/p{n}"),
            &format!(
                "<html><head><title>Seite {n}</title></head><body><a href=\"/\">Start</a></body></html>"
            ),
        );
    }
    site
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

/// Final state of the automatic analysis; `crawl.stop` registers it before it returns.
async fn automatic_analysis(app: &os_app::App) -> OpStatus {
    let op = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(op) = app
                .ops()
                .active_statuses()
                .into_iter()
                .find(|op| op.label_key == "op-analysis")
            {
                break op;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the analysis starts");
    tokio::time::timeout(Duration::from_secs(60), app.ops().wait(&op.op_id))
        .await
        .expect("the analysis ends")
        .expect("known operation")
}

/// A one-thread crawl on the simulated site with three crawled URLs.
async fn started(app: &os_app::App, auto_analysis: bool) -> (PathBuf, CrawlKey) {
    let mut config = cfg();
    config.speed.threads = 1;
    config.analysis.auto_at_end = auto_analysis;
    config.images.parse_inline_css_backgrounds = true;
    let started = os_app::session::control::start_with_fetcher(
        app,
        ctx(),
        request("https://fixture.test/".into()),
        config,
        SimFetcher::new(site()),
    )
    .await
    .unwrap();
    let path = PathBuf::from(&started.file);
    until_crawled(&path, 3).await;
    (path, CrawlKey(started.crawl_id))
}

fn assert_stopped(path: &Path) {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    assert_eq!(file.meta().unwrap().state, os_model::CrawlState::Completed);
    assert_eq!(
        file.get_meta("end_reason").unwrap().as_deref(),
        Some("stopped")
    );
}

// spec: CRAWL-126
// spec: CRAWL-141
// spec: ONPAGE-178
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_from_paused_with_a_held_reader_then_analysis_and_an_empty_wal() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let (path, key) = started(&app, true).await;

    // Pause while a reader holds an older snapshot: the merge waits in the background; stop commits during it.
    let held = reader(&path);
    call(&app, "crawl.pause", json!(null)).await;
    assert!(wal_bytes(&path) > 0, "the merge waits for the reader");
    call(&app, "crawl.stop", json!(null)).await;
    assert_stopped(&path);

    // The automatic analysis completes while the reader still holds its snapshot.
    let result = automatic_analysis(&app).await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    {
        let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
        assert_eq!(
            file.get_meta("analysis_state").unwrap().as_deref(),
            Some("completed")
        );
        let images: String = file
            .conn()
            .query_row(
                "SELECT state FROM analysis_steps WHERE step='images'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(images, "done");
    }
    assert!(wal_bytes(&path) > 0, "the reader still holds the WAL");

    // Once the reader ends, the WAL is merged without any user action.
    drop(held);
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0, "WAL empty after the reader ended");
}

// spec: CRAWL-141
// spec: CRAWL-126
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn analysis_leaves_its_frames_to_a_background_merge_when_a_reader_holds_the_wal() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let (path, key) = started(&app, false).await;
    call(&app, "crawl.stop", json!(null)).await;
    assert_stopped(&path);
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0, "no merge pending before the analysis");

    // The analysis' final checkpoint cannot truncate the WAL while the reader holds its snapshot.
    let held = reader(&path);
    let result = operation(&app, "analysis.start", json!({"steps": null})).await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    assert!(wal_bytes(&path) > 0, "the reader keeps the analysis frames");

    // The analysis started a background merge: it finishes once the reader ends.
    drop(held);
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0, "the analysis frames are merged");
}

// spec: CRAWL-126
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quit_runs_a_pending_view_cache_write_first_and_leaves_an_empty_wal() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let (path, key) = started(&app, false).await;
    call(&app, "crawl.stop", json!(null)).await;
    os_app::session::merge::wait_merged(&app, &key).await;
    assert_eq!(wal_bytes(&path), 0);

    // A view-cache write waits in the write queue (held until quitting has begun).
    let (release, held) = std::sync::mpsc::channel::<()>();
    let pending = {
        let (app, key) = (app.clone(), key.clone());
        tokio::spawn(async move {
            app.write_queue()
                .submit_file(&key, move |file| {
                    held.recv().ok();
                    os_store::view_cache::save(
                        file,
                        "fix-wal:probe",
                        "p",
                        &os_store::view_cache::Entry {
                            step_stamp: "s".into(),
                            built_seq: 1,
                            data: vec![1, 2, 3],
                        },
                    )
                    .map_err(|e| {
                        os_app::CmdError::new(os_app::ErrorCode::Store, "err-store")
                            .with_detail(e.to_string())
                    })
                })
                .await
        })
    };
    let quit = {
        let app = app.clone();
        tokio::spawn(async move { os_app::quit::confirm(&app, true).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    release.send(()).unwrap();
    quit.await.unwrap().unwrap();
    assert!(
        pending.await.unwrap().unwrap(),
        "the pending view-cache write ran"
    );
    assert_eq!(wal_bytes(&path), 0, "no WAL content after quit");
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert!(
        os_store::view_cache::load(&file, "fix-wal:probe", "p")
            .unwrap()
            .is_some(),
        "the pending write is in the main file"
    );
}
