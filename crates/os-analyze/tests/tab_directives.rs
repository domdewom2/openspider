//! Tab „Directives“ (TECH-051 bis TECH-056, TECH-064) und Direktiven-Extraktion (TECH-050, TECH-066) gegen
//! einen echten Crawl der Fixture `tech/directives`.
//!
//! Die erwarteten Filtermengen stehen ausdrücklich im Test (aus den Akzeptanzkriterien abgeleitet); die
//! Golden-Dateien unter `tests/golden/tech/directives/` halten dieselben Mengen fest (TECH-007).

use std::collections::BTreeSet;
use std::path::PathBuf;

use os_analyze::defs::SortKey;
use os_analyze::views::{query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};

const TAB: &str = "Directives";

// ---------------------------------------------------------------------------------------------------------------
// Hilfen

struct Crawled {
    fx: FixtureCrawl,
    cfg: CrawlConfig,
}

async fn crawl(cfg: CrawlConfig) -> Crawled {
    let fx = crawl_fixture("tech/directives", cfg.clone()).await;
    Crawled { fx, cfg }
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Null => String::new(),
    }
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl Crawled {
    fn norm(&self, text: &str) -> String {
        text.replace(&self.fx.site.base_url(), "")
    }

    fn page(&self, filter: &str, columns: &[&str]) -> ViewPage {
        let mut q = ViewQuery::new(TAB, filter);
        q.columns = ColumnSelection::Named(columns.iter().map(|c| c.to_string()).collect());
        q.limit = 500;
        q.sort = vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }];
        query_view(&self.fx.file, &self.cfg, &q).unwrap_or_else(|e| panic!("{TAB}:{filter}: {e}"))
    }

    fn set(&self, filter: &str) -> BTreeSet<String> {
        self.page(filter, &["Address"])
            .rows
            .iter()
            .map(|r| self.norm(&text(&r[0])))
            .collect()
    }

    /// Wert einer Spalte; nummerierte Spalten (`Basis n`) werden über ihre Basis angefragt, eine Nummer jenseits
    /// des Maximums der Ansicht ergibt einen leeren Text.
    fn cell(&self, filter: &str, address: &str, column: &str) -> Value {
        let request = match column.rsplit_once(' ') {
            Some((base, n)) if n.parse::<u32>().is_ok() => base,
            _ => column,
        };
        let page = self.page(filter, &["Address", request]);
        let row = page
            .rows
            .iter()
            .find(|r| self.norm(&text(&r[0])) == address)
            .unwrap_or_else(|| panic!("{address} fehlt im Filter {filter}"));
        match page.columns.iter().position(|c| c == column) {
            Some(i) => row[i].clone(),
            None => Value::Text(String::new()),
        }
    }

    fn text_cell(&self, filter: &str, address: &str, column: &str) -> String {
        self.norm(&text(&self.cell(filter, address, column)))
    }

    fn status(&self, address: &str) -> String {
        self.text_cell("All", address, "Indexability Status")
    }
}

fn slug(filter: &str) -> String {
    filter.to_ascii_lowercase().replace([' ', '-'], "_")
}

fn golden(c: &Crawled, filters: &[&str]) {
    for filter in filters {
        let page = c.page(filter, &["Address", "Occurrences", "Indexability Status"]);
        let mut out = String::from("Address,Occurrences,Indexability Status\n");
        for r in &page.rows {
            let cells: Vec<String> = r.iter().map(|v| c.norm(&text(v))).collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "../../tests/golden/tech/directives",
            &format!("directives__{}.csv", slug(filter)),
        ]
        .iter()
        .collect();
        os_testsite::golden::assert_golden(path, &out);
    }
}

const FILTERS: &[&str] = &[
    "All", "Index", "Noindex", "Follow", "Nofollow", "None", "Refresh",
];

const ALL: &[&str] = &[
    "/",
    "/all.html",
    "/body-noindex.html",
    "/canonical-and-robots.html",
    "/conflict-meta.html",
    "/conflict-mixed.html",
    "/follow-nofollow.html",
    "/index-follow.html",
    "/max.html",
    "/mixed-case.html",
    "/nofollow.html",
    "/noindex-bingbot.html",
    "/noindex-googlebot.html",
    "/noindex-header.html",
    "/noindex-meta.html",
    "/noindex.pdf",
    "/none.html",
    "/ok.html",
    "/refresh-0.html",
    "/refresh-5-self.html",
    "/refresh-bare.html",
    "/refresh-header.html",
    "/refresh-query.html",
    "/refresh-query.html?x=1",
    "/refresh-self-0.html",
    "/refresh-self-300.html",
    "/refresh-self-fragment.html",
    "/refresh-self-header.html",
    "/unavailable-past.html",
    "/xrobots-agent.html",
    "/xrobots-two.html",
];

const INDEX: &[&str] = &[
    "/conflict-meta.html",
    "/conflict-mixed.html",
    "/index-follow.html",
];

const NOINDEX: &[&str] = &[
    "/body-noindex.html",
    "/canonical-and-robots.html",
    "/conflict-meta.html",
    "/conflict-mixed.html",
    "/mixed-case.html",
    "/noindex-googlebot.html",
    "/noindex-header.html",
    "/noindex-meta.html",
    "/noindex.pdf",
    "/xrobots-agent.html",
    "/xrobots-two.html",
];

const FOLLOW: &[&str] = &[
    "/follow-nofollow.html",
    "/index-follow.html",
    "/mixed-case.html",
];

const NOFOLLOW: &[&str] = &[
    "/follow-nofollow.html",
    "/nofollow.html",
    "/xrobots-two.html",
];

const REFRESH: &[&str] = &[
    "/refresh-0.html",
    "/refresh-5-self.html",
    "/refresh-bare.html",
    "/refresh-header.html",
    "/refresh-query.html",
    "/refresh-query.html?x=1",
    "/refresh-self-0.html",
    "/refresh-self-300.html",
    "/refresh-self-fragment.html",
    "/refresh-self-header.html",
];

// ---------------------------------------------------------------------------------------------------------------
// Grundmenge und Spalten

// spec: TECH-051
#[tokio::test]
async fn population_is_every_internal_url_with_a_2xx_response() {
    let c = crawl(CrawlConfig::default()).await;
    let all = c.set("All");
    assert_eq!(all, set(ALL));
    // SQ-03: X-Robots-Tag zählt für jeden Inhaltstyp, aber nur bei 2xx.
    assert!(all.contains("/noindex.pdf"));
    assert!(!all.contains("/gone.html"));
    // Nicht gecrawlte Ziele (hier: nur über die Nofollow-Seite erreichbar) haben keine Antwort.
    assert!(!all.contains("/only-via-nofollow-page.html"));
    // Jeder Filter ist eine Teilmenge von All.
    for filter in FILTERS {
        assert!(c.set(filter).is_subset(&all), "{filter} ⊆ All");
    }
}

// spec: TECH-051
#[tokio::test]
async fn columns_and_occurrences_follow_the_spec() {
    let c = crawl(CrawlConfig::default()).await;
    let mut q = ViewQuery::new(TAB, "All");
    q.limit = 500;
    let page = query_view(&c.fx.file, &c.cfg, &q).unwrap();
    assert_eq!(
        page.columns,
        [
            "Address",
            "Occurrences",
            "Meta Robots 1",
            "Meta Robots 2",
            "X-Robots-Tag 1",
            "X-Robots-Tag 2",
            "Meta Refresh 1",
            "Canonical Link Element 1",
            "HTTP Canonical 1",
            "Indexability",
            "Indexability Status",
        ]
    );
    // Occurrences = Meta-Robots- + X-Robots-Tag-Werte
    let occurrences = |a: &str| c.cell("All", a, "Occurrences");
    assert_eq!(occurrences("/ok.html"), Value::Int(0));
    assert_eq!(occurrences("/noindex-meta.html"), Value::Int(1));
    assert_eq!(occurrences("/conflict-meta.html"), Value::Int(2));
    assert_eq!(occurrences("/conflict-mixed.html"), Value::Int(2));
    assert_eq!(occurrences("/xrobots-two.html"), Value::Int(2));
    // Indexierbarkeit ist Standard; die effektive Wirkung bleibt eine OS-Erweiterung.
    q.columns = ColumnSelection::All;
    let all = query_view(&c.fx.file, &c.cfg, &q).unwrap();
    assert_eq!(
        &all.columns[all.columns.len() - 4..],
        [
            "Indexability",
            "Indexability Status",
            "Effective Index",
            "Effective Follow"
        ]
    );
}

// spec: TECH-050
#[tokio::test]
async fn raw_values_keep_their_spelling_agents_and_valued_directives() {
    let c = crawl(CrawlConfig::default()).await;
    let raw = |a: &str, col: &str| c.text_cell("All", a, col);

    // Schreibweise und Leerzeichen bleiben im Rohwert, die Auswertung ist case-insensitiv.
    assert_eq!(raw("/mixed-case.html", "Meta Robots 1"), "NoIndex , FOLLOW");
    assert!(c.set("Noindex").contains("/mixed-case.html"));
    assert!(c.set("Follow").contains("/mixed-case.html"));
    // Agent-spezifisches Meta mit Präfix; nur der Token des Crawlers (googlebot) wirkt.
    assert_eq!(
        raw("/noindex-googlebot.html", "Meta Robots 1"),
        "googlebot: noindex"
    );
    assert_eq!(
        raw("/noindex-bingbot.html", "Meta Robots 1"),
        "bingbot: noindex"
    );
    // X-Robots-Tag: Agent-Präfix je Abschnitt, mehrere Header in Header-Reihenfolge
    assert_eq!(
        raw("/xrobots-agent.html", "X-Robots-Tag 1"),
        "googlebot: noindex"
    );
    assert_eq!(
        raw("/xrobots-agent.html", "X-Robots-Tag 2"),
        "otherbot: nofollow"
    );
    assert_eq!(raw("/xrobots-two.html", "X-Robots-Tag 1"), "nofollow");
    assert_eq!(raw("/xrobots-two.html", "X-Robots-Tag 2"), "noindex");
    // wertbehaftete Direktiven, das Datum von unavailable_after behält sein Komma
    assert_eq!(
        raw("/max.html", "Meta Robots 1"),
        "max-snippet:50, max-image-preview:large, max-video-preview:-1"
    );
    assert_eq!(
        raw("/unavailable-past.html", "Meta Robots 1"),
        "unavailable_after: Thursday, 31-Dec-26 23:59:59 GMT"
    );
    // Meta Robots im Body wird erfasst und wirkt (TECH-065: vorsichtige Annahme)
    assert!(c.set("Noindex").contains("/body-noindex.html"));
    assert_eq!(c.status("/body-noindex.html"), "noindex");
    // Header-Angaben nicht-HTML (PDF)
    assert_eq!(raw("/noindex.pdf", "X-Robots-Tag 1"), "noindex");
}

// ---------------------------------------------------------------------------------------------------------------
// Filter

// spec: TECH-052
#[tokio::test]
async fn index_filter_wants_the_explicit_token_and_all_does_not_count() {
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("Index"), set(INDEX));
    assert!(!c.set("Index").contains("/all.html"), "all ist nicht index");
    assert!(
        !c.set("Index").contains("/ok.html"),
        "ohne Robots-Angabe nicht enthalten"
    );
    assert_eq!(c.status("/all.html"), "");
}

// spec: TECH-053
#[tokio::test]
async fn noindex_filter_checks_the_token_in_meta_and_header_for_the_crawler() {
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("Noindex"), set(NOINDEX));
    // Meta, X-Robots-Tag, PDF-Header, 404 mit Header, googlebot-Meta: enthalten
    for a in [
        "/noindex-meta.html",
        "/noindex-header.html",
        "/noindex.pdf",
        "/noindex-googlebot.html",
    ] {
        assert!(c.set("Noindex").contains(a), "{a}");
    }
    // `none` allein steht unter None, nicht unter Noindex – die Seite ist trotzdem non-indexable.
    assert!(!c.set("Noindex").contains("/none.html"));
    assert!(c.set("None").contains("/none.html"));
    assert_eq!(c.status("/none.html"), "noindex");
    // bingbot gilt nicht für den Crawler: nicht im Filter, Seite indexierbar.
    assert!(!c.set("Noindex").contains("/noindex-bingbot.html"));
    assert_eq!(
        c.text_cell("All", "/noindex-bingbot.html", "Indexability"),
        "Indexable"
    );
    // Nur der zum Token passende Teil eines X-Robots-Tag mit Agent-Präfix wirkt.
    assert!(c.set("Noindex").contains("/xrobots-agent.html"));
    assert!(!c.set("Nofollow").contains("/xrobots-agent.html"));
    // Status noindex für die Seiten, die auch sonst nicht indexierbar sind (404 → Client Error geht vor)
    assert_eq!(c.status("/noindex-meta.html"), "noindex");
    assert!(!c.set("All").contains("/gone.html"));
    assert_eq!(c.status("/noindex.pdf"), "noindex");
}

// spec: TECH-054
#[tokio::test]
async fn follow_filter_wants_the_explicit_token() {
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("Follow"), set(FOLLOW));
}

// spec: TECH-055
#[tokio::test]
async fn nofollow_filter_and_the_frontier() {
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("Nofollow"), set(NOFOLLOW));

    // Standard: die Links einer Nofollow-Seite kommen nicht in die Frontier, die Outlinks bleiben gespeichert
    // und tragen Follow = false.
    let host = c.fx.site.base_url();
    let host = host
        .trim_start_matches("http://")
        .split(':')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(c.fx.site.count(&host, "/only-via-nofollow-page.html"), 0);
    let follow: Option<i64> = c
        .fx
        .file
        .conn()
        .query_row(
            "SELECT l.follow FROM links l JOIN urls s ON s.id = l.source_id \
             WHERE s.address LIKE '%/nofollow.html' AND l.target_raw LIKE '%only-via-nofollow-page.html'",
            [],
            |r| r.get(0),
        )
        .ok();
    assert_eq!(follow, Some(0), "Outlink gespeichert mit Follow = false");
}

// spec: TECH-055, TECH-051
#[tokio::test]
async fn follow_nofollow_option_changes_crawling_but_not_the_filters() {
    // „Follow Internal Nofollow“ (CRAWL-056): das Ziel wird gecrawlt; die Filter dieses Tabs bleiben gleich,
    // und der Link behält in der Anzeige Follow = false.
    let mut cfg = CrawlConfig::default();
    cfg.spider.follow_internal_nofollow = true;
    let c = crawl(cfg).await;
    assert_eq!(c.set("Nofollow"), set(NOFOLLOW));
    assert_eq!(c.set("Noindex"), set(NOINDEX));
    assert!(c.set("All").contains("/only-via-nofollow-page.html"));
    let follow: i64 = c
        .fx
        .file
        .conn()
        .query_row(
            "SELECT l.follow FROM links l JOIN urls s ON s.id = l.source_id \
             WHERE s.address LIKE '%/nofollow.html' AND l.target_raw LIKE '%only-via-nofollow-page.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(follow, 0);
}

// spec: TECH-056
#[tokio::test]
async fn none_filter_acts_as_noindex_and_nofollow() {
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("None"), set(&["/none.html"]));
    // Wirkung: noindex + nofollow
    assert_eq!(c.status("/none.html"), "noindex");
    assert_eq!(
        c.cell("None", "/none.html", "Effective Index"),
        Value::Bool(false)
    );
    assert_eq!(
        c.cell("None", "/none.html", "Effective Follow"),
        Value::Bool(false)
    );
}

// spec: TECH-066
#[tokio::test]
async fn conflicting_directives_are_restrictive_and_listed_in_both_filters() {
    let c = crawl(CrawlConfig::default()).await;
    // index + noindex in zwei Tags und Meta + Header: noindex wirkt, die Seite steht in beiden Filtern
    for a in ["/conflict-meta.html", "/conflict-mixed.html"] {
        assert!(c.set("Index").contains(a), "{a} in Index");
        assert!(c.set("Noindex").contains(a), "{a} in Noindex");
        assert_eq!(c.status(a), "noindex", "{a}");
        assert_eq!(c.cell("Noindex", a, "Effective Index"), Value::Bool(false));
    }
    assert_eq!(
        c.text_cell("All", "/conflict-meta.html", "Meta Robots 1"),
        "index"
    );
    assert_eq!(
        c.text_cell("All", "/conflict-meta.html", "Meta Robots 2"),
        "noindex"
    );
    assert_eq!(
        c.text_cell("All", "/conflict-mixed.html", "X-Robots-Tag 1"),
        "noindex"
    );
    // follow + nofollow: nofollow wirkt
    assert!(c.set("Follow").contains("/follow-nofollow.html"));
    assert!(c.set("Nofollow").contains("/follow-nofollow.html"));
    assert_eq!(
        c.cell("Nofollow", "/follow-nofollow.html", "Effective Follow"),
        Value::Bool(false)
    );
    // Der kleinste max-snippet-Wert gehört zur Detailansicht (M2); die Rohwerte liefert M1.
}

// spec: TECH-064, TECH-068
#[tokio::test]
async fn refresh_filter_and_the_indexability_it_causes() {
    // „Self Referencing Meta Refresh“ gibt es nur mit der Option (K-57: Standard aus)
    let mut cfg = CrawlConfig::default();
    cfg.advanced.respect_self_ref_meta_refresh = true;
    let c = crawl(cfg).await;
    assert_eq!(c.set("Refresh"), set(REFRESH));

    // Ziel ≠ eigene URL (Meta und Header, Query unterscheidet): Redirected
    for a in [
        "/refresh-0.html",
        "/refresh-header.html",
        "/refresh-query.html",
    ] {
        assert_eq!(c.status(a), "Redirected", "{a}");
    }
    // Selbst-Refresh (eigene URL mit/ohne Fragment, ohne Ziel, jede Verzögerung, Header): Self Referencing Meta Refresh
    for a in [
        "/refresh-5-self.html",
        "/refresh-bare.html",
        "/refresh-self-0.html",
        "/refresh-self-300.html",
        "/refresh-self-fragment.html",
        "/refresh-query.html?x=1",
    ] {
        assert_eq!(c.status(a), "Self Referencing Meta Refresh", "{a}");
    }
    assert_eq!(c.status("/refresh-self-header.html"), "Redirected");
    // Rohwert in der Spalte Meta Refresh
    assert_eq!(
        c.text_cell("Refresh", "/refresh-0.html", "Meta Refresh 1"),
        "0; url=/ok.html"
    );
    assert_eq!(
        c.text_cell("Refresh", "/refresh-bare.html", "Meta Refresh 1"),
        "10"
    );
    // Das Weiterleitungsziel wird gecrawlt (Crawl Meta Refresh, Standard an)
    assert!(c.set("All").contains("/ok.html"));
}

// spec: TECH-064, TECH-068
#[tokio::test]
async fn self_referencing_refresh_stays_in_the_filter_without_the_option() {
    // TECH-068, K-57: Standard (Option aus) → Selbst-Refresh ändert die Indexierbarkeit nicht, die Seite bleibt in
    // „Refresh“.
    let c = crawl(CrawlConfig::default()).await;
    assert_eq!(c.set("Refresh"), set(REFRESH));
    for a in [
        "/refresh-5-self.html",
        "/refresh-bare.html",
        "/refresh-self-0.html",
        "/refresh-self-300.html",
        "/refresh-self-fragment.html",
    ] {
        assert_eq!(
            c.text_cell("Refresh", a, "Indexability"),
            "Indexable",
            "{a}"
        );
        assert_eq!(c.status(a), "", "{a}");
    }
    assert_eq!(c.status("/refresh-0.html"), "Redirected");
    assert_eq!(c.status("/refresh-self-header.html"), "Redirected");
}

// M2-Ergänzungen: tab_directives_m2.rs; schichtübergreifend: pending_w1_d06.rs.

// Golden-Dateien (TECH-007)
// spec: TECH-051
#[tokio::test]
async fn goldens_directives() {
    let c = crawl(CrawlConfig::default()).await;
    golden(&c, FILTERS);
}

// spec: TECH-003 [M2]
// spec: TECH-008 [M2]
// spec: TECH-051 [M2]
#[tokio::test]
async fn outside_head_flags_survive_validation_and_respect_switches() {
    for validation in [true, false] {
        let mut cfg = CrawlConfig::default();
        cfg.advanced.html_validation = validation;
        cfg.advanced.respect_noindex = true;
        let c = Crawled {
            fx: crawl_fixture("tech-m2-directives", cfg.clone()).await,
            cfg,
        };
        // Body-Element und implizit durch img geschlossener Head bleiben extrahiert.
        for (path, expected) in [
            ("/body.html", "noindex, noarchive"),
            ("/after-img.html", "noindex"),
        ] {
            assert_eq!(c.text_cell("Noindex", path, "Meta Robots 1"), expected);
        }
        let outside = c.set("Outside <head>");
        if validation {
            assert!(outside.contains("/body.html"));
            assert!(outside.contains("/after-img.html"));
            assert!(!outside.contains("/noarchive.html"));
        } else {
            assert!(outside.is_empty());
        }
        let mut stmt = c.fx.file.conn().prepare(
            "SELECT r.in_head FROM directives r JOIN urls u ON u.id = r.url_id WHERE r.source = 'meta_robots' AND (u.address LIKE '%/body.html' OR u.address LIKE '%/after-img.html')"
        ).unwrap();
        let flags = stmt
            .query_map([], |r| r.get::<_, bool>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(flags, vec![false, false]);
    }
}
