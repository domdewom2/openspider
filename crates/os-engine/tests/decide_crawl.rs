//! Crawl-Integrationstests gegen die Fixtures von M1-P15b (CRAWL-038/039/044/045/046/056, TECH-210, dazu die
//! Engine-Anteile von CRAWL-098/099 und CRAWL-109/110).
//!
//! Alle Tests laufen über `os_engine::testing::crawl_fixture` und brauchen den echten Crawl-Kern aus P15. Bis dahin
//! sind sie `#[ignore = "wartet auf M1-P15"]`; der Orchestrator schaltet sie in Welle 2.5 (M1-W25) scharf. Die
//! gleichen Entscheidungen prüft `decide_fixtures.rs` ohne Engine (grün schon in Welle 2).
//!
//! Geprüft wird über das Request-Log der Testsite (wurde ein Ziel angefragt?) und die Crawl-Datei (`urls`,
//! `links`, `canonicals`, `sitemaps`, `robots_files`).

use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, RobotsMode, SpiderConfig, StoreCrawl};

// ---------------------------------------------------------------------------------------------------------------
// Hilfen

/// Zeile der Tabelle `urls`.
#[derive(Debug)]
struct UrlRow {
    in_tables: bool,
    state: String,
    status: Option<i64>,
    reason: Option<String>,
}

fn sql_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn count(fx: &FixtureCrawl, sql: &str) -> i64 {
    fx.file
        .conn()
        .query_row(sql, [], |r| r.get(0))
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Zeile zur URL (`None`, wenn die URL gar nicht in der Crawl-Datei steht).
fn url_row(fx: &FixtureCrawl, url: &str) -> Option<UrlRow> {
    let q = sql_quote(url);
    if count(fx, &format!("SELECT COUNT(*) FROM urls WHERE url = '{q}'")) == 0 {
        return None;
    }
    let sql = format!(
        "SELECT in_tables, state, status_code, not_crawled_reason FROM urls WHERE url = '{q}'"
    );
    Some(
        fx.file
            .conn()
            .query_row(&sql, [], |r| {
                Ok(UrlRow {
                    in_tables: r.get::<_, i64>(0)? != 0,
                    state: r.get(1)?,
                    status: r.get(2)?,
                    reason: r.get(3)?,
                })
            })
            .unwrap_or_else(|e| panic!("{sql}: {e}")),
    )
}

/// Ziel steht in den Tabellen.
fn in_tables(fx: &FixtureCrawl, url: &str) -> bool {
    url_row(fx, url).is_some_and(|r| r.in_tables)
}

/// Anzahl Anfragen an `127.0.0.1` für den Pfad.
fn hits(fx: &FixtureCrawl, path: &str) -> usize {
    fx.site.count("127.0.0.1", path)
}

/// Anzahl Anfragen an den externen Testhost.
fn ext_hits(fx: &FixtureCrawl, path: &str) -> usize {
    fx.site.count("external.test", path)
}

fn link_rows(fx: &FixtureCrawl, target_raw: &str) -> i64 {
    count(
        fx,
        &format!(
            "SELECT COUNT(*) FROM links WHERE target_raw = '{}'",
            sql_quote(target_raw)
        ),
    )
}

fn with(f: impl FnOnce(&mut CrawlConfig)) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    f(&mut cfg);
    cfg
}

type Slot = fn(&mut SpiderConfig) -> &mut StoreCrawl;

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-038: Speichern und Crawlen, alle vier Kombinationen je Linktyp

struct Case {
    name: &'static str,
    slot: Slot,
    external: bool,
    /// Pfad des Ziels (extern: Pfad auf `external.test`).
    path: &'static str,
    /// Rohwert des Attributs im HTML, falls der Link in der Tabelle `links` steht.
    link_raw: Option<&'static str>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "interner Hyperlink",
            slot: |s| &mut s.internal,
            external: false,
            path: "/m/intern.html",
            link_raw: Some("/m/intern.html"),
        },
        Case {
            name: "externer Hyperlink",
            slot: |s| &mut s.external,
            external: true,
            path: "/extern.html",
            link_raw: Some("http://external.test/extern.html"),
        },
        Case {
            name: "Bild",
            slot: |s| &mut s.images,
            external: false,
            path: "/m/bild.png",
            link_raw: Some("/m/bild.png"),
        },
        Case {
            name: "CSS",
            slot: |s| &mut s.css,
            external: false,
            path: "/m/style.css",
            link_raw: Some("/m/style.css"),
        },
        Case {
            name: "JavaScript",
            slot: |s| &mut s.javascript,
            external: false,
            path: "/m/app.js",
            link_raw: Some("/m/app.js"),
        },
        Case {
            name: "Medien",
            slot: |s| &mut s.media,
            external: false,
            path: "/m/film.mp4",
            link_raw: Some("/m/film.mp4"),
        },
        Case {
            name: "iframe",
            slot: |s| &mut s.iframes,
            external: false,
            path: "/m/frame.html",
            link_raw: Some("/m/frame.html"),
        },
        Case {
            name: "Canonical",
            slot: |s| &mut s.canonicals,
            external: false,
            path: "/m/canonical.html",
            link_raw: None,
        },
        Case {
            name: "rel=next",
            slot: |s| &mut s.pagination,
            external: false,
            path: "/m/next.html",
            link_raw: None,
        },
        Case {
            name: "hreflang",
            slot: |s| &mut s.hreflang,
            external: false,
            path: "/m/hreflang-fr.html",
            link_raw: None,
        },
        Case {
            name: "Meta Refresh",
            slot: |s| &mut s.meta_refresh,
            external: false,
            path: "/m/refresh.html",
            link_raw: None,
        },
    ]
}

// spec: CRAWL-038
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn store_crawl_matrix_all_four_combinations_per_link_type() {
    for case in cases() {
        for (store, crawl) in [(true, true), (true, false), (false, true), (false, false)] {
            let cfg = with(|c| *(case.slot)(&mut c.spider) = StoreCrawl { store, crawl });
            let fx = crawl_fixture("crawl-store-crawl-matrix", cfg).await;
            let url = if case.external {
                format!("http://external.test{}", case.path)
            } else {
                fx.site.url(case.path)
            };
            let requested = if case.external {
                ext_hits(&fx, case.path)
            } else {
                hits(&fx, case.path)
            };
            let what = format!("{} store={store} crawl={crawl}", case.name);

            // Crawlen: angefragt – unabhängig vom Speichern.
            assert_eq!(requested > 0, crawl, "{what}: Anfrage");

            match (store, crawl) {
                (true, true) => {
                    let row = url_row(&fx, &url).unwrap_or_else(|| panic!("{what}: Zeile fehlt"));
                    assert!(row.in_tables, "{what}");
                    assert_eq!(row.state, "done", "{what}");
                    assert_eq!(row.status, Some(200), "{what}");
                }
                (true, false) => {
                    // Speichern an, Crawlen aus: in den Tabellen, ohne Statuscode, nicht angefragt.
                    let row = url_row(&fx, &url).unwrap_or_else(|| panic!("{what}: Zeile fehlt"));
                    assert!(row.in_tables, "{what}");
                    assert_ne!(row.state, "done", "{what}");
                    assert_eq!(row.status, None, "{what}");
                    let expected = if case.external {
                        "external_not_checked"
                    } else {
                        "store_only"
                    };
                    assert_eq!(row.reason.as_deref(), Some(expected), "{what}");
                }
                (false, true) => {
                    // Speichern aus, Crawlen an: angefragt, aber nicht in den Tabellen.
                    assert!(!in_tables(&fx, &url), "{what}");
                }
                (false, false) => {
                    // Beide aus: weder angefragt noch angezeigt, der Link ist nicht in den Outlinks.
                    assert!(url_row(&fx, &url).is_none(), "{what}");
                    if let Some(raw) = case.link_raw {
                        assert_eq!(link_rows(&fx, raw), 0, "{what}: Outlink");
                    }
                }
            }
            if let (true, Some(raw)) = (store, case.link_raw) {
                assert_eq!(link_rows(&fx, raw), 1, "{what}: Outlink");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-039

// spec: CRAWL-039
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn images_are_fetched_by_default_and_not_when_image_crawl_is_off() {
    let fx = crawl_fixture("crawl-store-crawl-matrix", CrawlConfig::default()).await;
    assert_eq!(hits(&fx, "/m/bild.png"), 1);
    let row = url_row(&fx, &fx.site.url("/m/bild.png")).expect("Bild in der Crawl-Datei");
    assert!(row.in_tables);
    assert_eq!(row.status, Some(200));
    assert_eq!(
        count(&fx, "SELECT COUNT(*) FROM links WHERE link_type = 'Image'"),
        1
    );

    let fx = crawl_fixture(
        "crawl-store-crawl-matrix",
        with(|c| c.spider.images = StoreCrawl::STORE_ONLY),
    )
    .await;
    assert_eq!(hits(&fx, "/m/bild.png"), 0);
    assert!(in_tables(&fx, &fx.site.url("/m/bild.png")));
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-044 / CRAWL-045

// spec: CRAWL-044
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn internal_links_are_crawled_once_and_counted_per_instance() {
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    assert_eq!(hits(&fx, "/a"), 1, "drei Links auf /a, ein Abruf");
    assert_eq!(hits(&fx, "/b"), 1);
    assert_eq!(hits(&fx, "/area-ziel"), 1, "<area> wirkt wie <a>");
    // Jede Instanz bleibt in den Outlinks der Quellseite.
    assert_eq!(link_rows(&fx, "/a"), 3);
    // `<a>` ohne href, `href="#"`, mailto:, tel: und javascript: erzeugen keine neue URL.
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND url NOT LIKE '%/robots.txt'"
        ),
        4,
        "Startseite, /a, /b und /area-ziel"
    );
}

// spec: CRAWL-044
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn internal_crawl_off_requests_only_the_start_url() {
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| c.spider.internal = StoreCrawl::STORE_ONLY),
    )
    .await;
    for path in ["/a", "/b", "/area-ziel"] {
        assert_eq!(hits(&fx, path), 0, "{path}");
    }
    let row = url_row(&fx, &fx.site.url("/a")).expect("/a bleibt gespeichert");
    assert!(row.in_tables);
    assert_eq!(row.status, None);
}

// spec: CRAWL-045
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn external_links_are_checked_once_and_not_followed() {
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    assert_eq!(ext_hits(&fx, "/ext"), 1);
    let row = url_row(&fx, "http://external.test/ext").expect("externe URL");
    assert_eq!(row.status, Some(200));
    assert!(row.in_tables);
}

// spec: CRAWL-045
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn external_crawl_off_sends_no_request_to_external_hosts() {
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| c.spider.external = StoreCrawl::STORE_ONLY),
    )
    .await;
    assert_eq!(ext_hits(&fx, "/ext"), 0);
    assert!(fx.site.requests().iter().all(|r| r.host != "external.test"));
    let row = url_row(&fx, "http://external.test/ext").expect("externe URL bleibt gespeichert");
    assert_eq!(row.status, None);
    assert!(row.in_tables);
    // Interne Seiten sind davon nicht betroffen.
    assert_eq!(hits(&fx, "/a"), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-046

// spec: CRAWL-046
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn canonical_targets_are_fetched_by_default() {
    let fx = crawl_fixture("crawl-canonicals", CrawlConfig::default()).await;
    for path in [
        "/nur-canonical",
        "/nur-header-canonical",
        "/nur-canonical-a",
        "/nur-canonical-b",
        "/nur-body-canonical",
        "/nur-canonical-gross",
        "/nur-canonical-multi",
    ] {
        assert_eq!(hits(&fx, path), 1, "{path}");
    }
    // Mehrere Canonical-Elemente werden alle erfasst; das Canonical im body ist als „außerhalb von head“ markiert.
    let id = |p: &str| {
        count(
            &fx,
            &format!(
                "SELECT id FROM urls WHERE url = '{}'",
                sql_quote(&fx.site.url(p))
            ),
        )
    };
    let canon = |p: &str| {
        count(
            &fx,
            &format!("SELECT COUNT(*) FROM canonicals WHERE url_id = {}", id(p)),
        )
    };
    assert_eq!(canon("/c1"), 1);
    assert_eq!(canon("/c2"), 1);
    assert_eq!(canon("/c3"), 2);
    assert_eq!(canon("/c5"), 1, "rel=\"Canonical\" in Großbuchstaben");
    assert_eq!(canon("/c6"), 1, "rel=\"canonical nofollow\"");
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT in_head FROM canonicals WHERE url_id = {}",
                id("/c4")
            )
        ),
        0
    );
}

// spec: CRAWL-046
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn canonical_crawl_off_keeps_the_column_but_requests_nothing() {
    let fx = crawl_fixture(
        "crawl-canonicals",
        with(|c| c.spider.canonicals = StoreCrawl::STORE_ONLY),
    )
    .await;
    for path in [
        "/nur-canonical",
        "/nur-header-canonical",
        "/nur-canonical-a",
        "/nur-body-canonical",
    ] {
        assert_eq!(hits(&fx, path), 0, "{path}");
        let row = url_row(&fx, &fx.site.url(path)).expect("Canonical-Ziel bleibt gespeichert");
        assert_eq!(row.status, None, "{path}");
    }
    let c1 = count(
        &fx,
        &format!(
            "SELECT COUNT(*) FROM urls WHERE url = '{}' AND canonical_url = '{}'",
            sql_quote(&fx.site.url("/c1")),
            sql_quote(&fx.site.url("/nur-canonical"))
        ),
    );
    assert_eq!(c1, 1, "Canonical-Spalte von /c1");
}

// spec: CRAWL-046
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn canonical_store_off_leaves_columns_and_tab_empty() {
    let fx = crawl_fixture(
        "crawl-canonicals",
        with(|c| c.spider.canonicals = StoreCrawl::OFF),
    )
    .await;
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM canonicals"), 0);
    assert_eq!(
        count(&fx, "SELECT COUNT(*) FROM urls WHERE canonical_count > 0"),
        0
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE canonical_url IS NOT NULL"
        ),
        0
    );
    for path in ["/nur-canonical", "/nur-header-canonical"] {
        assert_eq!(hits(&fx, path), 0, "{path}");
        assert!(url_row(&fx, &fx.site.url(path)).is_none(), "{path}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-056

const INTERNAL_NOFOLLOW: [&str; 4] = [
    "/nur-nofollow",
    "/nur-sponsored",
    "/nur-ugc",
    "/nofollow-noopener",
];
const EXTERNAL_NOFOLLOW: [&str; 3] = ["/ext-nofollow", "/ext-sponsored", "/ext-ugc"];

// spec: CRAWL-056
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn nofollow_links_are_stored_with_follow_false_but_not_requested() {
    let fx = crawl_fixture("crawl-nofollow", CrawlConfig::default()).await;
    for path in INTERNAL_NOFOLLOW {
        assert_eq!(hits(&fx, path), 0, "{path}");
        let row = url_row(&fx, &fx.site.url(path)).expect("nofollow-Ziel bleibt gespeichert");
        assert!(row.in_tables, "{path}");
        assert_eq!(row.status, None, "{path}");
        assert_eq!(row.reason.as_deref(), Some("nofollow"), "{path}");
        assert_eq!(
            count(
                &fx,
                &format!("SELECT follow FROM links WHERE target_raw = '{path}'")
            ),
            0,
            "{path}: Folgen = false"
        );
    }
    for path in EXTERNAL_NOFOLLOW {
        assert_eq!(ext_hits(&fx, path), 0, "{path}");
    }
    // Links ohne rel und die Gegenprobe werden angefragt.
    assert_eq!(hits(&fx, "/frei"), 1);
    assert_eq!(ext_hits(&fx, "/ext-follow"), 1);
    // Derselbe Link zusätzlich als Follow-Link: angefragt.
    assert_eq!(hits(&fx, "/auch-follow"), 1);
    // Auf Seiten mit Nofollow-Direktive (Meta, X-Robots-Tag, googlebot) wird keiner ihrer Links verfolgt.
    for page in [
        "/meta-nofollow",
        "/header-nofollow",
        "/googlebot-nofollow",
        "/header-googlebot-nofollow",
    ] {
        assert_eq!(hits(&fx, page), 1, "{page} selbst wird abgerufen");
    }
    for behind in [
        "/hinter-meta",
        "/hinter-header",
        "/hinter-googlebot",
        "/hinter-header-googlebot",
    ] {
        assert_eq!(hits(&fx, behind), 0, "{behind}");
    }
}

// spec: CRAWL-056
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn follow_internal_nofollow_requests_internal_targets_only() {
    let fx = crawl_fixture(
        "crawl-nofollow",
        with(|c| c.spider.follow_internal_nofollow = true),
    )
    .await;
    for path in INTERNAL_NOFOLLOW {
        assert_eq!(hits(&fx, path), 1, "{path}");
    }
    for path in ["/hinter-meta", "/hinter-header", "/hinter-googlebot"] {
        assert_eq!(hits(&fx, path), 1, "{path}: Seiten-nofollow aufgehoben");
    }
    for path in EXTERNAL_NOFOLLOW {
        assert_eq!(ext_hits(&fx, path), 0, "{path}");
    }
    // Die Anzeige bleibt: Folgen = false.
    assert_eq!(
        count(
            &fx,
            "SELECT follow FROM links WHERE target_raw = '/nur-nofollow'"
        ),
        0
    );
}

// spec: CRAWL-056
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn follow_external_nofollow_requests_external_targets_only() {
    let fx = crawl_fixture(
        "crawl-nofollow",
        with(|c| c.spider.follow_external_nofollow = true),
    )
    .await;
    for path in EXTERNAL_NOFOLLOW {
        assert_eq!(ext_hits(&fx, path), 1, "{path}");
    }
    for path in INTERNAL_NOFOLLOW {
        assert_eq!(hits(&fx, path), 0, "{path}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// TECH-210: Sitemap-Einbindung in den Crawl

// spec: TECH-210
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn robots_sitemap_lines_are_followed_when_linked_sitemaps_are_crawled() {
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.sitemaps.auto_discover_robots = true;
        }),
    )
    .await;
    // robots.txt → Sitemap-Index → Sitemap → die nicht verlinkte Seite.
    assert_eq!(hits(&fx, "/sitemap-index.xml"), 1);
    assert_eq!(hits(&fx, "/sitemap.xml"), 1);
    assert_eq!(hits(&fx, "/nur-in-sitemap"), 1);
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM sitemaps WHERE kind = 'index' AND depth = 0 AND found_via = 'robots'"
        ),
        1
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM sitemaps WHERE kind = 'urlset' AND depth = 1 AND entries = 2"
        ),
        1
    );
}

// spec: TECH-210
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn robots_sitemap_lines_are_ignored_by_default_and_without_autodiscovery() {
    // Standard: „Verlinkte XML-Sitemaps crawlen“ ist aus.
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    assert_eq!(hits(&fx, "/sitemap-index.xml"), 0);
    assert_eq!(hits(&fx, "/nur-in-sitemap"), 0);
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM sitemaps"), 0);

    // Ohne Autodiscovery über robots.txt ebenso.
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.sitemaps.auto_discover_robots = false;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/sitemap-index.xml"), 0);
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM sitemaps"), 0);

    // robots.txt ignoriert: keine Autodiscovery.
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.robots.mode = RobotsMode::Ignore;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/robots.txt"), 0);
    assert_eq!(hits(&fx, "/sitemap-index.xml"), 0);
}

// spec: TECH-210
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn sitemap_as_start_url_is_processed_without_crawl_linked() {
    // Start-URL ist eine Sitemap-Index-Datei: sie wird als Sitemap ausgewertet (kein HTML-Crawl), ihre
    // Unter-Sitemap und deren URLs fließen in die Warteschlange; `/nur-in-sitemap` hat keine Crawl-Tiefe.
    let fx = os_engine::testing::crawl_fixture_from(
        "crawl-links-basic",
        "/sitemap-index.xml",
        CrawlConfig::default(),
    )
    .await;
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM sitemaps"), 2);
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM sitemaps WHERE kind = 'index' AND depth = 0 AND found_via = 'start'"
        ),
        1
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM sitemaps WHERE kind = 'urlset' AND depth = 1 AND entries = 2"
        ),
        1
    );
    assert_eq!(hits(&fx, "/nur-in-sitemap"), 1);
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT COUNT(*) FROM urls WHERE url = '{}' AND crawl_depth IS NULL AND status_code = 200",
                sql_quote(&fx.site.url("/nur-in-sitemap"))
            )
        ),
        1,
        "Sitemap-only-URL: abgerufen, ohne Crawl-Tiefe"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-098 / CRAWL-099: robots.txt auf Request-Log-Ebene (Engine-Anteil, Nachtrag Welle-1-Sichtung)

// spec: CRAWL-098
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn robots_respect_blocks_with_status_zero_and_lists_robots_txt_as_url() {
    let fx = crawl_fixture("crawl-robots", CrawlConfig::default()).await;
    assert_eq!(
        hits(&fx, "/robots.txt"),
        1,
        "robots.txt je Host genau einmal"
    );
    assert_eq!(hits(&fx, "/privat/x"), 0);
    assert_eq!(hits(&fx, "/frei/a"), 1);
    assert_eq!(
        hits(&fx, "/privat/oeffentlich"),
        1,
        "Allow ist länger als Disallow"
    );

    let blocked = fx.site.url("/privat/x");
    let sql = format!(
        "SELECT COUNT(*) FROM urls WHERE url = '{}' AND status_code = 0 AND status = 'Blocked by Robots.txt' \
         AND robots_blocked = 1 AND robots_matched_line IS NOT NULL AND robots_txt_url = '{}'",
        sql_quote(&blocked),
        sql_quote(&fx.site.url("/robots.txt"))
    );
    assert_eq!(
        count(&fx, &sql),
        1,
        "gesperrte URL mit Status 0, Zeile und robots.txt-URL"
    );

    // Die robots.txt erscheint selbst als URL-Zeile.
    let robots = url_row(&fx, &fx.site.url("/robots.txt")).expect("robots.txt als URL-Zeile");
    assert_eq!(robots.status, Some(200));
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM robots_files"), 1);
}

// spec: CRAWL-099
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn robots_ignore_never_fetches_robots_txt() {
    let fx = crawl_fixture("crawl-robots", with(|c| c.robots.mode = RobotsMode::Ignore)).await;
    assert_eq!(hits(&fx, "/robots.txt"), 0);
    assert_eq!(hits(&fx, "/privat/x"), 1);
    let row = url_row(&fx, &fx.site.url("/privat/x")).expect("/privat/x");
    assert_eq!(row.status, Some(200));
}

// spec: CRAWL-099
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn robots_ignore_but_report_fetches_and_marks_would_be_blocked() {
    let fx = crawl_fixture(
        "crawl-robots",
        with(|c| c.robots.mode = RobotsMode::IgnoreButReport),
    )
    .await;
    assert_eq!(hits(&fx, "/robots.txt"), 1);
    assert_eq!(hits(&fx, "/privat/x"), 1);
    let sql = format!(
        "SELECT COUNT(*) FROM urls WHERE url = '{}' AND status_code = 200 AND robots_blocked = 1 \
         AND robots_matched_line IS NOT NULL",
        sql_quote(&fx.site.url("/privat/x"))
    );
    assert_eq!(
        count(&fx, &sql),
        1,
        "echter Statuscode 200 und Markierung „wäre blockiert“"
    );
    let free = format!(
        "SELECT COUNT(*) FROM urls WHERE url = '{}' AND robots_blocked = 0",
        sql_quote(&fx.site.url("/frei/a"))
    );
    assert_eq!(count(&fx, &free), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-109 / CRAWL-110: Include/Exclude auf Engine-Ebene (os_url::rules entscheidet, P15 wendet an)

// spec: CRAWL-109
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn include_rules_limit_the_crawl_but_not_the_start_url() {
    let fx = crawl_fixture(
        "crawl-include-exclude",
        with(|c| c.scope.include = vec![".*/blog/.*".to_string()]),
    )
    .await;
    assert_eq!(hits(&fx, "/"), 1, "die Start-URL wird immer gecrawlt");
    assert_eq!(hits(&fx, "/blog/a"), 1);
    assert_eq!(hits(&fx, "/blog/b"), 1);
    assert_eq!(hits(&fx, "/shop/x"), 0);
    assert_eq!(hits(&fx, "/Blog/c"), 0, "Groß-/Kleinschreibung zählt");
    assert_eq!(
        hits(&fx, "/img/a.png"),
        0,
        "Include wirkt auch auf Ressourcen"
    );
    // Nicht gecrawlte URLs bleiben als Outlink der Startseite sichtbar.
    assert_eq!(link_rows(&fx, "/shop/x"), 1);
}

// spec: CRAWL-110
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn exclude_rules_skip_matching_urls_but_keep_the_outlink() {
    let fx = crawl_fixture(
        "crawl-include-exclude",
        with(|c| {
            c.scope.exclude = vec![
                r".*\.pdf".to_string(),
                r".*[?&]sortierung=.*".to_string(),
                ".*%20.*".to_string(),
            ];
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/handbuch.pdf"), 0);
    assert_eq!(fx.site.count("127.0.0.1", "/handbuch.pdf?v=2"), 1);
    assert_eq!(fx.site.count("127.0.0.1", "/liste?sortierung=preis"), 0);
    assert_eq!(
        fx.site
            .count("127.0.0.1", "/liste?seite=2&sortierung=preis"),
        0
    );
    assert_eq!(fx.site.count("127.0.0.1", "/liste?seite=2"), 1);
    assert_eq!(hits(&fx, "/a%20b"), 0);
    assert_eq!(link_rows(&fx, "/handbuch.pdf"), 1, "bleibt Outlink");
}

// spec: CRAWL-110
#[tokio::test]
#[ignore = "wartet auf M1-P15"]
async fn exclude_with_full_url_pattern_spares_other_hosts() {
    // Port der Testsite ist vorab unbekannt: `[0-9]+` ersetzt ihn.
    let fx = crawl_fixture(
        "crawl-include-exclude",
        with(|c| {
            c.scope.exclude = vec![r"http://127\.0\.0\.1:[0-9]+/kalender/.*".to_string()];
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/kalender/2025-01"), 0);
    // blog.site.test/kalender/x liegt auf einem anderen Host (extern) und wird geprüft.
    assert_eq!(fx.site.count("blog.site.test", "/kalender/x"), 1);
}
