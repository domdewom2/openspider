use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, Value};

// spec: ONPAGE-038, ONPAGE-039, ONPAGE-040, ONPAGE-041, ONPAGE-042
#[tokio::test]
async fn header_filters_include_all_2xx_content_types_but_keep_hsts_population() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-security", "https://site.test/", cfg.clone()).await;
    for status in [200, 204, 299, 301, 404, 500] {
        fx.file
            .conn()
            .execute(
                "UPDATE urls SET status_code=?1 WHERE url='https://site.test/image.jpg'",
                [status],
            )
            .unwrap();
        for filter in [
            "Missing HSTS Header",
            "Missing Content-Security-Policy Header",
            "Missing X-Content-Type-Options Header",
            "Missing X-Frame-Options Header",
            "Missing Secure Referrer-Policy Header",
        ] {
            let mut q = ViewQuery::new("Security", filter);
            q.columns = ColumnSelection::Named(vec!["Address".into()]);
            q.limit = 500;
            let got = query_view(&fx.file, &cfg, &q)
                .unwrap()
                .rows
                .into_iter()
                .any(|r| r[0] == Value::from("https://site.test/image.jpg"));
            assert_eq!(
                got,
                filter == "Missing HSTS Header" || status < 300,
                "{filter} {status}"
            );
        }
    }
}
