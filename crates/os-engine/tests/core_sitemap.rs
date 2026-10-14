//! Sitemap-only-URLs (CRAWL-005): ohne Tiefe, nach allen per Link erreichbaren URLs. Der Abruf läuft über
//! `sitemaps::ingest` (P15b). Die Verdrahtung (Kennungen, `SitemapWrite`, Queue-Einträge) prüft der Unit-Test
//! `worker::tests::sitemap_batch_*`. Dazu K-41d: nur Einträge desselben Ursprungs wie die Sitemap-Datei.

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

// spec: TECH-210
#[tokio::test]
async fn sitemap_entries_of_another_scheme_host_or_port_are_dropped_like_invalid_ones() {
    // K-41d: /ok-a und /ok-b (ausdrücklicher Standardport) gehören zur Sitemap-Datei; andere Port, https und
    // other.test nicht. Verworfene Einträge: keine URL-Zeile, keine Warteschlange, kein Abruf, aber
    // `invalid_entries` und ein Ereignis mit der Anzahl.
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg.sitemaps.urls = vec!["http://site.test/sitemap.xml".to_string()];
    let fx = crawl_fixture_from("crawl-sitemaps-foreign", "http://site.test/", cfg).await;

    for ok in ["/ok-a", "/ok-b"] {
        let r = row_of(&fx, &format!("http://site.test{ok}"));
        assert_eq!(r.via, "sitemap", "{ok}");
        assert_eq!(r.status_code, Some(200), "{ok}");
        assert_eq!(count_requests(&fx, "site.test", ok), 1, "{ok}");
    }
    for foreign in ["/andere-port", "/anderes-schema", "/anderer-host"] {
        assert_eq!(
            scalar(
                &fx,
                &format!("SELECT COUNT(*) FROM urls WHERE url LIKE '%{foreign}'")
            ),
            0,
            "{foreign}: keine URL-Zeile"
        );
        assert!(
            fx.site.requests().iter().all(|r| r.path() != foreign),
            "{foreign}: nicht abgerufen"
        );
    }
    assert_eq!(scalar(&fx, "SELECT COUNT(*) FROM sitemap_entries"), 2);
    assert_eq!(
        scalar(
            &fx,
            "SELECT entries FROM sitemaps WHERE url LIKE '%/sitemap.xml'"
        ),
        5,
        "alle <loc>-Einträge der Datei"
    );
    assert_eq!(
        scalar(
            &fx,
            "SELECT invalid_entries FROM sitemaps WHERE url LIKE '%/sitemap.xml'"
        ),
        3,
        "drei fremde Einträge zählen wie ungültige"
    );
    let events: Vec<(String, String)> = {
        let mut stmt = fx
            .file
            .conn()
            .prepare(
                "SELECT kind, message FROM crawl_events WHERE code = 'sitemap_foreign_entries'",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, "warn");
    assert!(events[0].1.starts_with("3 "), "{}", events[0].1);
}
