//! Link-instance transport uses the same assembly path in text and rendering modes.
use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;

// spec: CRAWL-053
// spec: TECH-143
#[tokio::test]
async fn uncrawlable_capture_keeps_raw_schemes_without_fetching_or_creating_url_rows() {
    for capture in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.spider.store_uncrawlable_links = capture;
        let fx = crawl_fixture_from("tech-m2-links", "http://site.test/", cfg).await;
        let db = fx.file.conn();
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM links WHERE target_raw='mailto:hello@site.test' AND target_id IS NULL AND crawlable=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, i64::from(capture));
        let unsupported: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM urls WHERE url LIKE 'mailto:%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unsupported, 0);
        assert!(!fx
            .site
            .requests()
            .iter()
            .any(|r| r.path_and_query == "/hidden.html"));
        let reasons: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM links l JOIN urls u ON u.id=l.target_id WHERE u.url='http://site.test/hidden.html' AND l.crawlable=0 AND l.uncrawlable_reason IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reasons, if capture { 2 } else { 0 });
    }
}

// spec: CRAWL-055
// spec: ONPAGE-033
// spec: ONPAGE-037
#[tokio::test]
async fn srcset_switch_preserves_each_candidate_and_its_raw_protocol() {
    for extract in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.extraction.srcset = extract;
        cfg.spider.external.crawl = false;
        let fx = crawl_fixture_from("onpage-m2-security", "https://site.test/", cfg).await;
        let mut stmt = fx.file.conn().prepare(
            "SELECT l.target_raw FROM links l JOIN urls s ON s.id=l.source_id WHERE s.url='https://site.test/https/resources.html' AND l.link_type='Image'",
        ).unwrap();
        let raw: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for candidate in [
            "http://cdn.test/one.png",
            "//cdn.test/two.png",
            "http://cdn.test/three.png",
        ] {
            assert_eq!(
                raw.iter().filter(|s| s.as_str() == candidate).count(),
                1,
                "{candidate}: {raw:?}"
            );
        }
        let visible: i64 = fx.file.conn().query_row(
            "SELECT COUNT(*) FROM urls WHERE url IN ('http://cdn.test/one.png', 'https://cdn.test/two.png', 'http://cdn.test/three.png') AND in_tables=1",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(visible, if extract { 3 } else { 0 });
        let linked_instances: i64 = fx.file.conn().query_row(
            "SELECT COUNT(*) FROM image_instances i JOIN urls s ON s.id=i.page_id WHERE s.url='https://site.test/https/resources.html' AND i.source='srcset' AND i.image_id IS NOT NULL",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(linked_instances, if extract { 3 } else { 0 });
        assert!(!fx.site.requests().iter().any(|r| r.host == "cdn.test"));
    }
}
