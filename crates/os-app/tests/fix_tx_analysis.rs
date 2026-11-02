//! Automatic crawl analysis right after the end of a crawl while the background WAL merge still waits for a reader
//! (CRAWL-126, OS-Festlegung vom 01.11.2026; CRAWL-141). The merge holds the write lock up to 100 ms per attempt;
//! analysis transactions that read before they write must wait for it instead of failing with `SQLITE_BUSY`.
// The reader stands for another process (e.g. a CLI query) holding a snapshot, outside the app's ReadPool.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::dto::{OpState, OpStatus};
use os_engine::testing::{SimFetcher, SimSite};
use os_store::open::wal_bytes;
use os_store::{CrawlFile, OpenMode};
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

/// Final state of the automatic analysis started at the end of the crawl.
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
    .expect("the automatic analysis starts");
    tokio::time::timeout(Duration::from_secs(60), app.ops().wait(&op.op_id))
        .await
        .expect("the automatic analysis ends")
        .expect("known operation")
}

// spec: CRAWL-126
// spec: CRAWL-141
// spec: ONPAGE-178
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_analysis_completes_while_the_wal_merge_waits_for_a_reader() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let mut config = cfg();
    config.speed.threads = 1;
    config.analysis.auto_at_end = true;
    config.images.parse_inline_css_backgrounds = true;
    let started = os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request("https://fixture.test/".into()),
        config,
        SimFetcher::new(site()),
    )
    .await
    .unwrap();
    let path = PathBuf::from(&started.file);
    until_crawled(&path, 3).await;

    // The crawl ends while a reader holds an older snapshot: the merge runs in the background when the analysis starts.
    let held = reader(&path);
    completed(&app).await;
    assert!(wal_bytes(&path) > 0, "the merge waits for the reader");
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
        let backgrounds: i64 = file
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM url_flags f JOIN urls u ON u.id=f.url_id \
                 WHERE f.flag='image_background' AND u.url='https://fixture.test/hero.png'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(backgrounds, 1, "the images step published its result");
    }

    // Once the snapshot ends, the running merge also returns the analysis frames to the main file. (Writes of other
    // app services after the merge, e.g. the view-cache precomputation, may start a new WAL; see the package report.)
    let pending = wal_bytes(&path);
    drop(held);
    let key = os_app::pool::CrawlKey(started.crawl_id.clone());
    os_app::session::merge::wait_merged(&app, &key).await;
    assert!(
        wal_bytes(&path) < pending,
        "the merge took the analysis frames along: {} of {pending} bytes left",
        wal_bytes(&path)
    );
}
