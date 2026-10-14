//! Weiterleitungen: HTTP (CRAWL-014), Meta-/HTTP-Refresh (CRAWL-015), Ketten und Schleifen (CRAWL-017) und das
//! Limit je Kette (CRAWL-072). Fixtures: `crawl-redirects`, `crawl-refresh`, `crawl-redirect-chains`.

#[path = "core_support.rs"]
mod support;

use os_engine::testing::crawl_fixture_from;
use os_model::CrawlConfig;
use support::*;

const HOST: &str = "site.test";

fn u(path: &str) -> String {
    url(HOST, path)
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-014

// spec: CRAWL-014
#[tokio::test]
async fn all_five_http_redirect_statuses_are_stored_with_their_target_and_the_target_is_requested_once(
) {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    let ziel = row_of(&fx, &u("/ziel"));
    for (path, status) in [
        ("/r301", 301),
        ("/r302", 302),
        ("/r303", 303),
        ("/r307", 307),
        ("/r308", 308),
    ] {
        let r = row_of(&fx, &u(path));
        assert_eq!(r.status_code, Some(status), "{path}");
        assert_eq!(
            r.redirect_url.as_deref(),
            Some("http://site.test/ziel"),
            "{path}"
        );
        assert_eq!(r.redirect_type.as_deref(), Some("http"), "{path}");
        assert_eq!(r.redirect_target, Some(ziel.id), "{path}");
        assert_eq!(r.state, "done");
    }
    // das Ziel kommt über die normale Deduplizierung genau einmal an den Server
    assert_eq!(count_requests(&fx, HOST, "/ziel"), 1);
    assert_eq!(ziel.status_code, Some(200));
    // Ziel = Quelle + 1
    assert_eq!(row_of(&fx, &u("/r301")).depth, Some(1));
    assert_eq!(ziel.depth, Some(2));
}

// spec: CRAWL-014
#[tokio::test]
async fn relative_location_is_resolved_against_the_requested_url() {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    let r = row_of(&fx, &u("/dir/r-relativ"));
    assert_eq!(r.status_code, Some(301));
    assert_eq!(
        r.redirect_url.as_deref(),
        Some("http://site.test/ziel"),
        "../ziel von /dir/…"
    );
}

// spec: CRAWL-014
#[tokio::test]
async fn a_3xx_without_location_is_stored_without_target_and_without_a_follow_up_request() {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    let r = row_of(&fx, &u("/r-leer"));
    assert_eq!(r.status_code, Some(301));
    assert_eq!(r.redirect_url, None);
    assert_eq!(r.redirect_type, None);
    assert_eq!(r.redirect_target, None);
    // der Body (mit Link) einer 3xx-Antwort wird nie ausgewertet
    assert!(row(&fx, &u("/body-link")).is_none());
    assert_eq!(count_requests(&fx, HOST, "/body-link"), 0);
    // auch /r301 hat einen Body mit diesem Link
    assert_eq!(
        scalar(
            &fx,
            "SELECT COUNT(*) FROM links WHERE target_raw LIKE '%body-link%'"
        ),
        0
    );
}

// spec: CRAWL-014
#[tokio::test]
async fn external_redirect_target_is_stored_as_external_and_checked_once() {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    let r = row_of(&fx, &u("/r-extern"));
    assert_eq!(r.redirect_url.as_deref(), Some("https://other.test/"));
    let target = row_of(&fx, "https://other.test/");
    assert!(!target.is_internal);
    assert_eq!(target.status_code, Some(200), "einmal geprüft");
    assert_eq!(r.redirect_target, Some(target.id));
    let hits = fx
        .site
        .requests()
        .iter()
        .filter(|q| q.host == "other.test" && q.path_and_query == "/")
        .count();
    assert_eq!(hits, 1);
    // externe Seiten werden nicht geparst: der Link /other-secret wird nie entdeckt
    assert!(row(&fx, "https://other.test/other-secret").is_none());
    assert_eq!(
        scalar(
            &fx,
            &format!("SELECT COUNT(*) FROM links WHERE source_id = {}", target.id)
        ),
        0
    );
}

// spec: CRAWL-014
#[tokio::test]
async fn an_excluded_redirect_target_is_not_requested_but_the_redirect_url_is_filled() {
    let mut cfg = CrawlConfig::default();
    cfg.scope.exclude = vec![".*/privat/.*".to_string()];
    let fx = crawl_fixture_from("crawl-redirects", "http://site.test/", cfg).await;
    let r = row_of(&fx, &u("/r-excluded"));
    assert_eq!(r.status_code, Some(301));
    assert_eq!(r.redirect_url.as_deref(), Some("http://site.test/privat/"));
    assert_eq!(count_requests(&fx, HOST, "/privat/"), 0, "nicht angefragt");
    let target = row_of(&fx, &u("/privat/"));
    assert_eq!(target.reason.as_deref(), Some("excluded"));
    assert_eq!(target.status_code, None);
    assert_eq!(
        scalar(
            &fx,
            "SELECT value FROM crawl_counters WHERE key = 'excluded_skipped'"
        ),
        1
    );
}

// spec: CRAWL-014
#[tokio::test]
async fn location_with_non_ascii_characters_is_percent_encoded() {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    let r = row_of(&fx, &u("/r-umlaut"));
    assert_eq!(
        r.redirect_url.as_deref(),
        Some("http://site.test/z%C3%AFel")
    );
    let target = row_of(&fx, &u("/z%C3%AFel"));
    assert_eq!(target.status_code, Some(200));
}

// spec: CRAWL-014
#[tokio::test]
async fn redirects_are_linked_as_http_redirect_inlinks_and_the_start_url_may_redirect() {
    let fx = crawl_fixture_from(
        "crawl-redirects",
        "http://site.test/r301",
        CrawlConfig::default(),
    )
    .await;
    let start = row_of(&fx, &u("/r301"));
    assert_eq!(start.depth, Some(0));
    assert_eq!(start.redirect_type.as_deref(), Some("http"));
    let target = row_of(&fx, &u("/ziel"));
    assert_eq!(target.depth, Some(1));
    let n = scalar(
        &fx,
        &format!(
            "SELECT COUNT(*) FROM links WHERE source_id = {} AND target_id = {} AND link_type = 'HTTP Redirect'",
            start.id, target.id
        ),
    );
    assert_eq!(n, 1);
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-015

// spec: CRAWL-015
#[tokio::test]
async fn meta_refresh_variants_are_recognised_and_keep_the_page_status() {
    let fx = crawl_fixture_from("crawl-refresh", "http://site.test/", CrawlConfig::default()).await;
    let cases = [
        ("/meta0", "/ziel1", 0),
        ("/meta5", "/ziel2", 5),
        ("/meta-quotes", "/ziel3", 0), // http-equiv in Großbuchstaben, URL='…'
        ("/meta-quotes2", "/ziel4", 5), // url="…"
        ("/meta-nospace", "/ziel8", 0),
    ];
    for (path, target, delay) in cases {
        let r = row_of(&fx, &u(path));
        assert_eq!(
            r.status_code,
            Some(200),
            "{path}: Statuscode der Seite bleibt"
        );
        assert_eq!(r.redirect_type.as_deref(), Some("meta_refresh"), "{path}");
        assert_eq!(
            r.redirect_url,
            Some(format!("http://site.test{target}")),
            "{path}"
        );
        assert_eq!(r.refresh_delay, Some(delay), "{path}: Verzögerung");
        let t = row_of(&fx, &u(target));
        assert_eq!(t.status_code, Some(200), "{target} wurde abgerufen");
        assert_eq!(t.depth, Some(2), "Refresh-Ziel = Quelle + 1");
        assert_eq!(r.redirect_target, Some(t.id));
    }
}

// spec: CRAWL-015
#[tokio::test]
async fn refresh_without_url_is_a_self_refresh_and_only_the_first_meta_tag_counts() {
    let fx = crawl_fixture_from("crawl-refresh", "http://site.test/", CrawlConfig::default()).await;
    // K-57: der Selbst-Refresh ist eine Weiterleitung auf sich selbst, ohne neuen Abruf
    let selfr = row_of(&fx, &u("/meta-self"));
    assert_eq!(
        selfr.redirect_url.as_deref(),
        Some(u("/meta-self").as_str())
    );
    assert_eq!(selfr.redirect_type.as_deref(), Some("meta_refresh"));
    assert_eq!(selfr.redirect_target, Some(selfr.id), "Ziel = eigene Zeile");
    assert_eq!(selfr.status_code, Some(200));
    assert_eq!(
        selfr.refresh_delay,
        Some(30),
        "Verzögerung wird trotzdem gespeichert"
    );
    assert_eq!(
        count_requests(&fx, HOST, "/meta-self"),
        1,
        "kein neuer Abruf"
    );
    // Schleife (Redirect Loop) und eine Linkzeile Meta Refresh mit Quelle = Ziel
    let loops: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags WHERE url_id = ?1 AND flag = 'redirect_loop'",
            [selfr.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(loops, 1);
    let link_rows: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM links WHERE source_id = ?1 AND target_id = ?1 AND link_type = 'Meta Refresh'",
            [selfr.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(link_rows, 1);
    // Standard (K-57): die Indexierbarkeit ändert sich nicht; mit der Option „Self Referencing Meta Refresh“
    let indexability = |fx: &os_engine::testing::FixtureCrawl| -> (String, Option<String>) {
        fx.file
            .conn()
            .query_row(
                "SELECT indexability, indexability_status FROM urls WHERE url = ?1",
                [u("/meta-self")],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    assert_eq!(indexability(&fx), ("Indexable".to_string(), None));
    let mut respect = CrawlConfig::default();
    respect.advanced.respect_self_ref_meta_refresh = true;
    let fx_on = crawl_fixture_from("crawl-refresh", "http://site.test/", respect).await;
    assert_eq!(
        indexability(&fx_on),
        (
            "Non-Indexable".to_string(),
            Some("Self Referencing Meta Refresh".to_string())
        )
    );
    let on = row_of(&fx_on, &u("/meta-self"));
    assert_eq!(on.redirect_type.as_deref(), Some("meta_refresh"));

    let two = row_of(&fx, &u("/meta-two"));
    assert_eq!(two.redirect_url.as_deref(), Some("http://site.test/ziel-a"));
    assert!(row(&fx, &u("/ziel-b")).is_none(), "zweites Tag zählt nicht");
    assert_eq!(count_requests(&fx, HOST, "/ziel-b"), 0);
}

// spec: CRAWL-015
#[tokio::test]
async fn a_refresh_header_to_itself_is_an_http_refresh_to_itself_but_one_without_url_is_no_redirect(
) {
    // K-57: mit eigener URL wie der Meta-Refresh: Redirect URL = eigene Adresse, Typ HTTP Refresh, Schleife, kein
    // neuer Abruf. Ohne URL (`Refresh: 60`) führt die Referenz keine Weiterleitung (Paritätslauf M1).
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("scenario.toml"),
        r#"
[[route]]
path = "/"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>start</title></head><body><a href=\"/h-bare\">a</a> <a href=\"/h-own\">b</a></body></html>"

[[route]]
path = "/h-bare"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>h-bare</title></head><body>x</body></html>"
[route.headers]
Refresh = "60"

[[route]]
path = "/h-own"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>h-own</title></head><body>x</body></html>"
[route.headers]
Refresh = "5; url=/h-own"
"#,
    )
    .unwrap();
    let fx = os_engine::testing::crawl_dir_with(
        dir.path(),
        "/",
        CrawlConfig::default(),
        os_engine::testing::test_deps(),
    )
    .await;
    let bare = all_rows(&fx)
        .into_iter()
        .find(|r| r.url.ends_with("/h-bare"))
        .expect("/h-bare");
    assert_eq!((bare.redirect_type, bare.redirect_url), (None, None));
    assert_eq!(bare.refresh_delay, Some(60));
    {
        let path = "/h-own";
        let r = all_rows(&fx)
            .into_iter()
            .find(|r| r.url.ends_with(path))
            .unwrap_or_else(|| panic!("{path}"));
        assert_eq!(r.redirect_type.as_deref(), Some("http_refresh"), "{path}");
        assert_eq!(r.redirect_url.as_deref(), Some(r.url.as_str()), "{path}");
        assert_eq!(r.redirect_target, Some(r.id), "{path}");
        let hits = fx
            .site
            .requests()
            .iter()
            .filter(|q| q.path() == path)
            .count();
        assert_eq!(hits, 1, "{path}: kein neuer Abruf");
    }
}

// spec: CRAWL-015
#[tokio::test]
async fn http_refresh_header_is_parsed_like_a_meta_refresh() {
    let fx = crawl_fixture_from("crawl-refresh", "http://site.test/", CrawlConfig::default()).await;
    let r = row_of(&fx, &u("/http-refresh"));
    assert_eq!(r.status_code, Some(200));
    assert_eq!(r.redirect_type.as_deref(), Some("http_refresh"));
    assert_eq!(r.redirect_url.as_deref(), Some("http://site.test/ziel5"));
    assert_eq!(row_of(&fx, &u("/ziel5")).status_code, Some(200));
}

// spec: CRAWL-015
#[tokio::test]
async fn an_http_3xx_wins_over_a_meta_refresh_in_its_body() {
    let fx = crawl_fixture_from("crawl-refresh", "http://site.test/", CrawlConfig::default()).await;
    let r = row_of(&fx, &u("/meta-and-301"));
    assert_eq!(r.status_code, Some(301));
    assert_eq!(r.redirect_type.as_deref(), Some("http"));
    assert_eq!(r.redirect_url.as_deref(), Some("http://site.test/ziel6"));
    assert!(
        row(&fx, &u("/ziel7")).is_none(),
        "Meta-Refresh im 3xx-Body wird ignoriert"
    );
}

// spec: CRAWL-015
#[tokio::test]
async fn disabling_meta_refresh_crawling_stores_the_redirect_but_does_not_request_the_target() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.meta_refresh.crawl = false;
    let fx = crawl_fixture_from("crawl-refresh", "http://site.test/", cfg).await;
    let r = row_of(&fx, &u("/meta0"));
    assert_eq!(r.redirect_url.as_deref(), Some("http://site.test/ziel1"));
    assert_eq!(count_requests(&fx, HOST, "/ziel1"), 0);
    assert_eq!(
        row_of(&fx, &u("/ziel1")).reason.as_deref(),
        Some("store_only"),
        "Grund wie in der Speichern/Crawlen-Matrix (decide, CRAWL-038)"
    );
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-017

fn chain_cfg() -> CrawlConfig {
    CrawlConfig::default()
}

// spec: CRAWL-017
#[tokio::test]
async fn a_chain_is_followed_to_its_end_and_each_hop_is_requested_once() {
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", chain_cfg()).await;
    for (p, to) in [("/a", "/b"), ("/b", "/c"), ("/c", "/d")] {
        let r = row_of(&fx, &u(p));
        assert_eq!(r.status_code, Some(301));
        assert_eq!(r.redirect_url, Some(format!("http://site.test{to}")));
        assert_eq!(count_requests(&fx, HOST, p), 1, "{p}");
    }
    assert_eq!(row_of(&fx, &u("/d")).status_code, Some(200));
    // Hop-Tiefen: Quelle + 1 je Schritt
    assert_eq!(row_of(&fx, &u("/a")).depth, Some(1));
    assert_eq!(row_of(&fx, &u("/d")).depth, Some(4));
}

// spec: CRAWL-017
#[tokio::test]
async fn loops_and_self_redirects_are_flagged_and_never_requested_twice() {
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", chain_cfg()).await;
    for p in ["/x", "/y", "/s"] {
        assert_eq!(count_requests(&fx, HOST, p), 1, "{p}: genau ein Request");
        assert_eq!(row_of(&fx, &u(p)).status_code, Some(301));
    }
    let f = flags(&fx);
    let looped: Vec<_> = f
        .iter()
        .filter(|(_, flag)| flag == "redirect_loop")
        .map(|(url, _)| url.as_str())
        .collect();
    for p in ["/x", "/y", "/s"] {
        assert!(looped.contains(&u(p).as_str()), "{p} fehlt in {looped:?}");
    }
    // Ketten ohne Schleife tragen das Flag nicht
    for p in ["/a", "/b", "/c", "/d", "/mix1", "/ok"] {
        assert!(!looped.contains(&u(p).as_str()), "{p} ist keine Schleife");
    }
}

// spec: CRAWL-017
#[tokio::test]
async fn chains_over_different_redirect_types_are_followed_as_one() {
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", chain_cfg()).await;
    // 301 → Meta Refresh → 302 → 200
    let m1 = row_of(&fx, &u("/mix1"));
    let m2 = row_of(&fx, &u("/mix2"));
    let m3 = row_of(&fx, &u("/mix3"));
    let ok = row_of(&fx, &u("/ok"));
    assert_eq!(
        (m1.status_code, m1.redirect_type.as_deref()),
        (Some(301), Some("http"))
    );
    assert_eq!(
        (m2.status_code, m2.redirect_type.as_deref()),
        (Some(200), Some("meta_refresh"))
    );
    assert_eq!(
        (m3.status_code, m3.redirect_type.as_deref()),
        (Some(302), Some("http"))
    );
    assert_eq!(ok.status_code, Some(200));
    assert_eq!(m1.redirect_target, Some(m2.id));
    assert_eq!(m2.redirect_target, Some(m3.id));
    assert_eq!(m3.redirect_target, Some(ok.id));
}

// spec: CRAWL-017
#[tokio::test]
async fn chains_across_hosts_are_followed_and_a_target_back_in_scope_is_crawled_normally() {
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", chain_cfg()).await;
    let hop = row_of(&fx, &u("/ext-hop"));
    assert_eq!(hop.redirect_url.as_deref(), Some("https://other.test/back"));
    let back = row_of(&fx, "https://other.test/back");
    assert!(!back.is_internal);
    assert_eq!(
        back.status_code,
        Some(301),
        "externer Zwischenschritt wird abgerufen"
    );
    assert_eq!(back.redirect_url.as_deref(), Some("http://site.test/ok"));
    // /ok ist intern und wurde nur einmal abgerufen, obwohl es von zwei Ketten erreicht wird
    assert_eq!(count_requests(&fx, HOST, "/ok"), 1);
    assert!(row_of(&fx, &u("/ok")).is_internal);
}

// ---------------------------------------------------------------------------------------------------
// CRAWL-072

// spec: CRAWL-072
#[tokio::test]
async fn the_redirect_limit_stops_a_chain_after_the_configured_number_of_hops() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_redirects = 3;
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", cfg).await;
    // Start-URL und 3 Ziele: hop1 … hop4; hop5 nicht
    for p in ["/hop1", "/hop2", "/hop3", "/hop4"] {
        assert_eq!(count_requests(&fx, HOST, p), 1, "{p}");
    }
    assert_eq!(count_requests(&fx, HOST, "/hop5"), 0);
    assert_eq!(count_requests(&fx, HOST, "/hop6"), 0);
    // die letzte Hop-Zeile nennt das nicht mehr verfolgte Ziel und trägt das Flag
    let last = row_of(&fx, &u("/hop4"));
    assert_eq!(last.status_code, Some(301));
    assert_eq!(last.redirect_url.as_deref(), Some("http://site.test/hop5"));
    let limited: Vec<_> = flags(&fx)
        .into_iter()
        .filter(|(_, f)| f == "redirect_limit_reached")
        .map(|(u, _)| u)
        .collect();
    assert!(limited.contains(&u("/hop4")), "{limited:?}");
    assert!(!limited.contains(&u("/hop3")));
    assert_eq!(
        row_of(&fx, &u("/hop5")).reason.as_deref(),
        Some("redirect_limit")
    );
}

// spec: CRAWL-072
#[tokio::test]
async fn the_count_starts_anew_for_every_chain() {
    let mut cfg = CrawlConfig::default();
    cfg.limits.max_redirects = 3;
    let fx = crawl_fixture_from("crawl-redirect-chains", "http://site.test/", cfg).await;
    // zweite Kette im selben Crawl: ebenfalls Start + 3 Ziele
    for p in ["/c2-1", "/c2-2", "/c2-3", "/c2-4"] {
        assert_eq!(count_requests(&fx, HOST, p), 1, "{p}");
    }
    assert_eq!(count_requests(&fx, HOST, "/c2-5"), 0);
    // der Standard (10) verfolgt beide Ketten bis zum Ende
    let fx = crawl_fixture_from(
        "crawl-redirect-chains",
        "http://site.test/",
        CrawlConfig::default(),
    )
    .await;
    assert_eq!(row_of(&fx, &u("/hop6")).status_code, Some(200));
    assert_eq!(row_of(&fx, &u("/c2-6")).status_code, Some(200));
    assert!(flags(&fx)
        .iter()
        .all(|(_, f)| f != "redirect_limit_reached"));
}

// spec: OUT-031, TECH-131, CRAWL-015
#[tokio::test]
async fn synthetic_link_rows_of_head_elements_carry_position_head_and_the_head_path() {
    // K-63 (Paritätsbefund N-09): Canonical, hreflang, Rel Next, Meta Refresh und CSS im Kopf: Position Head und
    // Pfad `//head/<element>[n]`; Header-Quellen haben beides leer.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("scenario.toml"),
        r#"
[[route]]
path = "/"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><meta charset=\"utf-8\"><link rel=\"canonical\" href=\"/\"><link rel=\"stylesheet\" href=\"/s.css\"><link rel=\"alternate\" hreflang=\"en\" href=\"/\"><link rel=\"next\" href=\"/n\"><meta http-equiv=\"refresh\" content=\"7; url=/n\"><title>t</title></head><body><a href=\"/n\">n</a></body></html>"

[[route]]
path = "/n"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>n</title></head><body>n</body></html>"
[route.headers]
Link = "</>; rel=\"canonical\""

[[route]]
path = "/s.css"
content_type = "text/css"
body = "a{}"
"#,
    )
    .unwrap();
    let fx = os_engine::testing::crawl_dir_with(
        dir.path(),
        "/",
        CrawlConfig::default(),
        os_engine::testing::test_deps(),
    )
    .await;
    let q = |sql: &str| -> Vec<(String, Option<String>, Option<String>)> {
        let mut stmt = fx.file.conn().prepare(sql).unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    let rows = q(
        "SELECT l.link_type, l.link_path, l.link_position FROM links l \
         JOIN urls s ON s.id = l.source_id WHERE s.url LIKE '%/' AND s.discovered_via = 'start' ORDER BY l.position",
    );
    let row = |t: &str| {
        rows.iter()
            .find(|r| r.0 == t)
            .unwrap_or_else(|| panic!("{t}: {rows:?}"))
    };
    for (ty, path) in [
        ("Canonical", "//head/link[1]"),
        ("CSS", "//head/link[2]"),
        ("Hreflang", "//head/link[3]"),
        ("Rel Next", "//head/link[4]"),
        ("Meta Refresh", "//head/meta[2]"),
    ] {
        assert_eq!(
            (row(ty).1.as_deref(), row(ty).2.as_deref()),
            (Some(path), Some("Head")),
            "{ty}"
        );
    }
    // Body-Link unverändert, Header-Quelle ohne Pfad und Position
    assert_eq!(row("Hyperlink").1.as_deref(), Some("/html[1]/body[1]/a[1]"));
    assert_eq!(row("Hyperlink").2.as_deref(), Some("Content"));
    let hdr = q(
        "SELECT l.link_type, l.link_path, l.link_position FROM links l JOIN urls s ON s.id = l.source_id \
         WHERE s.url LIKE '%/n' AND l.link_type = 'HTTP Canonical'",
    );
    assert_eq!(hdr, [("HTTP Canonical".to_string(), None, None)]);
}
