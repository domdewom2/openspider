//! Infrastruktur-Nachweise der Fix-Runde M2-F05a.
use os_engine::testing::{test_deps, SimFetcher, SimPage, SimSite};
use os_engine::{resume_crawl, start_crawl, CrawlRequest, CrawlSource};
use os_model::CrawlConfig;
use os_store::{CrawlFile, OpenMode};

fn request(config: CrawlConfig, file: std::path::PathBuf) -> CrawlRequest {
    CrawlRequest {
        config,
        file,
        source: CrawlSource::Spider {
            start_url: "https://site.test/".into(),
        },
        project: "fixture".into(),
        name: "w1".into(),
    }
}

// spec: CRAWL-125
// Infrastruktur (M2-F05a) für CRAWL-125; Fachprüfung M2-E02.
#[tokio::test(start_paused = true)]
async fn resume_discovers_new_edges_after_the_persisted_maximum_sequence() {
    let site = SimSite::new()
        .page("https://site.test/", "<a href='/a'>a</a>")
        .page("https://site.test/a", "<a href='/b'>b</a>")
        .page("https://site.test/b", "<p>done</p>");
    let fetcher = SimFetcher::new(site);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("resume.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.limits.max_urls = 1;
    start_crawl(
        fetcher.clone(),
        request(cfg.clone(), path.clone()),
        test_deps(),
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    // Sequenz absichtlich oberhalb der Queue-/Meta-Sequenz, wie nach früheren Entdeckungen.
    file.conn()
        .execute("UPDATE discovery_edges SET seq = seq + 100000", [])
        .unwrap();
    let before: u64 = file
        .conn()
        .query_row("SELECT MAX(seq) FROM discovery_edges", [], |r| r.get(0))
        .unwrap();
    let old_count: u64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM discovery_edges", [], |r| r.get(0))
        .unwrap();
    drop(file);
    cfg.limits.max_urls = 0;
    resume_crawl(fetcher, path.clone(), Some(cfg), test_deps())
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let (count, distinct, added): (u64, u64, u64) = file
        .conn()
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT seq), SUM(seq > ?1) FROM discovery_edges",
            [before],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(count, distinct);
    assert_eq!(count, old_count + 1);
    assert_eq!(added, 1);
}

// spec: TECH-071, TECH-210
// Infrastruktur (M2-F05a) für TECH-071/210; Fachprüfung M2-D07/E02.
#[tokio::test(start_paused = true)]
async fn truncated_sitemap_and_non_html_hreflang_reach_the_file_without_zero_facts() {
    let alternates = (0..502)
        .map(|i| {
            format!(
                "<xhtml:link rel='alternate' hreflang='x-{i}' href='https://site.test/alt-{i}'/>"
            )
        })
        .collect::<String>();
    let xml = format!("<urlset xmlns='http://www.sitemaps.org/schemas/sitemap/0.9' xmlns:xhtml='http://www.w3.org/1999/xhtml'><url><loc>https://site.test/doc.pdf</loc>{alternates}</url><url><loc>https://site.test/plain</loc></url></urlset>");
    let links = (0..503)
        .map(|i| format!("<https://site.test/alt-{i}>; rel=alternate; hreflang=x-{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let site = SimSite::new()
        .page("https://site.test/", "<p>root</p>")
        .asset("https://site.test/sitemap.xml", "application/xml", &xml)
        .with(
            "https://site.test/doc.pdf",
            SimPage {
                status: 200,
                headers: vec![
                    ("content-type".into(), "application/pdf".into()),
                    ("link".into(), links),
                ],
                body: "%PDF-fixture".into(),
                ..Default::default()
            },
        )
        .page("https://site.test/plain", "<p>plain</p>");
    let mut cfg = CrawlConfig::default();
    cfg.sitemaps.urls = vec!["https://site.test/sitemap.xml".into()];
    cfg.spider.hreflang.crawl = false;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.ospider");
    start_crawl(
        SimFetcher::new(site),
        request(cfg, path.clone()),
        test_deps(),
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    for (key, value) in [("hreflang.truncated_sitemap", 2), ("hreflang.truncated", 3)] {
        let actual: i64 = file.conn().query_row("SELECT e.v_int FROM url_ext e JOIN urls u ON u.id=e.url_id WHERE e.key=?1 AND u.url='https://site.test/doc.pdf'", [key], |r| r.get(0)).unwrap();
        assert_eq!(actual, value);
    }
    let zeros: i64 = file.conn().query_row("SELECT COUNT(*) FROM url_ext WHERE (key LIKE 'hreflang.truncated%' AND v_int=0) OR key='http.attempts'", [], |r| r.get(0)).unwrap();
    assert_eq!(zeros, 0);
}
