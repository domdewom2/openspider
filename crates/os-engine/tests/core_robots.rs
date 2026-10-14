//! robots.txt im Kern: die Zeile der robots.txt zählt nicht (CRAWL-066/CRAWL-130), gesperrte URLs werden nicht
//! abgerufen und tragen Statuscode 0. Die Regelauswertung selbst (CRAWL-098/099) prüft `os-fetch`, die
//! Modi „Ignorieren, aber melden“ und die Listenmodus-Voreinstellung prüft P15b (`decide_*.rs`).

#[path = "core_support.rs"]
mod support;

use os_engine::testing::crawl_fixture;
use os_model::CrawlConfig;
use support::*;

// spec: CRAWL-066
#[tokio::test]
async fn the_robots_txt_row_exists_but_does_not_count_towards_the_limit_or_the_crawled_counter() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_urls = 3;
    cfg.speed.threads = 1;
    let fx = crawl_fixture("crawl-robots", cfg).await;
    let base = fx.site.base_url();

    // genau 3 Abrufe außer der robots.txt, die zusätzlich einmal abgerufen wurde
    assert_eq!(
        requested_paths(&fx, None).len(),
        3,
        "{:?}",
        requested_paths(&fx, None)
    );
    let robots_hits = fx
        .site
        .requests()
        .iter()
        .filter(|r| r.path() == "/robots.txt")
        .count();
    assert_eq!(robots_hits, 1);

    let r = row_of(&fx, &format!("{base}/robots.txt"));
    assert_eq!(r.via, "robots_txt");
    assert_eq!(r.status_code, Some(200));
    assert!(
        !r.in_tables,
        "K-54: die robots.txt interner Hosts erscheint in keinem Tab"
    );
    // die robots.txt steht mit Inhalt in robots_files
    let (status, attempts, has_body): (String, i64, bool) = fx
        .file
        .conn()
        .query_row(
            "SELECT fetch_status, attempts, body IS NOT NULL FROM robots_files WHERE robots_url = ?1",
            [format!("{base}/robots.txt")],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((status.as_str(), attempts, has_body), ("ok", 1, true));
    // gecrawlt = alle Zeilen mit Status außer der robots.txt
    let with_status = scalar(
        &fx,
        "SELECT COUNT(*) FROM urls WHERE status_code IS NOT NULL AND discovered_via != 'robots_txt'",
    );
    assert_eq!(fx.summary.crawled as i64, with_status);
}

// spec: CRAWL-098, CRAWL-066
#[tokio::test]
async fn blocked_urls_get_status_zero_are_never_requested_and_do_not_use_up_the_limit() {
    let fx = crawl_fixture("crawl-robots", one_thread()).await;
    let base = fx.site.base_url();
    // „Disallow: /privat/“ in der Gruppe OpenSpider
    let blocked = row_of(&fx, &format!("{base}/privat/x"));
    assert_eq!(blocked.status_code, Some(0));
    assert_eq!(blocked.status.as_deref(), Some("Blocked by Robots.txt"));
    assert_eq!(blocked.state, "done");
    let (flag, line): (i64, Option<String>) = fx
        .file
        .conn()
        .query_row(
            "SELECT robots_blocked, robots_matched_line FROM urls WHERE url = ?1",
            [format!("{base}/privat/x")],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(flag, 1);
    assert!(line.unwrap_or_default().contains("/privat/"));
    assert!(!requested_paths(&fx, None).contains(&"/privat/x".to_string()));
    // ein erlaubter Pfad derselben Seite wird abgerufen
    assert_eq!(
        row_of(&fx, &format!("{base}/privat/oeffentlich")).status_code,
        Some(200)
    );
    // gesperrte Zeilen zählen als abgeschlossen („gecrawlt“), aber nicht als Abruf
    assert!(
        scalar(
            &fx,
            "SELECT value FROM crawl_counters WHERE key = 'robots_blocked_internal'"
        ) >= 1
    );

    let mut cfg = one_thread();
    cfg.limits.max_urls = 4;
    let fx = crawl_fixture("crawl-robots", cfg).await;
    assert_eq!(
        requested_paths(&fx, None).len(),
        4,
        "gesperrte URLs verbrauchen kein Limit"
    );
    assert!(fx.summary.crawled >= 4);
}

// spec: CRAWL-114, CRAWL-098
#[tokio::test]
async fn the_gate_takes_its_token_from_the_user_agent_preset() {
    // Voreinstellung „Googlebot“ ohne eigenes Token: es gilt die Gruppe „User-agent: Googlebot“
    // (sperrt /nur-google/), nicht die des OpenSpider-Tokens (sperrt /privat/).
    let mut cfg = one_thread();
    cfg.user_agent.preset = "googlebot-smartphone".to_string();
    let fx = crawl_fixture("crawl-robots", cfg).await;
    let base = fx.site.base_url();
    let google_only = row_of(&fx, &format!("{base}/nur-google/a"));
    assert_eq!(google_only.status_code, Some(0));
    assert_eq!(google_only.status.as_deref(), Some("Blocked by Robots.txt"));
    assert_eq!(
        row_of(&fx, &format!("{base}/privat/x")).status_code,
        Some(200)
    );
    assert!(!requested_paths(&fx, None).contains(&"/nur-google/a".to_string()));
}
