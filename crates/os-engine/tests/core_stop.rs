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

// ---------------------------------------------------------------------------------------------------
// Weiterleitungs-Flags laufen über den Writer (M1.md 3.11): sie stehen auch nach Pause und Stopp in der Datei.

/// Szenario: `/` verlinkt eine Schleife (`/loop-a` ↔ `/loop-b`), den Anfang einer Kette (`/chain-1` → `/chain-2`
/// → `/chain-3`) und einen langsamen Schwanz `/tail-1` → `/tail-2` → … (je 120 ms), damit der Crawl noch läuft,
/// wenn die Flags in der Datei stehen.
fn flag_site() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let page = |links: &[String]| {
        let mut body = String::from("<!doctype html><html><head><title>t</title></head><body>");
        for l in links {
            body.push_str(&format!("<a href=\"{l}\">{l}</a>"));
        }
        body.push_str("</body></html>");
        body.replace('"', "\\\"")
    };
    let mut toml = String::new();
    let mut route = |path: &str, extra: &str| {
        toml.push_str(&format!("[[route]]\npath = \"{path}\"\n{extra}\n\n"));
    };
    route(
        "/",
        &format!(
            "body = \"{}\"",
            page(&[
                "/loop-a".to_string(),
                "/loop-b".to_string(),
                "/chain-1".to_string(),
                "/tail-1".to_string()
            ])
        ),
    );
    route("/loop-a", "status = 301\nlocation = \"/loop-b\"");
    route("/loop-b", "status = 301\nlocation = \"/loop-a\"");
    route("/chain-1", "status = 301\nlocation = \"/chain-2\"");
    route("/chain-2", "status = 301\nlocation = \"/chain-3\"");
    route("/chain-3", &format!("body = \"{}\"", page(&[])));
    for n in 1..=30 {
        route(
            &format!("/tail-{n}"),
            &format!(
                "delay_ms = 120\nbody = \"{}\"",
                page(&[format!("/tail-{}", n + 1)])
            ),
        );
    }
    std::fs::write(dir.path().join("scenario.toml"), toml).unwrap();
    dir
}

/// Wartet, bis die Datei `want` Flags enthält (Lesen aus einer zweiten Verbindung, WAL).
async fn wait_for_flags(path: &std::path::Path, want: i64) {
    for _ in 0..400 {
        if let Ok(file) = CrawlFile::open(path, OpenMode::ReadOnly) {
            let n: i64 = file
                .conn()
                .query_row("SELECT COUNT(*) FROM url_flags", [], |r| r.get(0))
                .unwrap_or(0);
            if n >= want {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("die Flags erscheinen nicht in der Datei");
}

fn flags_in(file: &CrawlFile) -> Vec<(String, String)> {
    let mut stmt = file
        .conn()
        .prepare("SELECT u.address, f.flag FROM url_flags f JOIN urls u ON u.id = f.url_id ORDER BY u.address, f.flag")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

async fn flags_survive(pause: bool) {
    let site = flag_site();
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.limits.max_redirects = 1;
    let run = support::launch(site.path(), "/", cfg).await;
    // Schleife (2) + Limit an /chain-2 (1)
    wait_for_flags(&run.path, 3).await;
    if pause {
        run.handle.pause();
    } else {
        run.handle.stop();
    }
    let summary = run.handle.wait().await.unwrap();
    assert_eq!(
        summary.state,
        if pause {
            CrawlState::Paused
        } else {
            CrawlState::Completed
        }
    );

    let file = CrawlFile::open(&run.path, OpenMode::ReadOnly).unwrap();
    let base = run.site.base_url();
    assert_eq!(
        flags_in(&file),
        vec![
            (
                format!("{base}/chain-2"),
                "redirect_limit_reached".to_string()
            ),
            (format!("{base}/loop-a"), "redirect_loop".to_string()),
            (format!("{base}/loop-b"), "redirect_loop".to_string()),
        ]
    );
    let (queue, _) = queue_is_consistent(&file);
    assert!(queue > 0, "der Crawl wurde mittendrin beendet");
}

// spec: CRAWL-017, CRAWL-072
#[tokio::test]
async fn redirect_flags_are_in_the_file_after_a_pause() {
    flags_survive(true).await;
}

// spec: CRAWL-017, CRAWL-072
#[tokio::test]
async fn redirect_flags_are_in_the_file_after_a_stop() {
    flags_survive(false).await;
}
