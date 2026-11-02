//! M2-N28: Der Fortschrittszähler der Engine (`Progress::discovered`; Statusleiste, MCP, CLI-Fortschritt) zählt
//! dieselben gefundenen URLs wie Overview und Bericht (`os_store::counters::encountered`, UI-061, CRAWL-130):
//! ohne die robots.txt-Zeilen (K-54) und Sitemap-Dateien, mit verborgenen URLs. Auch nach dem Fortsetzen, wenn
//! das Seen-Set aus der Crawl-Datei entsteht.

use os_engine::testing::{test_deps, SimFetcher, SimSite};
use os_engine::{resume_crawl, start_crawl, CrawlHandle, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, Progress};
use os_store::{CrawlFile, OpenMode};

fn site() -> SimSite {
    SimSite::new()
        .asset(
            "https://site.test/robots.txt",
            "text/plain",
            "User-agent: *\nDisallow: /privat/\n",
        )
        .asset(
            "https://site.test/sitemap.xml",
            "application/xml",
            "<urlset xmlns='http://www.sitemaps.org/schemas/sitemap/0.9'>\
             <url><loc>https://site.test/nur-sitemap</loc></url>\
             <url><loc>https://site.test/a</loc></url></urlset>",
        )
        .page(
            "https://site.test/",
            "<a href='/a'>a</a><a href='/b'>b</a><a href='/ausgeschlossen'>x</a>\
             <a href='/privat/x'>p</a><a href='https://other.test/ext'>e</a>",
        )
        .page("https://site.test/a", "<a href='/c'>c</a>")
        .page("https://site.test/b", "<p>b</p>")
        .page("https://site.test/c", "<p>c</p>")
        .page("https://site.test/nur-sitemap", "<p>s</p>")
        .asset(
            "https://other.test/robots.txt",
            "text/plain",
            "User-agent: *\n",
        )
        .page("https://other.test/ext", "<p>e</p>")
}

fn config(max_urls: u64) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.limits.max_urls = max_urls;
    cfg.scope.exclude = vec![".*/ausgeschlossen".into()];
    cfg.robots.show_blocked_internal = false;
    cfg.sitemaps.urls = vec!["https://site.test/sitemap.xml".into()];
    cfg
}

/// Letzter Fortschrittswert der Engine nach dem Ende (Pause, Limit oder Abschluss).
async fn finish(handle: CrawlHandle) -> Progress {
    let rx = handle.watch();
    handle.wait().await.unwrap();
    let p = rx.borrow().clone();
    p
}

fn counts(path: &std::path::Path) -> (u64, u64, u64) {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let all: u64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    let auxiliary: u64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE in_tables = 0 AND (discovered_via = 'robots_txt' \
             OR id IN (SELECT url_id FROM sitemaps WHERE url_id IS NOT NULL))",
            [],
            |r| r.get(0),
        )
        .unwrap();
    (
        os_store::counters::encountered(file.conn()).unwrap(),
        all,
        auxiliary,
    )
}

// spec: UI-061
// spec: CRAWL-130
// spec: CRAWL-098
#[tokio::test(start_paused = true)]
async fn engine_progress_counts_encountered_urls_like_the_crawl_file_before_and_after_resume() {
    let fetcher = SimFetcher::new(site());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n28.ospider");
    let request = CrawlRequest {
        config: config(2),
        file: path.clone(),
        source: CrawlSource::Spider {
            start_url: "https://site.test/".into(),
        },
        project: "fixture".into(),
        name: "n28".into(),
    };
    // URL-Limit: Ende mit Rest in der Warteschlange, danach Fortsetzen ohne Limit.
    let first = finish(
        start_crawl(fetcher.clone(), request, test_deps())
            .await
            .unwrap(),
    )
    .await;
    let (encountered, all, auxiliary) = counts(&path);
    assert!(auxiliary >= 1, "robots.txt-Zeile vor dem Fortsetzen");
    assert_eq!(encountered, all - auxiliary);
    assert_eq!(first.discovered, encountered, "Limit erreicht");

    let resumed = finish(
        resume_crawl(fetcher, path.clone(), Some(config(0)), test_deps())
            .await
            .unwrap(),
    )
    .await;
    let (encountered, all, auxiliary) = counts(&path);
    // robots.txt von site.test und other.test, die Sitemap-Datei
    assert_eq!(auxiliary, 3);
    assert_eq!(encountered, all - auxiliary);
    // /, /a, /b, /c, /nur-sitemap, other.test/ext, verborgen: /ausgeschlossen, /privat/x
    assert_eq!(encountered, 8);
    assert_eq!(resumed.discovered, encountered, "nach dem Fortsetzen");
}
