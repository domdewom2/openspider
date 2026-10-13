//! Sitemap-only-URLs (CRAWL-005): ohne Tiefe, nach allen per Link erreichbaren URLs. Der Abruf läuft über
//! `sitemaps::ingest` (P15b); bis zu dessen Merge liefert der P00b-Stub eine leere Liste. Die Verdrahtung
//! (Kennungen, `SitemapWrite`, Queue-Einträge) prüft der Unit-Test `worker::tests::sitemap_batch_*`.

#[path = "core_support.rs"]
mod support;

use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;
use support::*;

// spec: CRAWL-005
#[tokio::test]
async fn urls_found_only_in_a_sitemap_have_no_depth_and_are_crawled_after_all_linked_urls() {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.sitemaps.urls = vec!["http://site.test/sitemap.xml".to_string()];
    let fx = crawl_fixture_from("crawl-depth", "http://site.test/", cfg).await;
    let only = row_of(&fx, "http://site.test/nur-sitemap");
    assert_eq!(only.depth, None, "nur über die Sitemap: Tiefe leer");
    assert_eq!(only.via, "sitemap");
    assert_eq!(only.status_code, Some(200));
    // ihre Kinder haben ebenfalls keine Tiefe
    assert_eq!(row_of(&fx, "http://site.test/nur-sitemap-kind").depth, None);
    // /1 steht in der Sitemap und ist verlinkt: Tiefe 1 bleibt
    assert_eq!(row_of(&fx, "http://site.test/1").depth, Some(1));
    // Reihenfolge: nach allen Link-URLs
    let paths = requested_paths(&fx, Some("site.test"));
    let pos = |p: &str| paths.iter().position(|x| x == p).unwrap();
    assert!(pos("/nur-sitemap") > pos("/10"), "{paths:?}");
    // die Sitemap selbst ist erfasst
    let n = scalar(
        &fx,
        "SELECT COUNT(*) FROM sitemaps WHERE url LIKE '%/sitemap.xml'",
    );
    assert_eq!(n, 1);
    assert_eq!(
        scalar(&fx, "SELECT COUNT(*) FROM sitemap_entries"),
        2,
        "zwei <loc>-Einträge"
    );
}
