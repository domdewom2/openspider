//! Crawl-Reihenfolge, Tiefe und Tiefenlimit (CRAWL-005, CRAWL-067) gegen die Fixture `crawl-depth`.
//!
//! Kette `/` → `/1` → … → `/10`, Direktlink `/` → `/7`, `/2` bindet `/style.css` ein, `/3` ein Bild.
//! Der Start erfolgt auf `http://site.test/`, damit die absoluten URLs der Sitemap intern sind.

#[path = "core_support.rs"]
mod support;

use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;
use support::*;

const START: &str = "http://site.test/";

fn depth_of(fx: &os_engine::testing::FixtureCrawl, path: &str) -> Option<i64> {
    row_of(fx, &url("site.test", path)).depth
}

// spec: CRAWL-005
#[tokio::test]
async fn depth_is_the_minimum_number_of_clicks() {
    let fx = crawl_fixture_from("crawl-depth", START, CrawlConfig::default()).await;
    assert_eq!(depth_of(&fx, "/"), Some(0));
    // Kette Start → A → B → C: C hat Tiefe 3
    assert_eq!(depth_of(&fx, "/1"), Some(1));
    assert_eq!(depth_of(&fx, "/2"), Some(2));
    assert_eq!(depth_of(&fx, "/3"), Some(3));
    // /7 ist zusätzlich direkt verlinkt: Tiefe 1 (Minimum), die Seiten dahinter zählen ab dort
    assert_eq!(depth_of(&fx, "/7"), Some(1));
    assert_eq!(depth_of(&fx, "/8"), Some(2));
    assert_eq!(depth_of(&fx, "/9"), Some(3));
    assert_eq!(
        depth_of(&fx, "/10"),
        Some(4),
        "über /7 statt über die Kette von /1"
    );
}

// spec: CRAWL-005
#[tokio::test]
async fn breadth_first_with_one_thread_never_requests_a_deeper_url_first() {
    let fx = crawl_fixture_from("crawl-depth", START, one_thread()).await;
    // Tiefe jeder abgerufenen Seite in Abrufreihenfolge
    let depths: Vec<i64> = requested_paths(&fx, Some("site.test"))
        .iter()
        .map(|p| depth_of(&fx, p).unwrap_or_else(|| panic!("{p} ohne Tiefe")))
        .collect();
    assert!(depths.len() >= 11, "{depths:?}");
    assert!(
        depths.windows(2).all(|w| w[0] <= w[1]),
        "Tiefen im Request-Log nicht aufsteigend: {depths:?}"
    );
}

// spec: CRAWL-005
#[tokio::test]
async fn resources_get_the_depth_of_the_including_page_plus_one() {
    let fx = crawl_fixture_from("crawl-depth", START, CrawlConfig::default()).await;
    // /2 hat Tiefe 2, /3 hat Tiefe 3
    assert_eq!(depth_of(&fx, "/style.css"), Some(3));
    assert_eq!(depth_of(&fx, "/img/b3.png"), Some(4));
}

// spec: CRAWL-067
#[tokio::test]
async fn depth_limit_two_stops_the_chain_after_the_third_level() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_depth = Some(2);
    let fx = crawl_fixture_from("crawl-depth", START, cfg).await;
    // /, /1, /2 werden angefragt, /3 nicht
    let paths = requested_paths(&fx, Some("site.test"));
    for p in ["/", "/1", "/2"] {
        assert!(paths.contains(&p.to_string()), "{p} fehlt in {paths:?}");
    }
    assert!(!paths.contains(&"/3".to_string()), "{paths:?}");
    // /3 steht als Outlink von /2 in der Datei, ohne Statuscode und ohne Eintrag in den Tabellen
    let r3 = row_of(&fx, &url("site.test", "/3"));
    assert_eq!(
        (r3.state.as_str(), r3.reason.as_deref()),
        ("discovered", Some("depth_limit"))
    );
    assert_eq!(r3.status_code, None);
    assert!(!r3.in_tables);
    let r2 = row_of(&fx, &url("site.test", "/2"));
    let outlinks_to_3 = scalar(
        &fx,
        &format!(
            "SELECT COUNT(*) FROM links WHERE source_id = {} AND target_id = {} AND link_type = 'Hyperlink'",
            r2.id, r3.id
        ),
    );
    assert_eq!(outlinks_to_3, 1, "/3 bleibt Outlink von /2");
    // alle abgerufenen Seiten liegen höchstens auf Tiefe 2 (Ressourcen dürfen eine Ebene tiefer liegen)
    assert_eq!(
        scalar(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE content_class = 'HTML' AND status_code IS NOT NULL AND crawl_depth > 2"
        ),
        0
    );
    assert!(
        scalar(
            &fx,
            "SELECT value FROM crawl_counters WHERE key = 'skipped_depth_limit'"
        ) > 0
    );
}

// spec: CRAWL-067
#[tokio::test]
async fn depth_zero_crawls_only_the_start_url() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_depth = Some(0);
    let fx = crawl_fixture_from("crawl-depth", START, cfg).await;
    let html_done = scalar(
        &fx,
        "SELECT COUNT(*) FROM urls WHERE content_class = 'HTML' AND status_code IS NOT NULL",
    );
    assert_eq!(html_done, 1, "nur die Start-URL");
    assert_eq!(row_of(&fx, START).status_code, Some(200));
    // die Links der Startseite sind gespeichert
    assert_eq!(
        row_of(&fx, &url("site.test", "/1")).reason.as_deref(),
        Some("depth_limit")
    );
    assert_eq!(
        row_of(&fx, &url("site.test", "/7")).reason.as_deref(),
        Some("depth_limit")
    );
}
