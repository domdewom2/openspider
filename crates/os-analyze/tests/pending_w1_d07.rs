//! Full crawl validation coverage requires M2-D10's validation facts (K2-26).
use os_analyze::views::{query_view, state_of, ViewQuery};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, FilterState};

// spec: TECH-008 [M2]
// spec: TECH-086
#[tokio::test]
async fn crawl_transports_validation_coverage_for_outside_hreflang() {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.html_validation = true;
    let fx = crawl_fixture_from(
        "tech-m2-hreflang",
        "http://site.test/outside.html",
        cfg.clone(),
    )
    .await;
    let q = ViewQuery::new("Hreflang", "Outside <head>");
    let on = query_view(&fx.file, &cfg, &q).unwrap();
    assert_eq!(on.rows.len(), 1);
    assert_eq!(
        state_of(&fx.file, &cfg, "Hreflang", "Outside <head>").unwrap(),
        FilterState::Ready
    );
    cfg.advanced.html_validation = false;
    let fx = crawl_fixture_from(
        "tech-m2-hreflang",
        "http://site.test/outside.html",
        cfg.clone(),
    )
    .await;
    assert!(query_view(&fx.file, &cfg, &q).unwrap().rows.is_empty());
    assert!(matches!(
        state_of(&fx.file, &cfg, "Hreflang", "Outside <head>").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    assert_eq!(
        query_view(
            &fx.file,
            &cfg,
            &ViewQuery::new("Hreflang", "Contains Hreflang")
        )
        .unwrap()
        .rows
        .len(),
        1
    );
}
