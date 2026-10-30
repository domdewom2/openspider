use os_engine::testing::crawl_fixture_from;
use os_export::{export_bulk, Format};
use os_model::CrawlConfig;

// spec: CRAWL-056
#[tokio::test]
async fn anchor_export_omits_nofollow_only_targets_but_source_details_keep_the_relation() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.images.crawl = false;
    let fx = crawl_fixture_from("n10-links", "http://site.test/", cfg.clone()).await;
    let dir = tempfile::tempdir().unwrap();
    let files = export_bulk(
        &fx.file,
        &cfg,
        "Links:All Anchor Text",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    let csv = std::fs::read_to_string(&files[0].path).unwrap();
    assert!(!csv.contains("nofollow-only.html"));
    assert!(csv.contains("three.html"), "followed elsewhere");
    let count: u64 = fx.file.conn().query_row("SELECT COUNT(*) FROM links l JOIN urls t ON t.id=l.target_id WHERE t.url LIKE '%nofollow-only.html'", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
}
