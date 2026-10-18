use os_analyze::views::{self, ViewQuery};
use os_model::{CrawlConfig, Value};

// spec: ONPAGE-079
#[tokio::test]
#[ignore = "wartet auf M2-E01"]
async fn fragment_rows_reuse_one_fetch_and_base_bookmarks() {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.crawl_fragments = true;
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/onpage-m2-url");
    let fx = os_engine::testing::crawl_dir_with(
        &dir,
        "http://site.test/",
        cfg.clone(),
        os_engine::testing::test_deps(),
    )
    .await;
    assert_eq!(fx.site.count("site.test", "/bookmarks.html"), 1);
    let fragments: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT count(*) FROM urls WHERE url LIKE 'http://site.test/bookmarks.html#%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(fragments >= 9);
    let base_facts: i64 = fx.file.conn().query_row("SELECT count(*) FROM url_ext e JOIN urls u ON u.id=e.url_id WHERE e.key='crawl.fragment_base' AND u.url LIKE '%bookmarks.html#%'",[],|r| r.get(0)).unwrap();
    assert!(base_facts >= 9);
    let page =
        views::query_view(&fx.file, &cfg, &ViewQuery::new("URL", "Broken Bookmark")).unwrap();
    let mut addresses: Vec<_> = page
        .rows
        .iter()
        .map(|row| match &row[0] {
            Value::Text(v) => v.clone(),
            _ => panic!("Address"),
        })
        .collect();
    addresses.sort();
    assert_eq!(
        addresses,
        [
            "http://site.test/bookmarks.html#Kontakt",
            "http://site.test/bookmarks.html#preise"
        ]
    );
}
