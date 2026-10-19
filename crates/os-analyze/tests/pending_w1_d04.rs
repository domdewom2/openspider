//! Vollständige Integrationstests gegen die fremden F05-Erweiterungspunkte (C9/K2-26).
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_model::{CrawlConfig, Value};
fn paths(file: &os_store::CrawlFile, cfg: &CrawlConfig, base: &str, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new("Canonicals", filter);
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    let mut paths: Vec<_> = query_view(file, cfg, &q)
        .unwrap()
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.replace(base, ""),
            _ => panic!("Adresse"),
        })
        .collect();
    paths.sort();
    paths
}
// spec: TECH-011 [M2]
#[tokio::test]
#[ignore = "M2: wartet auf M2-E01"]
async fn respect_canonical_hides_sources_but_preserves_crawling_and_the_canonicalised_filter() {
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_canonical = true;
    let crawl = os_engine::testing::crawl_fixture("tech-m2-canonicals", cfg.clone()).await;
    let all = paths(&crawl.file, &cfg, &crawl.site.base_url(), "All");
    assert!(!all.contains(&"/source".into()));
    assert!(all.contains(&"/child".into()));
    assert!(
        paths(&crawl.file, &cfg, &crawl.site.base_url(), "Canonicalised")
            .contains(&"/source".into())
    );
    assert!(crawl
        .site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/child"));
}
