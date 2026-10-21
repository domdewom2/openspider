//! Cross-package capture integration; the D08 logic itself is tested without ignores.
use os_model::{CrawlConfig, Value};

// spec: TECH-143
#[tokio::test]
async fn captured_uncrawlable_navigation_is_filterable_and_never_queued() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.store_uncrawlable_links = true;
    let fx =
        os_engine::testing::crawl_fixture_from("tech-m2-links", "http://site.test/", cfg.clone())
            .await;
    let page = os_analyze::views::query_view(
        &fx.file,
        &cfg,
        &os_analyze::views::ViewQuery::new("Links", "Pages With Uncrawlable Internal Outlinks"),
    )
    .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0][0], Value::Text("http://site.test/".into()));
    let hidden:(String,i64)=fx.file.conn().query_row("SELECT u.state,COUNT(l.id) FROM urls u JOIN links l ON l.target_id=u.id WHERE u.address='http://site.test/hidden.html' AND l.crawlable=0 GROUP BY u.id",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_ne!(hidden.0, "done");
    assert_eq!(hidden.1, 2);
    let raw:i64=fx.file.conn().query_row("SELECT COUNT(*) FROM links WHERE target_raw='mailto:hello@site.test' AND target_id IS NULL",[],|r|r.get(0)).unwrap();
    assert_eq!(raw, 1);
}
