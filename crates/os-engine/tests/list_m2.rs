use os_engine::{list_input, start_crawl, testing, CrawlRequest, CrawlSource};
use os_model::{CrawlConfig, CrawlMode};
use os_store::{CrawlFile, OpenMode};

async fn run(
    mut cfg: CrawlConfig,
    items: &[&str],
    spider: bool,
) -> (os_testsite::TestSite, tempfile::TempDir, CrawlFile) {
    let site = os_testsite::TestSite::start("crawl-m2-list").await.unwrap();
    cfg.speed.threads = 1;
    let deps = testing::test_deps();
    let fetch = testing::site_fetcher(&site, &cfg, &deps);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.ospider");
    start_crawl(
        fetch,
        CrawlRequest {
            config: cfg,
            source: if spider {
                CrawlSource::Spider {
                    start_url: items[0].into(),
                }
            } else {
                CrawlSource::List {
                    items: items.iter().map(|s| s.to_string()).collect(),
                    label: "fixture".into(),
                }
            },
            file: path.clone(),
            project: "test".into(),
            name: "test".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    (site, dir, file)
}

// spec: CRAWL-030
// spec: OUT-085 [M2]
#[tokio::test]
async fn redirects_follow_beyond_depth_only_when_enabled_and_keep_hop_limit() {
    for (enabled, max, expected) in [(false, 10, 1), (true, 10, 3), (true, 1, 2)] {
        let mut cfg = list_input::list_config(CrawlConfig::default());
        cfg.advanced.always_follow_redirects = enabled;
        cfg.limits.max_redirects = max;
        let (site, _dir, file) = run(cfg, &["http://site.test/a"], false).await;
        assert_eq!(
            site.requests()
                .iter()
                .filter(|r| matches!(r.path_and_query.as_str(), "/a" | "/b" | "/c"))
                .count(),
            expected
        );
        assert!(!site
            .requests()
            .iter()
            .any(|r| r.path_and_query == "/unrelated"));
        assert_eq!(
            file.conn()
                .query_row(
                    "SELECT COUNT(*) FROM urls WHERE status_code IS NOT NULL",
                    [],
                    |r| r.get::<_, usize>(0)
                )
                .unwrap(),
            expected
        );
    }
}

// spec: CRAWL-030
#[tokio::test]
async fn canonical_and_external_chains_are_parsed_and_spider_obeys_the_option() {
    for spider in [false, true] {
        let mut cfg = list_input::list_config(CrawlConfig::default());
        if spider {
            cfg.mode = CrawlMode::Spider;
        }
        cfg.advanced.always_follow_canonicals = true;
        cfg.advanced.always_follow_redirects = true;
        cfg.spider.external.crawl = false;
        let (site, _dir, file) = run(cfg.clone(), &["http://site.test/p1"], spider).await;
        for path in ["/p1", "/p2", "/p3", "/p4"] {
            assert_eq!(
                site.requests()
                    .iter()
                    .filter(|r| r.path_and_query == path)
                    .count(),
                1,
                "{path}"
            );
        }
        assert_eq!(
            file.conn()
                .query_row(
                    "SELECT canonical_url FROM urls WHERE address='http://other.test/p3'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "http://other.test/p4"
        );
        let (site, _dir, _) = run(cfg, &["http://site.test/ext"], spider).await;
        for path in ["/ext", "/a", "/b", "/c"] {
            assert!(
                site.requests().iter().any(|r| r.path_and_query == path),
                "{path}"
            );
        }
    }
}

// spec: CRAWL-029
#[tokio::test]
async fn upload_rows_preserve_duplicates_raw_case_and_excluded_status_for_export() {
    let text = include_str!("../../../tests/fixtures/crawl-m2-list/dupes.txt");
    let inputs: Vec<_> = text.lines().collect();
    let mut cfg = list_input::list_config(CrawlConfig::default());
    cfg.scope.exclude = vec![".*/privat/.*".into()];
    let (_site, _dir, file) = run(cfg, &inputs, false).await;
    let mut stmt = file.conn().prepare("SELECT l.original,u.address,u.status_code,l.outcome FROM list_input l LEFT JOIN urls u ON u.id=l.url_id ORDER BY l.position").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<u16>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        inputs
    );
    assert_eq!(rows[3].1.as_deref(), Some("http://site.test/a"));
    assert_eq!(rows[3].2, rows[0].2);
    assert_eq!(rows[2].2, None);
    assert_eq!(rows[2].3, "excluded");
}

// spec: CRAWL-030
#[tokio::test]
async fn disabled_canonicals_rules_url_limits_and_loops_remain_bounded() {
    let cfg = list_input::list_config(CrawlConfig::default());
    let (site, _dir, _) = run(cfg.clone(), &["http://site.test/p1"], false).await;
    assert!(!site.requests().iter().any(|r| r.path_and_query == "/p2"));
    let mut enabled = cfg;
    enabled.advanced.always_follow_redirects = true;
    enabled.scope.exclude = vec![".*/b$".into()];
    let (site, _dir, _) = run(enabled.clone(), &["http://site.test/a"], false).await;
    assert!(!site.requests().iter().any(|r| r.path_and_query == "/b"));
    enabled.scope.exclude.clear();
    enabled.limits.max_urls = 1;
    let (site, _dir, _) = run(enabled.clone(), &["http://site.test/a"], false).await;
    assert!(!site.requests().iter().any(|r| r.path_and_query == "/b"));
    enabled.limits.max_urls = 0;
    let (site, _dir, _) = run(enabled, &["http://site.test/loop"], false).await;
    assert_eq!(
        site.requests()
            .iter()
            .filter(|r| r.path_and_query == "/loop")
            .count(),
        1
    );
}

// spec: CRAWL-030
#[tokio::test]
async fn canonical_chain_respects_the_hop_limit() {
    let mut cfg = list_input::list_config(CrawlConfig::default());
    cfg.advanced.always_follow_canonicals = true;
    cfg.limits.max_redirects = 1;
    let (site, _dir, _) = run(cfg, &["http://site.test/p1"], false).await;
    assert!(site.requests().iter().any(|r| r.path_and_query == "/p2"));
    assert!(!site.requests().iter().any(|r| r.path_and_query == "/p3"));
}

// spec: CRAWL-030
#[tokio::test]
async fn resuming_a_limited_list_keeps_external_canonicals_and_the_hop_budget() {
    for max_hops in [2, 10] {
        let mut cfg = list_input::list_config(CrawlConfig::default());
        cfg.advanced.always_follow_canonicals = true;
        cfg.spider.external.crawl = false;
        cfg.limits.max_redirects = max_hops;
        cfg.limits.max_urls = 2;
        let (site, dir, file) = run(cfg.clone(), &["http://site.test/p1"], false).await;
        drop(file);
        cfg.limits.max_urls = 0;
        let deps = testing::test_deps();
        os_engine::resume_crawl(
            testing::site_fetcher(&site, &cfg, &deps),
            dir.path().join("test.ospider"),
            Some(cfg),
            deps,
        )
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
        assert!(site.requests().iter().any(|r| r.path_and_query == "/p3"));
        assert_eq!(
            site.requests().iter().any(|r| r.path_and_query == "/p4"),
            max_hops > 2
        );
    }
}
