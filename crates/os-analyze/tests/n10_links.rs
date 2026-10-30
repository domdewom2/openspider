use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, Value};

fn addresses(file: &os_store::CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new("Links", filter);
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    q.limit = 500;
    query_view(file, cfg, &q)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| {
            let Value::Text(s) = &r[0] else {
                panic!("address")
            };
            s.clone()
        })
        .collect()
}

// spec: TECH-132
// spec: TECH-133
#[tokio::test]
async fn followed_depth_and_nofollow_outlinks_drive_the_filters() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.images.crawl = false;
    let fx = crawl_fixture_from("n10-links", "http://site.test/", cfg.clone()).await;
    assert_eq!(
        addresses(&fx.file, &cfg, "Pages With High Crawl Depth"),
        vec!["http://site.test/four.html"]
    );
    assert!(
        addresses(&fx.file, &cfg, "Pages Without Internal Outlinks").is_empty(),
        "nofollow links count as outlinks"
    );
    cfg.links.high_crawl_depth = 3;
    assert_eq!(
        addresses(&fx.file, &cfg, "Pages With High Crawl Depth").len(),
        2
    );
}

// spec: TECH-137
#[tokio::test]
async fn external_threshold_counts_follow_instances_including_duplicates() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.external.crawl = false;
    let fx = crawl_fixture_from("n10-threshold", "http://site.test/", cfg.clone()).await;
    let got = addresses(&fx.file, &cfg, "Pages With High External Outlinks");
    assert!(
        got.iter().any(|u| u.ends_with("external-101.html")),
        "{got:?}"
    );
    cfg.links.high_external_outlinks = 99;
    assert!(
        addresses(&fx.file, &cfg, "Pages With High External Outlinks")
            .iter()
            .any(|u| u.ends_with("external-99.html"))
    );
}
