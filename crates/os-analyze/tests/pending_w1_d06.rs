//! Vollständige Integrationstests gegen die noch ausstehenden Welle-1-Schritte.
use os_analyze::views::{self, ViewQuery};
use os_engine::testing::crawl_fixture;
use os_model::{CrawlConfig, FilterState};

// spec: TECH-051 [M2]
#[tokio::test]
#[ignore = "wartet auf M2-E01"]
async fn respect_noindex_hides_other_tables_but_keeps_directives() {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_noindex = true;
    let fx = crawl_fixture("tech-m2-directives", cfg.clone()).await;
    let shown: bool = fx
        .file
        .conn()
        .query_row(
            "SELECT in_tables FROM urls WHERE address LIKE '%/body.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!shown);
    let mut q = ViewQuery::new("Directives", "Noindex");
    q.limit = 500;
    let page = views::query_view(&fx.file, &cfg, &q).unwrap();
    assert!(page.rows.iter().any(|r| r
        .iter()
        .any(|v| matches!(v, os_model::Value::Text(s) if s.ends_with("/body.html")))));
}

// spec: TECH-008 [M2]
// spec: TECH-065
#[tokio::test]
#[ignore = "wartet auf M2-D10"]
async fn validation_step_supplies_coverage_marker_for_outside_head() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture("tech-m2-directives", cfg.clone()).await;
    assert_eq!(
        views::state_of(&fx.file, &cfg, "Directives", "Outside <head>").unwrap(),
        FilterState::Ready
    );
    let mut q = ViewQuery::new("Directives", "Outside <head>");
    q.limit = 500;
    let page = views::query_view(&fx.file, &cfg, &q).unwrap();
    assert_eq!(page.rows.len(), 3);
}
