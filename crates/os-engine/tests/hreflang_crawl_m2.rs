use os_engine::testing;
use os_model::CrawlConfig;

// spec: TECH-087
#[tokio::test]
async fn external_alternates_are_only_parsed_when_the_cdn_list_makes_them_internal() {
    for (crawl, cdn) in [(false, false), (true, false), (true, true)] {
        let mut cfg = CrawlConfig::default();
        cfg.spider.hreflang.crawl = crawl;
        if cdn {
            cfg.cdns.push("other.test".into());
        }
        let fx = testing::crawl_fixture_from("crawl-m2-list", "http://site.test/de", cfg).await;
        let (status, title, internal): (Option<u16>, Option<String>, bool) = fx.file.conn().query_row(
            "SELECT status_code,title_1,is_internal FROM urls WHERE address='http://other.test/fr'", [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(status, crawl.then_some(200));
        assert_eq!(internal, cdn);
        assert_eq!(title.as_deref(), cdn.then_some("Français"));
        let backlinks: u64 = fx.file.conn().query_row("SELECT COUNT(*) FROM hreflang WHERE url_id=(SELECT id FROM urls WHERE address='http://other.test/fr')", [], |r| r.get(0)).unwrap();
        assert_eq!(backlinks, u64::from(cdn));
        assert_eq!(
            fx.site.requests().iter().any(|r| r.path_and_query == "/fr"),
            crawl
        );
    }
}
