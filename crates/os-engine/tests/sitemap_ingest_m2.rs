use os_engine::{
    testing::{test_deps, SimFetcher, SimSite},
    CrawlRequest, CrawlSource,
};
use os_model::CrawlConfig;

async fn run(
    cfg: CrawlConfig,
    site: SimSite,
) -> (
    tempfile::TempDir,
    os_store::CrawlFile,
    std::sync::Arc<SimFetcher>,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let fetcher = SimFetcher::new(site);
    os_engine::start_crawl(
        fetcher.clone(),
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: path.clone(),
            project: "fixture".into(),
            name: "sitemap".into(),
        },
        test_deps(),
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    (
        dir,
        os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap(),
        fetcher,
    )
}
fn site() -> SimSite {
    let xml = include_str!("../../../tests/fixtures/crawl-m2-sitemaps/extensions.xml");
    SimSite::new()
        .page("https://site.test/", "<a href='/linked'>linked</a>")
        .page("https://site.test/linked", "<a href='/deep'>deep</a>")
        .page("https://site.test/deep", "done")
        .asset(
            "https://site.test/robots.txt",
            "text/plain",
            "User-agent: *\nSitemap: /s.xml\n",
        )
        .asset("https://site.test/s.xml", "application/xml", xml)
}
// spec: CRAWL-058, CRAWL-059, CRAWL-060
#[tokio::test]
async fn discovery_deduplicates_and_stores_metadata_after_link_frontier() {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.sitemaps.crawl_linked = true;
    cfg.sitemaps.urls = vec!["https://site.test/s.xml".into()];
    let (_dir, file, fetcher) = run(cfg, site()).await;
    assert_eq!(fetcher.count("https://site.test/s.xml"), 1);
    let row: (String, f64, i64, i64, i64) = file.conn().query_row("SELECT changefreq, priority, images, videos, news FROM sitemap_entries WHERE changefreq IS NOT NULL", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
    assert_eq!(row, ("daily".into(), 0.7, 1, 1, 1));
    let ext: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM sitemap_entry_ext", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ext, 3);
    let visible: i64 = file
        .conn()
        .query_row(
            "SELECT in_tables FROM urls WHERE url='https://site.test/s.xml'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(visible, 0);
    let req = fetcher.page_requests();
    let deep = req
        .iter()
        .position(|r| r.url == "https://site.test/deep")
        .unwrap();
    let added = req
        .iter()
        .position(|r| r.url == "https://site.test/second")
        .unwrap();
    assert!(deep < added);
    let depth: Option<i64> = file
        .conn()
        .query_row(
            "SELECT crawl_depth FROM urls WHERE url='https://site.test/second'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(depth, None);
}
// spec: CRAWL-058
#[tokio::test]
async fn disabled_discovery_makes_no_sitemap_requests() {
    let (_dir, _file, fetcher) = run(CrawlConfig::default(), site()).await;
    assert_eq!(fetcher.count("https://site.test/s.xml"), 0);
}
// spec: CRAWL-060 [M2]
#[tokio::test]
async fn partial_xml_is_written_with_error_and_complete_entries() {
    let mut cfg = CrawlConfig::default();
    cfg.sitemaps.urls = vec!["https://site.test/broken.xml".into()];
    let (_dir, file, _) = run(
        cfg,
        site().asset(
            "https://site.test/broken.xml",
            "application/xml",
            "<urlset><url><loc>https://site.test/valid</loc></url><url>",
        ),
    )
    .await;
    let (entries, error): (i64, Option<String>) = file
        .conn()
        .query_row("SELECT entries,error FROM sitemaps", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(entries, 1);
    assert!(error.is_some());
}
// spec: TECH-210 [M2]
#[tokio::test]
async fn cross_submission_requires_target_robots_and_matching_scheme_and_port() {
    for authorized in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.sitemaps.urls = vec!["https://cdn.test/s.xml".into()];
        let robots = if authorized {
            "Sitemap: https://cdn.test/s.xml"
        } else {
            "Sitemap: https://cdn.test/other.xml"
        };
        let xml="<urlset><url><loc>https://site.test/cross</loc></url><url><loc>http://site.test/scheme</loc></url><url><loc>https://site.test:444/port</loc></url></urlset>";
        let (_dir, file, fetcher) = run(
            cfg,
            site()
                .asset("https://site.test/robots.txt", "text/plain", robots)
                .asset("https://cdn.test/s.xml", "application/xml", xml),
        )
        .await;
        assert_eq!(
            fetcher.count("https://site.test/cross"),
            usize::from(authorized)
        );
        assert_eq!(fetcher.count("http://site.test/scheme"), 0);
        assert_eq!(fetcher.count("https://site.test:444/port"), 0);
        let invalid: i64 = file
            .conn()
            .query_row("SELECT invalid_entries FROM sitemaps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(invalid, if authorized { 2 } else { 3 });
    }
}
