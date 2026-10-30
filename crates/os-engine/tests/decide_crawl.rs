//! Crawl-Integrationstests gegen die Fixtures von M1-P15b (CRAWL-038/039/044/045/046/056, TECH-210, dazu die
//! Engine-Anteile von CRAWL-098/099 und CRAWL-109/110).
//!
//! Alle Tests laufen über `os_engine::testing::crawl_fixture` gegen den echten Crawl-Kern (P15, scharf geschaltet in
//! Welle 2.5, M1-W25). Die gleichen Entscheidungen prüft `decide_fixtures.rs` ohne Engine.
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

/// Anzahl Anfragen an `host` mit genau diesem `path_and_query`. `TestSite::count` vergleicht ohne `?` im Suchpfad
/// nur den Pfad; `/handbuch.pdf?v=2` zählte dort für `/handbuch.pdf` mit. Hier gilt immer die Vollübereinstimmung.
fn exact_hits(fx: &FixtureCrawl, host: &str, path_and_query: &str) -> usize {
    fx.site
        .requests()
        .iter()
        .filter(|r| r.host == host && r.path_and_query == path_and_query)
        .count()
}

/// Anzahl Anfragen an `127.0.0.1` für genau diesen Pfad (samt Query).
fn hits(fx: &FixtureCrawl, path: &str) -> usize {
    exact_hits(fx, "127.0.0.1", path)
}

/// Anzahl Anfragen an den externen Testhost.
fn ext_hits(fx: &FixtureCrawl, path: &str) -> usize {
    exact_hits(fx, "external.test", path)
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

/// Speichern aus, Crawlen an.
const CRAWL_ONLY: StoreCrawl = StoreCrawl {
    store: false,
    crawl: true,
};

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
                    // Speichern an, Crawlen aus: in den Tabellen, ohne Statuscode, nicht angefragt. Ausnahme
                    // hreflang (K-62) und Paginierung (SQ-12): keine Tabellen-Sichtbarkeit.
                    let row = url_row(&fx, &url).unwrap_or_else(|| panic!("{what}: Zeile fehlt"));
                    assert_eq!(
                        row.in_tables,
                        !matches!(case.name, "hreflang" | "rel=next"),
                        "{what}"
                    );
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

// spec: OUT-031
#[tokio::test]
async fn target_raw_is_the_markup_value_and_every_stored_link_has_a_target_row() {
    // K-37: `links.target_raw` trägt den Rohwert des Attributs, das aufgelöste Ziel steht über `target_id`.
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    let raw: Vec<String> = {
        let mut stmt = fx
            .file
            .conn()
            .prepare("SELECT target_raw FROM links ORDER BY position")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        raw,
        [
            "/a",
            "/a",
            "/a",
            "/b",
            "/area-ziel",
            "#",
            "http://external.test/ext"
        ],
        "Rohwerte wie im Markup, nicht die aufgelösten Adressen"
    );
    // Das aufgelöste Ziel liefert die Zeile in `urls`.
    let resolved: String = fx
        .file
        .conn()
        .query_row(
            "SELECT u.url FROM links l JOIN urls u ON u.id = l.target_id WHERE l.target_raw = '/b'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(resolved, fx.site.url("/b"));
    // Nicht auflösbare Links (mailto:, tel:, javascript:, ohne href) erzeugen weder URL- noch Linkzeile; `#`
    // zeigt auf die eigene Seite und hat deren Zeile als Ziel.
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links WHERE target_raw LIKE 'mailto:%' OR target_raw LIKE 'tel:%' \
             OR target_raw LIKE 'javascript:%'"
        ),
        0
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links l WHERE l.target_raw = '#' AND l.target_id = l.source_id"
        ),
        1
    );
    // Invariante: jeder gespeicherte Link hat eine Zielzeile (im selben oder einem früheren Batch).
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links l LEFT JOIN urls u ON u.id = l.target_id WHERE u.id IS NULL"
        ),
        0
    );

    // Auch Ziele, die Include ausschließt, haben eine Zielzeile – außerhalb der Tabellen.
    let fx = crawl_fixture(
        "crawl-include-exclude",
        with(|c| c.scope.include = vec![".*/blog/.*".to_string()]),
    )
    .await;
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links l LEFT JOIN urls u ON u.id = l.target_id WHERE u.id IS NULL"
        ),
        0
    );
    let shop = url_row(&fx, &fx.site.url("/shop/x")).expect("Zielzeile des abgelehnten Links");
    assert!(!shop.in_tables);
    assert_eq!(shop.reason.as_deref(), Some("not_included"));
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links l JOIN urls u ON u.id = l.target_id \
             WHERE l.target_raw = '/shop/x' AND u.state = 'discovered'"
        ),
        1
    );
}

// spec: CRAWL-045
#[tokio::test]
async fn external_links_are_checked_once_and_not_followed() {
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    assert_eq!(ext_hits(&fx, "/ext"), 1);
    let row = url_row(&fx, "http://external.test/ext").expect("externe URL");
    assert_eq!(row.status, Some(200));
    assert!(row.in_tables);
}

// spec: CRAWL-045
#[tokio::test]
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
async fn canonical_targets_are_fetched_by_default() {
    let fx = crawl_fixture("crawl-canonicals", CrawlConfig::default()).await;
    for path in [
        "/nur-canonical",
        "/nur-header-canonical",
        "/nur-canonical-a",
        "/nur-canonical-b",
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
    // SQ-04: Body-Canonicals hinterlassen nur den Problembeleg.
    assert_eq!(hits(&fx, "/nur-body-canonical"), 0);
    assert_eq!(canon("/c4"), 0);
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT v_int FROM url_ext WHERE url_id = {} AND key = 'canonical.outside_head'",
                id("/c4")
            )
        ),
        1
    );
}

// spec: CRAWL-046
#[tokio::test]
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

// spec: CRAWL-046
#[tokio::test]
async fn canonical_store_off_with_crawl_on_requests_the_target_but_leaves_every_column_empty() {
    // Die Startseite hat ein Canonical auf `/m/canonical.html`.
    let fx = crawl_fixture("crawl-store-crawl-matrix", CrawlConfig::default()).await;
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT COUNT(*) FROM urls WHERE url = '{}' AND canonical_count = 1 AND canonical_url = '{}' \
                 AND canonical_target_id IS NOT NULL AND canonical_self = 0",
                sql_quote(&fx.site.url("/")),
                sql_quote(&fx.site.url("/m/canonical.html"))
            )
        ),
        1,
        "Gegenprobe: mit Speichern an trägt die Startseite ihr Canonical"
    );

    let fx = crawl_fixture(
        "crawl-store-crawl-matrix",
        with(|c| c.spider.canonicals = CRAWL_ONLY),
    )
    .await;
    assert_eq!(hits(&fx, "/m/canonical.html"), 1, "Crawlen bleibt an");
    assert!(
        !in_tables(&fx, &fx.site.url("/m/canonical.html")),
        "gecrawlt, aber nicht gespeichert"
    );
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM canonicals"), 0);
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links WHERE link_type IN ('Canonical', 'HttpCanonical')"
        ),
        0
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE canonical_count > 0 OR canonical_url IS NOT NULL \
             OR canonical_target_id IS NOT NULL OR canonical_self IS NOT NULL"
        ),
        0
    );
    // CRAWL-046: kein Canonical in der Indexierbarkeit, also nie „Canonicalised“ (echtes `evaluate`, P24).
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE indexability_status = 'Canonicalised'"
        ),
        0
    );
    // Die Startseite trägt zusätzlich einen Meta Refresh auf eine andere URL: nach TECH-001 (M1-P24) ist sie
    // `Redirected` – aber eben nicht `Canonicalised`.
    let start: String = fx
        .file
        .conn()
        .query_row(
            "SELECT indexability_status FROM urls WHERE url = ?1",
            [fx.site.url("/")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(start, "Redirected");
    assert_ne!(start, "Canonicalised");

    // Gegenprobe auf einer Seite ohne Meta Refresh: `/c1` trägt ein Canonical auf `/nur-canonical`. Mit
    // Speichern an ist sie `Canonicalised`, mit Speichern aus (Crawlen an) nicht, sondern indexierbar.
    let status_of_c1 = |fx: &FixtureCrawl| -> (String, Option<String>) {
        fx.file
            .conn()
            .query_row(
                "SELECT indexability, indexability_status FROM urls WHERE url = ?1",
                [fx.site.url("/c1")],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    let fx = crawl_fixture("crawl-canonicals", CrawlConfig::default()).await;
    assert_eq!(
        status_of_c1(&fx),
        (
            "Non-Indexable".to_string(),
            Some("Canonicalised".to_string())
        ),
        "store = true: Canonicalised"
    );
    let fx = crawl_fixture(
        "crawl-canonicals",
        with(|c| c.spider.canonicals = CRAWL_ONLY),
    )
    .await;
    assert_eq!(hits(&fx, "/nur-canonical"), 1, "Crawlen bleibt an");
    assert_eq!(
        status_of_c1(&fx),
        ("Indexable".to_string(), None),
        "store = false: nie Canonicalised"
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE indexability_status = 'Canonicalised'"
        ),
        0
    );
}

// spec: CRAWL-038
#[tokio::test]
async fn hreflang_and_pagination_store_off_leave_detail_tables_and_columns_empty() {
    let fx = crawl_fixture("crawl-store-crawl-matrix", CrawlConfig::default()).await;
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 1, "Gegenprobe");
    assert_eq!(
        count(&fx, "SELECT COUNT(*) FROM pagination"),
        2,
        "Gegenprobe"
    );

    let fx = crawl_fixture(
        "crawl-store-crawl-matrix",
        with(|c| {
            c.spider.hreflang = CRAWL_ONLY;
            c.spider.pagination = CRAWL_ONLY;
        }),
    )
    .await;
    for path in ["/m/hreflang-fr.html", "/m/next.html", "/m/prev.html"] {
        assert_eq!(hits(&fx, path), 1, "{path}: Crawlen bleibt an");
        assert!(!in_tables(&fx, &fx.site.url(path)), "{path}");
    }
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 0);
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM pagination"), 0);
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links WHERE link_type IN ('Hreflang', 'HttpHreflang', 'RelNext', 'RelPrev')"
        ),
        0
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM urls WHERE hreflang_count > 0 OR has_rel_next = 1 OR has_rel_prev = 1"
        ),
        0
    );
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

// spec: CRAWL-056, ONPAGE-025
#[tokio::test]
async fn nofollow_links_are_stored_with_follow_false_but_not_requested() {
    let fx = crawl_fixture("crawl-nofollow", CrawlConfig::default()).await;
    for path in INTERNAL_NOFOLLOW {
        assert_eq!(hits(&fx, path), 0, "{path}");
        let row = url_row(&fx, &fx.site.url(path)).expect("nofollow-Ziel bleibt gespeichert");
        assert!(!row.in_tables, "{path}");
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
        assert!(
            in_tables(&fx, &format!("http://external.test{path}")),
            "{path}"
        );
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

// spec: CRAWL-056, ONPAGE-025
#[tokio::test]
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
        assert!(
            in_tables(&fx, &format!("http://external.test{path}")),
            "{path}"
        );
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

// spec: TECH-210
#[tokio::test]
async fn every_sitemap_file_is_requested_exactly_once_and_is_not_a_table_row() {
    // K-41a: Start-URL = Sitemap-Index, dieselbe Datei steht zusätzlich in der robots.txt (Autodiscovery).
    // Die Dateien holt nur der Sitemap-Ingest, je Crawl genau einmal.
    let fx = os_engine::testing::crawl_fixture_from(
        "crawl-links-basic",
        "/sitemap-index.xml",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.sitemaps.auto_discover_robots = true;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/sitemap-index.xml"), 1);
    assert_eq!(hits(&fx, "/sitemap.xml"), 1);
    assert_eq!(hits(&fx, "/nur-in-sitemap"), 1);
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM sitemaps"), 2);
    // Die URL-Zeilen der Dateien tragen den Status des Ingest-Abrufs und stehen nicht in den Tabellen.
    for path in ["/sitemap-index.xml", "/sitemap.xml"] {
        let row = url_row(&fx, &fx.site.url(path)).expect(path);
        assert_eq!(row.status, Some(200), "{path}");
        assert_eq!(row.state, "done", "{path}");
        assert!(!row.in_tables, "{path}");
    }
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT COUNT(*) FROM urls WHERE url = '{}' AND discovered_via = 'start' AND crawl_depth = 0",
                sql_quote(&fx.site.url("/sitemap-index.xml"))
            )
        ),
        1
    );
}

// spec: TECH-210
#[tokio::test]
async fn sitemap_hreflang_is_stored_per_loc_url_with_source_sitemap() {
    // `sitemap.xml` führt zu `/a` drei `xhtml:link`-Angaben (de → /a, en → /b, x-default → /nur-in-sitemap);
    // zweite Quelle für dieselbe URL gibt es nicht, die Angaben bleiben vollständig und fortlaufend nummeriert.
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.sitemaps.auto_discover_robots = true;
            // hreflang crawlen: die Ziele werden über `decide` wie HTML-hreflang behandelt
            c.spider.hreflang = StoreCrawl::ON;
        }),
    )
    .await;
    assert_eq!(
        count(&fx, "SELECT COUNT(*) FROM sitemap_entries"),
        2,
        "entries bleibt 2"
    );
    let sitemap_id = count(&fx, "SELECT id FROM sitemaps WHERE kind = 'urlset'");
    let a = fx.site.url("/a");
    type Row = (i64, String, String, i64, Option<String>, Option<i64>);
    let rows: Vec<Row> = {
        let mut stmt = fx
            .file
            .conn()
            .prepare(
                "SELECT h.position, h.code_norm, h.source, h.sitemap_id, h.url, h.target_id \
                 FROM hreflang h JOIN urls u ON u.id = h.url_id WHERE u.url = ?1 ORDER BY h.position",
            )
            .unwrap();
        stmt.query_map([&a], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(
        rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
        ["de", "en", "x-default"]
    );
    assert_eq!(
        rows.iter().map(|r| r.0).collect::<Vec<_>>(),
        [0, 1, 2],
        "position = order, fortlaufend"
    );
    assert!(rows.iter().all(|r| r.2 == "sitemap" && r.3 == sitemap_id));
    assert_eq!(rows[0].4.as_deref(), Some(a.as_str()));
    assert_eq!(rows[1].4.as_deref(), Some(fx.site.url("/b").as_str()));
    // target_id: jede Zielzeile existiert (die Ziele sind verlinkt bzw. stehen in der Sitemap).
    assert!(rows.iter().all(|r| r.5.is_some()), "{rows:?}");
    let target_of_en: String = fx
        .file
        .conn()
        .query_row(
            "SELECT u.url FROM hreflang h JOIN urls u ON u.id = h.target_id WHERE h.code_norm = 'en'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(target_of_en, fx.site.url("/b"));
    // Die Angaben gehören der Sitemap, nicht der Seite: andere Quellen sind nicht betroffen.
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM hreflang WHERE source <> 'sitemap'"
        ),
        0
    );

    // hreflang speichern aus: keine Zeilen.
    let fx = crawl_fixture(
        "crawl-links-basic",
        with(|c| {
            c.sitemaps.crawl_linked = true;
            c.sitemaps.auto_discover_robots = true;
            c.spider.hreflang = CRAWL_ONLY;
        }),
    )
    .await;
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 0);
}

// spec: CRAWL-038
#[tokio::test]
async fn sitemap_hreflang_with_store_off_and_crawl_on_requests_the_target_without_rows() {
    // `/nur-hreflang` steht nur als `xhtml:link` in hreflang-sitemap.xml. Mit hreflang „nur crawlen“ wird das
    // Ziel über decide/discover eingeplant und abgerufen (genau einmal), die hreflang-Tabelle bleibt leer;
    // mit „nur speichern“ ist es umgekehrt.
    let sitemaps = |c: &mut CrawlConfig| {
        c.sitemaps.crawl_linked = true;
        c.sitemaps.auto_discover_robots = true;
    };
    let fx = crawl_fixture(
        "crawl-sitemaps-foreign",
        with(|c| {
            sitemaps(c);
            c.spider.hreflang = CRAWL_ONLY;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/hreflang-sitemap.xml"), 1);
    assert_eq!(hits(&fx, "/nur-hreflang"), 1, "Ziel abgerufen");
    let row = url_row(&fx, &fx.site.url("/nur-hreflang")).expect("Zielzeile");
    assert_eq!(row.status, Some(200));
    assert!(!row.in_tables, "gespeichert wird nichts");
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 0);

    let fx = crawl_fixture(
        "crawl-sitemaps-foreign",
        with(|c| {
            sitemaps(c);
            c.spider.hreflang = StoreCrawl::STORE_ONLY;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/nur-hreflang"), 0, "nur speichern: kein Abruf");
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 2);

    let fx = crawl_fixture(
        "crawl-sitemaps-foreign",
        with(|c| {
            sitemaps(c);
            c.spider.hreflang = StoreCrawl::OFF;
        }),
    )
    .await;
    assert_eq!(hits(&fx, "/nur-hreflang"), 0, "aus: kein Abruf");
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM hreflang"), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// CRAWL-098 / CRAWL-099: robots.txt auf Request-Log-Ebene (Engine-Anteil, Nachtrag Welle-1-Sichtung)

// spec: CRAWL-098
#[tokio::test]
async fn robots_respect_blocks_with_status_zero_and_lists_robots_txt_as_url() {
    // K2-64: die Fixture-robots.txt hat `Crawl-delay: 2`; die Aussagen hängen nicht vom Abstand ab.
    let fx = crawl_fixture(
        "crawl-robots",
        with(|c| c.robots.respect_crawl_delay = false),
    )
    .await;
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

// spec: CRAWL-098
#[tokio::test]
async fn robots_txt_of_every_host_has_a_row_but_no_table_visibility() {
    // `crawl-links-basic` verlinkt `external.test`; dessen robots.txt wird ebenfalls geholt (K-40).
    let fx = crawl_fixture("crawl-links-basic", CrawlConfig::default()).await;
    assert_eq!(hits(&fx, "/robots.txt"), 1);
    assert_eq!(ext_hits(&fx, "/robots.txt"), 1);

    let internal = fx.site.url("/robots.txt");
    let row = url_row(&fx, &internal).expect("robots.txt des internen Hosts");
    assert!(
        !row.in_tables,
        "K-54: auch intern nicht in den Tabellen (die Zeile existiert)"
    );
    assert_eq!(row.status, Some(200));
    let (content_type, class): (String, String) = fx
        .file
        .conn()
        .query_row(
            "SELECT content_type, content_class FROM urls WHERE url = ?1",
            [&internal],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        content_type.starts_with("text/plain"),
        "Inhaltstyp Text: {content_type}"
    );
    assert_ne!(class, "HTML", "die robots.txt ist keine HTML-Seite");
    assert_eq!(
        count(
            &fx,
            &format!(
                "SELECT is_internal FROM urls WHERE url = '{}'",
                sql_quote(&internal)
            )
        ),
        1
    );

    let external = url_row(&fx, "http://external.test/robots.txt")
        .expect("robots.txt des externen Hosts bleibt als URL-Zeile");
    assert!(!external.in_tables, "extern: nicht in den Tabellen (K-40)");
    assert_eq!(external.status, Some(200));
    assert_eq!(count(&fx, "SELECT COUNT(*) FROM robots_files"), 2);
}

// spec: CRAWL-099
#[tokio::test]
async fn robots_ignore_never_fetches_robots_txt() {
    let fx = crawl_fixture("crawl-robots", with(|c| c.robots.mode = RobotsMode::Ignore)).await;
    assert_eq!(hits(&fx, "/robots.txt"), 0);
    assert_eq!(hits(&fx, "/privat/x"), 1);
    let row = url_row(&fx, &fx.site.url("/privat/x")).expect("/privat/x");
    assert_eq!(row.status, Some(200));
}

// spec: CRAWL-099
#[tokio::test]
async fn robots_ignore_but_report_fetches_and_marks_would_be_blocked() {
    let fx = crawl_fixture(
        "crawl-robots",
        with(|c| {
            c.robots.mode = RobotsMode::IgnoreButReport;
            // K2-64: `Crawl-delay: 2` der Fixture-robots.txt, nicht Gegenstand dieses Tests.
            c.robots.respect_crawl_delay = false;
        }),
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
    assert_eq!(hits(&fx, "/handbuch.pdf?v=2"), 1);
    assert_eq!(hits(&fx, "/liste?sortierung=preis"), 0);
    assert_eq!(hits(&fx, "/liste?seite=2&sortierung=preis"), 0);
    assert_eq!(hits(&fx, "/liste?seite=2"), 1);
    assert_eq!(hits(&fx, "/a%20b"), 0);
    assert_eq!(link_rows(&fx, "/handbuch.pdf"), 1, "bleibt Outlink");
}

// spec: CRAWL-110
#[tokio::test]
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
    assert_eq!(exact_hits(&fx, "blog.site.test", "/kalender/x"), 1);
}

// spec: TECH-071
#[tokio::test]
async fn hreflang_only_targets_have_no_table_visibility_and_no_link_row_when_hreflang_crawl_is_off()
{
    // K-62 (Paritätsbefund N-08): /it/ ist nur per hreflang bekannt, /de/ zusätzlich verlinkt. Mit
    // spider.hreflang.crawl = false (Standard) hat /it/ eine Zeile, aber keine Tabellen-Sichtbarkeit und keine
    // hreflang-Linkzeile; /de/ bleibt wie bisher in den Tabellen mit Linkzeile. Die hreflang-Spalten der
    // Quellseite führen beide Ziele.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("scenario.toml"),
        r#"
[[route]]
path = "/"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>start</title><link rel=\"alternate\" hreflang=\"de\" href=\"/de/\"><link rel=\"alternate\" hreflang=\"it\" href=\"/it/\"><link rel=\"alternate\" hreflang=\"en\" href=\"/\"></head><body><a href=\"/de/\">de</a></body></html>"

[[route]]
path = "/de/"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>de</title></head><body>de</body></html>"

[[route]]
path = "/it/"
content_type = "text/html; charset=utf-8"
body = "<!doctype html><html><head><title>it</title></head><body>it</body></html>"
"#,
    )
    .unwrap();
    let crawl = |cfg: CrawlConfig| {
        os_engine::testing::crawl_dir_with(dir.path(), "/", cfg, os_engine::testing::test_deps())
    };
    let fx = crawl(CrawlConfig::default()).await;
    let it = url_row(&fx, &fx.site.url("/it/")).expect("Zeile der nur per hreflang bekannten URL");
    assert!(!it.in_tables);
    assert_eq!(it.status, None);
    assert_eq!(hits(&fx, "/it/"), 0, "nicht angefragt");
    assert_eq!(link_rows(&fx, "/it/"), 0, "keine hreflang-Linkzeile");
    let de = url_row(&fx, &fx.site.url("/de/")).expect("/de/");
    assert!(de.in_tables);
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM links WHERE target_raw = '/de/' AND link_type = 'Hreflang'"
        ),
        1
    );
    // die hreflang-Spalten der Quellseite führen weiterhin alle Alternativen, mit Verweis auf die Zielzeilen
    assert_eq!(
        count(&fx, "SELECT COUNT(*) FROM hreflang WHERE source = 'html'"),
        3
    );
    assert_eq!(
        count(
            &fx,
            "SELECT COUNT(*) FROM hreflang WHERE source = 'html' AND target_id IS NOT NULL"
        ),
        3
    );

    // mit hreflang-Crawl wie jeder Link: Abruf, Tabellen-Sichtbarkeit und Linkzeile
    let on = crawl(with(|c| c.spider.hreflang = StoreCrawl::ON)).await;
    let it = url_row(&on, &on.site.url("/it/")).expect("/it/");
    assert!(it.in_tables);
    assert_eq!(it.status, Some(200));
    assert_eq!(link_rows(&on, "/it/"), 1);
}

// spec: CRAWL-047
// spec: CRAWL-048
// spec: CRAWL-050
// spec: CRAWL-051
#[tokio::test]
async fn m2_page_link_switches_keep_all_four_store_crawl_combinations_independent() {
    for (store, crawl) in [(false, false), (false, true), (true, false), (true, true)] {
        let cfg = with(|c| {
            let switch = StoreCrawl { store, crawl };
            c.spider.pagination = switch;
            c.spider.hreflang = switch;
            c.spider.iframes = switch;
            c.spider.mobile_alternate = switch;
        });
        let fx = crawl_fixture("crawl-m2-switches/crawl-resources", cfg).await;
        for target in ["/next", "/en", "/frame", "/mobile"] {
            assert_eq!(
                hits(&fx, target),
                usize::from(crawl),
                "{target} store={store} crawl={crawl}"
            );
        }
        assert_eq!(
            count(&fx, "SELECT count(*) FROM hreflang"),
            i64::from(store)
        );
        assert_eq!(
            count(&fx, "SELECT count(*) FROM pagination"),
            i64::from(store)
        );
        assert_eq!(link_rows(&fx, "/mobile"), i64::from(store));
    }
}
