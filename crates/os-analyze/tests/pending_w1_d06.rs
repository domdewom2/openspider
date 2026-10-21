//! Vollständige Integrationstests gegen die noch ausstehenden Welle-1-Schritte.
//! M1-Tests aus tab_directives.rs hierher verlagert (C9/K2-26).
//! M2-N01 entfernt nach E01/D10 die Ignores; siehe docs/plan/M2-reports/d06.md.
use os_analyze::views::{self, ViewQuery};
use os_engine::testing::crawl_fixture;
use os_model::{CrawlConfig, FilterState};

// spec: TECH-051 [M2]
#[tokio::test]
async fn respect_noindex_hides_noindex_urls_except_in_the_directives_filters() {
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
    let mut internal = ViewQuery::new("Internal", "All");
    internal.limit = 500;
    let page = views::query_view(&fx.file, &cfg, &internal).unwrap();
    assert!(!page.rows.iter().any(|r| r
        .iter()
        .any(|v| matches!(v, os_model::Value::Text(s) if s.ends_with("/body.html")))));
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
async fn outside_head_filter_needs_html_validation() {
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
    let mut disabled = cfg.clone();
    disabled.advanced.html_validation = false;
    assert!(matches!(
        views::state_of(&fx.file, &disabled, "Directives", "Outside <head>").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    assert!(views::query_view(&fx.file, &disabled, &q)
        .unwrap()
        .rows
        .is_empty());
}
