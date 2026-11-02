//! M2-N28 (Fable-Entscheidungen zu M2-N25):
//! 1. `Total URLs Encountered` nach der OS-Festlegung zu UI-061: Overview, Bericht „Crawl-Übersicht“,
//!    Statusleiste und Crawl-Liste zählen dieselben gefundenen URLs, ohne robots.txt-Zeilen (K-54) und
//!    Sitemap-Dateien (CRAWL-130). `fx-ui-basic`: 27 statt 29 (zwei robots.txt-Zeilen, interner und externer Host).
//! 2. Der Änderungsfeed liest und parst die gespeicherte robots.txt eines Origins nur einmal (UI-145, PERF-004)
//!    und meldet den Crawl-delay-Hinweis je Origin einmal, auch bei gleichzeitigen Aktualisierungen
//!    (Fortsetzen, CRAWL-145).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use common::*;
use os_model::CrawlConfig;
use os_store::{CrawlFile, OpenMode};
use serde_json::json;
use std::time::Duration;

fn summary_value(overview: &serde_json::Value, key: &str) -> u64 {
    overview["summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["key"] == key)
        .unwrap_or_else(|| panic!("{key}: {overview}"))["count"]
        .as_u64()
        .unwrap()
}

// spec: UI-061
// spec: CRAWL-130
// spec: CRAWL-098
#[tokio::test]
async fn overview_report_status_bar_and_crawl_list_agree_on_encountered_urls() {
    let site = os_testsite::TestSite::start_dir(fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    cfg().save_default(&app.config().paths.settings).unwrap();
    let started = call(
        &app,
        "crawl.start",
        serde_json::to_value(request(site.url("/"))).unwrap(),
    )
    .await;
    // Pausieren und Fortsetzen: Die Engine baut ihr Seen-Set dann aus der Crawl-Datei.
    tokio::time::timeout(Duration::from_secs(5), async {
        while site.requests().len() < 6 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    call(&app, "crawl.pause", json!(null)).await;
    call(&app, "crawl.resume", json!(null)).await;
    completed(&app).await;

    let path = app.sessions().current(&window()).unwrap().path;
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let robots: u64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE discovered_via = 'robots_txt' AND in_tables = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let all: u64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    assert_eq!((all, robots), (29, 2), "interner und externer Host");

    // Overview (Seitenleiste, Dashboard)
    let overview = call(&app, "sidebar.overview", json!({})).await;
    assert_eq!(summary_value(&overview, "Total URLs Encountered"), 27);
    assert_eq!(summary_value(&overview, "Total URLs Displayed"), 27);
    assert_eq!(summary_value(&overview, "Total URLs Crawled"), 27);

    // Bericht „Crawl-Übersicht“
    let cfg = file.config().unwrap();
    let report = os_export::reports::overview::build(
        &file,
        &cfg,
        &os_analyze::tabs::configured_for_file(&cfg, &file),
    )
    .unwrap();
    let encountered = report
        .rows
        .iter()
        .find(|r| r.section == "Summary" && r.item == "Total URLs Encountered")
        .unwrap();
    assert_eq!(
        encountered.urls,
        os_export::reports::overview::Cell::Int(27)
    );

    // Statusleiste (Fortschritt der Engine nach dem Fortsetzen) und Abgleich aus der Datei
    let status = app.assemble_tick().unwrap().crawl.unwrap();
    assert_eq!(status.discovered, 27);
    assert_eq!(
        os_store::queries::session_counts(file.conn())
            .unwrap()
            .discovered,
        27
    );
    drop(file);

    // Crawl-Liste
    let listings = call(&app, "crawls.list", json!({"show":"all"})).await;
    let item = listings
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["crawl"]["crawlId"] == started["crawlId"])
        .unwrap();
    assert_eq!(item["crawl"]["urls"], 27, "{item}");
}

fn robots(fx: &fixture::Fx, origin: &str, seconds: u32) {
    fx.write(&format!(
        "INSERT INTO robots_files(origin,robots_url,fetch_status,attempts,body) \
         VALUES ('{origin}','{origin}/robots.txt','ok',1,'User-agent: *\nCrawl-delay: {seconds}\n')"
    ));
}

fn delay_notices(app: &os_app::App) -> Vec<(String, f64)> {
    app.assemble_tick()
        .map(|t| {
            t.notices
                .iter()
                .filter(|n| n.code == "crawl-delay")
                .map(|n| {
                    (
                        n.args["origin"].as_str().unwrap().to_string(),
                        n.args["seconds"].as_f64().unwrap(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

// spec: UI-145
// spec: PERF-004
// spec: CRAWL-145
#[tokio::test]
async fn feed_parses_the_stored_robots_txt_of_an_origin_once() {
    let fx = fixture::Fx::config(CrawlConfig::default());
    robots(&fx, "https://site.test", 1);
    let session = fx.app.sessions().current(&window()).unwrap();
    os_app::session::feed::refresh(&fx.app, &session)
        .await
        .unwrap();
    assert_eq!(
        delay_notices(&fx.app),
        vec![("https://site.test".to_string(), 1.0)]
    );
    // Ein bekannter Origin wird nicht erneut gelesen: Eine (im Crawl nicht vorkommende) Änderung des
    // gespeicherten Texts bleibt ohne Wirkung. Ein neuer Origin wird gelesen.
    fx.write("UPDATE robots_files SET body = 'User-agent: *\nCrawl-delay: 7\n'");
    robots(&fx, "https://other.test", 3);
    os_app::session::feed::invalidate(&fx.app, &session.key);
    os_app::session::feed::refresh(&fx.app, &session)
        .await
        .unwrap();
    assert_eq!(
        delay_notices(&fx.app),
        vec![("https://other.test".to_string(), 3.0)]
    );
}

// spec: CRAWL-145
// spec: UI-013
#[tokio::test]
async fn concurrent_feed_refreshes_report_a_crawl_delay_once() {
    let fx = fixture::Fx::config(CrawlConfig::default());
    robots(&fx, "https://site.test", 2);
    let session = fx.app.sessions().current(&window()).unwrap();
    // Poll und Steuerpfad (z. B. beim Fortsetzen) aktualisieren gleichzeitig.
    let (a, b) = tokio::join!(
        os_app::session::feed::refresh(&fx.app, &session),
        os_app::session::feed::refresh(&fx.app, &session)
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(
        delay_notices(&fx.app),
        vec![("https://site.test".to_string(), 2.0)]
    );
    // Nach einer Unterbrechung des Feeds (Fortsetzen: `invalidate`) kein zweiter Hinweis.
    os_app::session::feed::invalidate(&fx.app, &session.key);
    os_app::session::feed::refresh(&fx.app, &session)
        .await
        .unwrap();
    assert!(delay_notices(&fx.app).is_empty());
}
