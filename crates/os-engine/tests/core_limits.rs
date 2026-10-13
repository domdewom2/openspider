//! Limits und Betrieb des Kerns gegen echte Fixtures: URL-Limit (CRAWL-066), Spider-Modus (CRAWL-026),
//! Threads gegen den Server (CRAWL-112), Fortschritt am Ende (CRAWL-130), Rendering-Modus (CRAWL-077).

#[path = "core_support.rs"]
mod support;

use std::time::Duration;

use os_engine::testing::{crawl_dir_with, crawl_fixture, crawl_fixture_from, test_deps};
use os_engine::{start_crawl, CrawlRequest, CrawlSource, EngineError};
use os_model::{CookieMode, CrawlConfig, CrawlState, EndReason, RenderingMode};
use support::*;

// ---------------------------------------------------------------------------------------------------
// CRAWL-026

// spec: CRAWL-026
#[tokio::test]
async fn hundred_connected_pages_give_exactly_one_hundred_html_rows_and_the_crawl_ends_by_itself() {
    let fx = crawl_fixture("crawl-site-100", CrawlConfig::default()).await;
    assert_eq!(fx.summary.state, CrawlState::Completed);
    assert_eq!(fx.summary.end_reason, Some(EndReason::QueueEmpty));
    let html = scalar(
        &fx,
        "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND in_tables = 1 AND content_class = 'HTML' AND status_code = 200",
    );
    assert_eq!(html, 100, "Intern › HTML");
    // Metadaten am Ende
    let meta = fx.file.meta().unwrap();
    assert_eq!(meta.state, CrawlState::Completed);
    assert_eq!(meta.end_reason, Some(EndReason::QueueEmpty));
    assert!(meta.started_at.is_some() && meta.ended_at.is_some() && meta.heartbeat_at.is_some());
    // Warteschlange ist leer, alle URLs sind abgearbeitet oder bewusst nicht gecrawlt
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM queue"), 0);
    assert_eq!(
        scalar(&fx, "SELECT COUNT(*) FROM urls WHERE state = 'queued'"),
        0
    );
}

// spec: CRAWL-026
#[tokio::test]
async fn start_records_owner_and_process_and_the_pages_carry_titles_and_links() {
    let fx = crawl_fixture("crawl-site-100", CrawlConfig::default()).await;
    assert_eq!(
        fx.file.get_meta("owner_kind").unwrap().as_deref(),
        Some("cli")
    );
    assert_eq!(
        fx.file.get_meta("owner_pid").unwrap(),
        Some(std::process::id().to_string())
    );
    // Seite 1: Titel, drei Kinder + Start als Hyperlinks, ein Bild
    let (title, h1, outlinks): (String, String, i64) = fx
        .file
        .conn()
        .query_row(
            "SELECT title_1, h1_1, outlinks FROM urls WHERE url LIKE '%/page-1.html'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((title.as_str(), h1.as_str()), ("Seite 1", "Seite 1"));
    assert_eq!(outlinks, 4, "Start + Seiten 4, 5, 6");
    let images = scalar(
        &fx,
        "SELECT COUNT(*) FROM image_instances WHERE page_id = (SELECT id FROM urls WHERE url LIKE '%/page-1.html') AND image_id IS NOT NULL",
    );
    assert_eq!(images, 1);
    // Antwort-Header werden gespeichert (K-13), Hash und Antwortzeit gesetzt
    assert!(
        scalar(
            &fx,
            "SELECT COUNT(*) FROM http_headers WHERE direction = 'response'"
        ) > 100
    );
    assert_eq!(
        scalar(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE status_code = 200 AND hash_md5 IS NULL"
        ),
        0
    );
    // K-02: übertragene Bytes insgesamt = übertragene Bytes
    assert_eq!(
        scalar(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE total_transferred_bytes != transferred_bytes"
        ),
        0
    );
}

// spec: CRAWL-109, CRAWL-110
#[tokio::test]
async fn the_start_url_is_crawled_regardless_of_include_and_exclude() {
    let mut cfg = CrawlConfig::default();
    cfg.scope.include = vec![".*/page-1\\.html".to_string()];
    cfg.scope.exclude = vec![".*/page-4\\.html".to_string()];
    let fx = crawl_fixture("crawl-site-100", cfg).await;
    assert_eq!(
        row_of(&fx, &format!("{}/", fx.site.base_url())).status_code,
        Some(200)
    );
    assert_eq!(
        row_of(&fx, &format!("{}/page-1.html", fx.site.base_url())).status_code,
        Some(200)
    );
    let r2 = row_of(&fx, &format!("{}/page-2.html", fx.site.base_url()));
    assert_eq!(
        (r2.reason.as_deref(), r2.status_code),
        (Some("not_included"), None)
    );
    // page-4 ist nur über page-1 erreichbar und ausgeschlossen
    let r4 = row_of(&fx, &format!("{}/page-4.html", fx.site.base_url()));
    assert_eq!(r4.reason.as_deref(), Some("excluded"));
    // Include-Treffer und Exclude-Zähler stehen in crawl_counters
    assert!(
        scalar(
            &fx,
            "SELECT value FROM crawl_counters WHERE key = 'excluded_skipped'"
        ) >= 1
    );
    assert!(
        scalar(
            &fx,
            "SELECT value FROM crawl_counters WHERE key = 'include_hits:1'"
        ) >= 1
    );
}

// spec: CRAWL-088
#[tokio::test]
async fn the_cookie_session_is_carried_along_to_the_redirect_target() {
    let fx = crawl_fixture_from("crawl-cookies", "/set", CrawlConfig::default()).await;
    let check = fx
        .site
        .requests()
        .into_iter()
        .find(|r| r.path() == "/check")
        .expect("Weiterleitungsziel /check abgerufen");
    let cookie = check.header("cookie").unwrap_or_default();
    assert!(cookie.contains("sid=abc123"), "Cookie fehlt: {cookie:?}");

    // ohne Cookies wird keiner gesendet
    let mut cfg = CrawlConfig::default();
    cfg.advanced.cookies = CookieMode::None;
    let fx = crawl_fixture_from("crawl-cookies", "/set", cfg).await;
    let check = fx
        .site
        .requests()
        .into_iter()
        .find(|r| r.path() == "/check")
        .unwrap();
    assert_eq!(check.header("cookie"), None);
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-066

fn page_requests(fx: &os_engine::testing::FixtureCrawl) -> usize {
    requested_paths(fx, None).len()
}

// spec: CRAWL-066
#[tokio::test]
async fn limit_ten_on_a_hundred_page_site_makes_exactly_ten_requests_even_with_many_threads() {
    for threads in [1, 5, 20] {
        let mut cfg = CrawlConfig::default();
        cfg.limits.max_urls = 10;
        cfg.speed.threads = threads;
        let fx = crawl_fixture("crawl-site-100", cfg).await;
        assert_eq!(page_requests(&fx), 10, "{threads} Threads: Request-Log");
        assert_eq!(
            fx.summary.end_reason,
            Some(EndReason::LimitReached),
            "{threads} Threads"
        );
        assert_eq!(fx.summary.state, CrawlState::Completed);
        assert_eq!(fx.summary.crawled, 10);
        // die robots.txt zählt nicht mit: sie wurde zusätzlich abgerufen
        let robots = fx
            .site
            .requests()
            .iter()
            .filter(|r| r.path() == "/robots.txt")
            .count();
        assert_eq!(robots, 1, "robots.txt einmal je Origin");
        // der Rest der Warteschlange bleibt in der Datei (Fortsetzen nach höherem Limit)
        assert!(scalar(&fx, "SELECT COUNT(*) FROM queue") > 0);
    }
}

// spec: CRAWL-066
#[tokio::test]
async fn redirect_targets_count_towards_the_limit() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_urls = 2;
    let fx = crawl_fixture_from("crawl-redirects", "http://site.test/r301", cfg).await;
    // /r301 und sein Ziel /ziel: zwei Abrufe, danach Ende
    assert_eq!(page_requests(&fx), 2);
    assert_eq!(row_of(&fx, "http://site.test/ziel").status_code, Some(200));

    let mut cfg = CrawlConfig::default();
    cfg.limits.max_urls = 1;
    let fx = crawl_fixture_from("crawl-redirects", "http://site.test/r301", cfg).await;
    assert_eq!(page_requests(&fx), 1, "Ziel wäre der zweite Abruf");
    assert_eq!(fx.summary.end_reason, Some(EndReason::LimitReached));
    assert_eq!(
        row_of(&fx, "http://site.test/ziel").state,
        "queued",
        "bleibt in der Warteschlange"
    );
    assert_eq!(
        row_of(&fx, "http://site.test/r301").redirect_url.as_deref(),
        Some("http://site.test/ziel")
    );
}

// spec: CRAWL-066
#[tokio::test]
async fn a_limit_of_zero_means_unlimited() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_urls = 0;
    let fx = crawl_fixture("crawl-site-100", cfg).await;
    assert_eq!(fx.summary.end_reason, Some(EndReason::QueueEmpty));
    assert!(fx.summary.crawled >= 100);
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-112

/// Kopie der Fixture `crawl-site-100` mit `delay_ms` auf jeder Seite (echte Zeit, deshalb kurz).
fn delayed_copy(delay_ms: u64) -> tempfile::TempDir {
    let src = os_testsite::fixtures_root().join("crawl-site-100");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&src, dir.path());
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

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

// spec: CRAWL-112
#[tokio::test]
async fn one_thread_never_causes_two_simultaneous_requests_and_five_threads_stay_at_five() {
    let dir = delayed_copy(15);
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    let fx = crawl_dir_with(dir.path(), "/", cfg, test_deps()).await;
    assert_eq!(fx.site.max_concurrency(), 1, "Threads 1");
    assert!(fx.summary.crawled >= 100);

    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 5;
    let fx = crawl_dir_with(dir.path(), "/", cfg, test_deps()).await;
    let max = fx.site.max_concurrency();
    assert!(
        (2..=5).contains(&max),
        "Threads 5: höchstens 5, gemessen {max}"
    );
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-130

// spec: CRAWL-130
#[tokio::test]
async fn crawled_counter_equals_the_rows_with_a_status_code_and_the_final_progress_is_complete() {
    let fx = crawl_fixture("crawl-site-100", CrawlConfig::default()).await;
    let rows_with_status = scalar(
        &fx,
        "SELECT COUNT(*) FROM urls WHERE status_code IS NOT NULL AND discovered_via != 'robots_txt'",
    );
    assert_eq!(fx.summary.crawled as i64, rows_with_status);
    // 100 Seiten; die 16 Ressourcen kommen hinzu, sobald die Speichern/Crawlen-Matrix (P15b) sie abruft
    assert!(rows_with_status >= 100, "{rows_with_status}");
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-077

// spec: CRAWL-077
#[tokio::test]
async fn text_mode_needs_no_browser_and_javascript_mode_is_refused_without_a_silent_fallback() {
    // Die Engine hängt nicht an os-render: im Textmodus kann kein Browserprozess entstehen.
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    assert!(!manifest.contains("os-render") && !manifest.contains("chromiumoxide"));

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("crawl.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    let fetcher = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new());
    let result = start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: file.clone(),
            project: "t".to_string(),
            name: "js".to_string(),
        },
        test_deps(),
    )
    .await;
    match result {
        Err(EngineError::Invalid(msg)) => assert!(msg.contains("JavaScript"), "{msg}"),
        other => panic!(
            "JavaScript-Modus muss abgewiesen werden, war: {:?}",
            other.map(|_| ())
        ),
    }
    assert!(!file.exists(), "es wird kein Crawl angelegt");
}

// spec: CRAWL-001
#[tokio::test]
async fn invalid_start_urls_and_missing_files_are_rejected_before_anything_is_created() {
    for bad in ["htp:/x", "", "ftp://site.test/"] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("crawl.ospider");
        let fetcher = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new());
        let result = start_crawl(
            fetcher,
            CrawlRequest {
                config: CrawlConfig::default(),
                source: CrawlSource::Spider {
                    start_url: bad.to_string(),
                },
                file: file.clone(),
                project: "t".to_string(),
                name: "x".to_string(),
            },
            test_deps(),
        )
        .await;
        assert!(matches!(result, Err(EngineError::Invalid(_))), "{bad:?}");
        assert!(!file.exists());
    }
    // ungültige Regex in Include
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.scope.include = vec!["(".to_string()];
    let fetcher = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new());
    let result = start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".to_string(),
            },
            file: dir.path().join("c.ospider"),
            project: "t".to_string(),
            name: "x".to_string(),
        },
        test_deps(),
    )
    .await;
    match result {
        Err(EngineError::Invalid(m)) => assert!(m.contains("Include"), "{m}"),
        other => panic!("{:?}", other.map(|_| ())),
    }
    let _ = Duration::ZERO;
}
