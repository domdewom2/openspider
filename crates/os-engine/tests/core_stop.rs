//! Stopp und Pause gegen die echte Crawl-Datei: die Warteschlange bleibt in der Datei, der Zustand stimmt
//! (CRAWL-026, CRAWL-130). Das Fortsetzen selbst (`resume_crawl`) ist Sache von P19.

#[path = "core_support.rs"]
mod support;

use std::time::Duration;

use os_model::{CrawlConfig, CrawlState, EndReason};
use os_store::{CrawlFile, OpenMode};

/// Kopie der Fixture `crawl-site-100` mit `delay_ms` je Seite.
fn slow_site(delay_ms: u64) -> tempfile::TempDir {
    let src = os_testsite::fixtures_root().join("crawl-site-100");
    let dir = tempfile::tempdir().unwrap();
    copy(&src, dir.path());
    let mut toml = String::new();
    for n in 0..100 {
        let path = if n == 0 {
            "/".to_string()
        } else {
            format!("/page-{n}.html")
        };
        toml.push_str(&format!(
            "[[route]]\npath = \"{path}\"\ndelay_ms = {delay_ms}\n\n"
        ));
    }
    std::fs::write(dir.path().join("scenario.toml"), toml).unwrap();
    dir
}

fn copy(from: &std::path::Path, to: &std::path::Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn queue_is_consistent(file: &CrawlFile) -> (i64, i64) {
    let queue: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM queue", [], |r| r.get(0))
        .unwrap();
    // jede Zeile der Warteschlange gehört zu einer URL im Zustand „queued“ – und umgekehrt
    let orphans: i64 = file
        .conn()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM queue q LEFT JOIN urls u ON u.id = q.url_id WHERE u.state IS NOT 'queued') \
             + (SELECT COUNT(*) FROM urls u LEFT JOIN queue q ON q.url_id = u.id WHERE u.state = 'queued' AND q.url_id IS NULL)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    (queue, orphans)
}

// spec: CRAWL-026
#[tokio::test]
async fn stop_keeps_the_queue_in_the_file_and_marks_the_crawl_as_stopped() {
    let site = slow_site(30);
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 2;
    let run = support::launch(site.path(), "/", cfg).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    run.handle.stop();
    let summary = run.handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Completed);
    assert_eq!(summary.end_reason, Some(EndReason::Stopped));

    let file = CrawlFile::open(&run.path, OpenMode::ReadOnly).unwrap();
    let meta = file.meta().unwrap();
    assert_eq!(meta.state, CrawlState::Completed);
    assert_eq!(meta.end_reason, Some(EndReason::Stopped));
    let (queue, orphans) = queue_is_consistent(&file);
    assert!(
        queue > 0,
        "Rest der Warteschlange bleibt für das Fortsetzen"
    );
    assert_eq!(orphans, 0, "Warteschlange und URL-Zustand stimmen überein");
    let resume = file.resume_state().unwrap();
    assert_eq!(resume.queue.len() as i64, queue);
    assert!(resume.next_url_id > 1 && resume.next_seq > 1);
}

// spec: CRAWL-026
#[tokio::test]
async fn pause_stores_running_pages_and_leaves_state_paused() {
    let site = slow_site(30);
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 3;
    let run = support::launch(site.path(), "/", cfg).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    run.handle.pause();
    let summary = run.handle.wait().await.unwrap();
    assert_eq!(summary.state, CrawlState::Paused);
    assert_eq!(summary.end_reason, None);

    let file = CrawlFile::open(&run.path, OpenMode::ReadOnly).unwrap();
    let meta = file.meta().unwrap();
    assert_eq!(meta.state, CrawlState::Paused);
    assert_eq!(meta.ended_at, None);
    let (queue, orphans) = queue_is_consistent(&file);
    assert!(queue > 0);
    assert_eq!(orphans, 0);
    // nichts ist halb fertig: jede abgerufene Seite hat Statuscode und Zustand done
    let broken: i64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE (status_code IS NOT NULL) != (state = 'done') AND discovered_via != 'robots_txt'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(broken, 0);
}
