//! Tab „Search Console“ (INTEG-115, 116, 123, 125; CRAWL-089): Fixture-Site im temporären Verzeichnis, gecrawlt mit der
//! Engine; die API-Werte stehen so in `api_values`, wie `os-integrations::gsc` sie schreibt (Quelle `gsc`, Ebene `url`,
//! dieselben Metriknamen). Orphan-Seiten, die nur über die API bekannt sind und nach INTEG-015 gecrawlt wurden, legt
//! der Test als URL-Zeilen ohne Inlinks an.

use std::collections::{BTreeMap, BTreeSet};

use os_analyze::{
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{CrawlConfig, Value};

/// Seiten der Fixture `integ-gsc-site`: Pfad und HTML.
const PAGES: &[(&str, &str)] = &[
    (
        "/",
        r#"<!doctype html><title>Start</title><a href="/clicks">c</a><a href="/zero">z</a>
           <a href="/noindex">n</a><a href="/canonicalised">k</a><a href="/nodata">d</a><img src="/img.png">
           <a href="/i/noindex-not-on-google">1</a><a href="/i/not-indexed">2</a><a href="/i/issues">3</a>
           <a href="/i/canonical">4</a><a href="/i/mobile">5</a><a href="/i/amp">6</a><a href="/i/rich">7</a>
           <a href="/i/partial">8</a>"#,
    ),
    ("/clicks", "<!doctype html><title>Clicks</title>"),
    ("/zero", "<!doctype html><title>Zero</title>"),
    (
        "/noindex",
        r#"<!doctype html><title>Noindex</title><meta name="robots" content="noindex">"#,
    ),
    (
        "/canonicalised",
        r#"<!doctype html><title>Canon</title><link rel="canonical" href="/clicks">"#,
    ),
    ("/nodata", "<!doctype html><title>No data</title>"),
    (
        "/i/noindex-not-on-google",
        r#"<!doctype html><title>I1</title><meta name="robots" content="noindex">"#,
    ),
    ("/i/not-indexed", "<!doctype html><title>I2</title>"),
    ("/i/issues", "<!doctype html><title>I3</title>"),
    ("/i/canonical", "<!doctype html><title>I4</title>"),
    ("/i/mobile", "<!doctype html><title>I5</title>"),
    ("/i/amp", "<!doctype html><title>I6</title>"),
    ("/i/rich", "<!doctype html><title>I7</title>"),
    ("/i/partial", "<!doctype html><title>I8</title>"),
];

fn site_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut toml = String::new();
    for (path, body) in PAGES {
        toml.push_str(&format!(
            "[[route]]\npath = \"{path}\"\ncontent_type = \"text/html\"\nbody = '''{body}'''\n\n"
        ));
    }
    toml.push_str("[[route]]\npath = \"/img.png\"\ncontent_type = \"image/png\"\nbody = \"png\"\n");
    std::fs::write(dir.path().join("scenario.toml"), toml).unwrap();
    dir
}

fn config(ignore_non_indexable: bool) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.url_inspection = true;
    cfg.issues.ignore_non_indexable = ignore_non_indexable;
    cfg
}

struct Fixture {
    fx: os_engine::testing::FixtureCrawl,
}

impl Fixture {
    async fn crawl(cfg: &CrawlConfig) -> Fixture {
        let dir = site_dir();
        let fx = os_engine::testing::crawl_dir_with(
            dir.path(),
            "/",
            cfg.clone(),
            os_engine::testing::test_deps(),
        )
        .await;
        let f = Fixture { fx };
        f.add_orphans();
        f.add_values();
        // Das Verzeichnis der Fixture-Site wird nach dem Crawl nicht mehr gebraucht.
        drop(dir);
        f
    }

    fn url(&self, path: &str) -> String {
        self.fx.site.url(path)
    }

    fn id(&self, path: &str) -> i64 {
        self.fx
            .file
            .conn()
            .query_row(
                "SELECT id FROM urls WHERE url = ?1",
                [self.url(path)],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| panic!("{path} nicht gecrawlt"))
    }

    /// Über die Search-Console-API gefundene, nicht verlinkte Seiten (INTEG-015).
    fn add_orphans(&self) {
        for (path, indexability, status) in [
            ("/orphan", "Indexable", None),
            ("/orphan-noindex", "Non-Indexable", Some("noindex")),
        ] {
            self.fx
                .file
                .conn()
                .execute(
                    "INSERT INTO urls (url, address, origin, is_internal, state, discovered_via, crawl_depth, \
                     folder_depth, url_length, status_code, content_class, content_type, indexability, \
                     indexability_status) VALUES (?1, ?1, 'link', 1, 'done', 'link', 1, 1, 30, 200, 'HTML', \
                     'text/html', ?2, ?3)",
                    (self.url(path), indexability, status),
                )
                .unwrap();
        }
    }

    fn put(&self, path: &str, metric: &str, real: Option<f64>, text: Option<&str>) {
        self.fx
            .file
            .conn()
            .execute(
                "INSERT INTO api_values (url_id, source, metric, level, v_real, v_text, fetched_at) \
                 VALUES (?1, 'gsc', ?2, 'url', ?3, ?4, '2026-10-19T12:00:00.000Z')",
                (self.id(path), metric, real, text),
            )
            .unwrap();
    }

    fn analytics(&self, path: &str, clicks: f64, impressions: f64, ctr: f64, position: f64) {
        self.put(path, "clicks", Some(clicks), None);
        self.put(path, "impressions", Some(impressions), None);
        self.put(path, "ctr", Some(ctr), None);
        self.put(path, "position", Some(position), None);
    }

    fn inspection(&self, path: &str, fields: &[(&str, &str)]) {
        for (metric, value) in fields {
            self.put(path, metric, None, Some(value));
        }
        self.put(
            path,
            "inspection_property",
            None,
            Some("sc-domain:127.0.0.1"),
        );
    }

    fn add_values(&self) {
        self.analytics("/", 12.0, 340.0, 0.035_294_117, 2.4);
        self.analytics("/clicks", 5.0, 120.0, 0.041_666_666, 3.456_789_123);
        self.analytics("/zero", 0.0, 120.0, 0.0, 11.2);
        self.analytics("/noindex", 0.0, 30.0, 0.0, 25.0);
        self.analytics("/canonicalised", 1.0, 0.0, 0.0, 0.0);
        self.analytics("/img.png", 2.0, 8.0, 0.25, 4.0);
        self.analytics("/orphan", 3.0, 40.0, 0.075, 8.0);
        self.analytics("/orphan-noindex", 0.0, 4.0, 0.0, 30.0);
        let on = ("summary", "URL is on Google");
        let issues = ("summary", "URL is on Google, but has issues");
        let off = ("summary", "URL is not on Google");
        self.inspection("/i/noindex-not-on-google", &[off]);
        self.inspection(
            "/i/not-indexed",
            &[off, ("coverage", "Crawled - currently not indexed")],
        );
        self.inspection("/i/issues", &[issues, ("rich_results", "Partial")]);
        self.inspection(
            "/i/canonical",
            &[
                on,
                ("user_canonical", "https://integ.test/i/canonical"),
                ("google_canonical", "https://integ.test/other"),
            ],
        );
        self.put("/i/canonical", "canonical_not_selected", Some(1.0), None);
        self.inspection("/i/mobile", &[issues, ("mobile_usability", "Fail")]);
        self.inspection("/i/amp", &[issues, ("amp_results", "Fail")]);
        self.inspection(
            "/i/rich",
            &[
                on,
                ("rich_results", "Pass"),
                ("rich_results_types", "Product snippets"),
                ("rich_results_types_errors", "Product snippets"),
            ],
        );
        // Abgefragt, aber ohne Teilergebnisse (kein Mobile/AMP/Rich Results) und mit gleichen Canonicals.
        self.inspection(
            "/i/partial",
            &[
                on,
                ("user_canonical", "https://integ.test/i/partial"),
                ("google_canonical", "https://integ.test/i/partial"),
            ],
        );
        self.put("/i/partial", "canonical_not_selected", Some(0.0), None);
    }

    /// Pfade der Treffer eines Filters.
    fn hits(&self, cfg: &CrawlConfig, filter: &str) -> BTreeSet<String> {
        let tabs = tabs::configured(cfg);
        let mut query = ViewQuery::new("Search Console", filter);
        query.limit = 500;
        let page = views::query_view_in(&tabs, &self.fx.file, cfg, &query).unwrap();
        let base = self.url("");
        page.rows
            .iter()
            .map(|r| match &r[0] {
                Value::Text(t) => t.strip_prefix(&base).unwrap_or(t).to_string(),
                other => panic!("{other:?}"),
            })
            .collect()
    }
}

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

// spec: INTEG-115
// spec: INTEG-123
#[tokio::test]
async fn columns_follow_the_spec_order_and_number_formats() {
    let cfg = config(true);
    let f = Fixture::crawl(&cfg).await;
    let tabs = tabs::configured(&cfg);
    let mut query = ViewQuery::new("Search Console", "All");
    query.columns = ColumnSelection::All;
    query.limit = 500;
    let page = views::query_view_in(&tabs, &f.fx.file, &cfg, &query).unwrap();
    assert_eq!(
        page.columns,
        [
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
            "Clicks",
            "Impressions",
            "CTR",
            "Position",
            "Summary",
            "Coverage",
            "Last Crawl",
            "Crawled As",
            "Crawl Allowed",
            "Page Fetch",
            "Indexing Allowed",
            "User-Declared Canonical",
            "Google-Selected Canonical",
            "Mobile Usability",
            "Mobile Usability Issues",
            "AMP Results",
            "AMP Issues",
            "Rich Results",
            "Rich Results Types",
            "Rich Results Types Errors",
            "Rich Results Warnings",
            "Inspection Property",
        ]
    );
    let row = |path: &str| {
        let url = f.url(path);
        page.rows
            .iter()
            .find(|r| r[0] == Value::Text(url.clone()))
            .unwrap_or_else(|| panic!("{path}"))
            .clone()
    };
    let clicks = row("/clicks");
    // Clicks/Impressions ganzzahlig, CTR als Anteil 0 – 1 mit vier Stellen (= zwei Stellen in Prozent), Position
    // praktisch ungerundet.
    assert_eq!(clicks[4], Value::Int(5));
    assert_eq!(clicks[5], Value::Int(120));
    assert_eq!(clicks[6], Value::Float(0.0417));
    assert_eq!(clicks[7], Value::Float(3.456789));
    assert_eq!(clicks[1], Value::Int(200));
    assert_eq!(clicks[2], Value::Text("Indexable".into()));
    // Ohne Inspection-Werte bleiben die Inspection-Spalten leer.
    assert!(clicks[8..].iter().all(|v| *v == Value::Null));
    let rich = row("/i/rich");
    assert_eq!(rich[8], Value::Text("URL is on Google".into()));
    assert_eq!(rich[23], Value::Text("Product snippets".into()));
    assert_eq!(rich[25], Value::Text("sc-domain:127.0.0.1".into()));
    // Ohne Search-Analytics-Zeile sind Clicks … Position leer.
    let nodata = row("/nodata");
    assert!(nodata[4..8].iter().all(|v| *v == Value::Null));
    // Grundmenge: interne URLs (14 Seiten, Bild, 2 Orphans).
    assert_eq!(page.total, 17);
}

// spec: INTEG-116
#[tokio::test]
async fn search_analytics_filters_match_the_expected_sets() {
    let cfg = config(true);
    let f = Fixture::crawl(&cfg).await;
    // expected-filters.json (INTEG-116)
    let expected: BTreeMap<&str, BTreeSet<String>> = BTreeMap::from([
        (
            "Clicks Above 0",
            set(&["/", "/clicks", "/canonicalised", "/img.png", "/orphan"]),
        ),
        (
            "No Search Analytics Data",
            set(&[
                "/nodata",
                "/i/noindex-not-on-google",
                "/i/not-indexed",
                "/i/issues",
                "/i/canonical",
                "/i/mobile",
                "/i/amp",
                "/i/rich",
                "/i/partial",
            ]),
        ),
        (
            "Non-Indexable with Search Analytics Data",
            set(&["/noindex", "/orphan-noindex"]),
        ),
        ("Orphan URLs", set(&[])),
    ]);
    for (filter, want) in &expected {
        assert_eq!(&f.hits(&cfg, filter), want, "{filter}");
    }
    // Die kanonisierte Seite mit 0 Impressions zählt nicht als „mit Daten“; die Seite mit 0 Clicks und 120
    // Impressions steht nicht in Clicks Above 0, hat aber Daten.
    assert!(!f.hits(&cfg, "No Search Analytics Data").contains("/zero"));
    let tabs = tabs::configured(&cfg);
    let counts = views::filter_counts_in(&tabs, &f.fx.file, &cfg).unwrap();
    for (filter, want) in &expected {
        let count = counts
            .iter()
            .find(|c| c.tab == "Search Console" && c.filter == *filter)
            .unwrap()
            .count;
        assert_eq!(
            count,
            (*filter != "Orphan URLs").then_some(want.len() as u64),
            "{filter}"
        );
    }
    // Ohne verbundene Search Console sind die Filter nicht anwendbar.
    let mut off = cfg.clone();
    off.api.gsc.enabled = false;
    off.api.gsc.url_inspection = false;
    let counts = views::filter_counts_in(&tabs::configured(&off), &f.fx.file, &off).unwrap();
    for filter in expected.keys() {
        let c = counts
            .iter()
            .find(|c| c.tab == "Search Console" && c.filter == *filter)
            .unwrap();
        assert_eq!(c.count, None, "{filter}");
    }
}

// spec: CRAWL-089
#[tokio::test]
async fn orphan_filter_respects_ignore_non_indexable() {
    let on = config(true);
    let f = Fixture::crawl(&on).await;
    assert_eq!(f.hits(&on, "Orphan URLs"), set(&[]));
    let off = config(false);
    assert_eq!(f.hits(&off, "Orphan URLs"), set(&[]));
    // Die Spaltenwerte bleiben in „All“ sichtbar; Filter für nicht indexierbare Seiten sind nie eingeschränkt.
    assert!(f.hits(&on, "All").contains("/orphan-noindex"));
    assert!(f
        .hits(&on, "Non-Indexable with Search Analytics Data")
        .contains("/orphan-noindex"));
}

// spec: INTEG-125
#[tokio::test]
async fn url_inspection_filters_match_the_expected_sets() {
    let cfg = config(true);
    let f = Fixture::crawl(&cfg).await;
    // expected-filters.json (INTEG-125)
    let expected: BTreeMap<&str, BTreeSet<String>> = BTreeMap::from([
        (
            "URL Is Not on Google",
            set(&["/i/noindex-not-on-google", "/i/not-indexed"]),
        ),
        ("Indexable URL Not Indexed", set(&["/i/not-indexed"])),
        (
            "URL is on Google, But Has Issues",
            set(&["/i/issues", "/i/mobile", "/i/amp"]),
        ),
        (
            "User-Declared Canonical Not Selected",
            set(&["/i/canonical"]),
        ),
        ("Page Is Not Mobile Friendly", set(&["/i/mobile"])),
        ("AMP URL Is Invalid", set(&["/i/amp"])),
        ("Rich Result Invalid", set(&["/i/rich"])),
    ]);
    for (filter, want) in &expected {
        assert_eq!(&f.hits(&cfg, filter), want, "{filter}");
    }
    // Abgefragte URLs ohne Daten zu einem Teilbereich fallen nicht in dessen Filter.
    for filter in [
        "Page Is Not Mobile Friendly",
        "AMP URL Is Invalid",
        "Rich Result Invalid",
        "User-Declared Canonical Not Selected",
    ] {
        assert!(!f.hits(&cfg, filter).contains("/i/partial"), "{filter}");
    }
    // Ohne URL Inspection sind die Inspection-Filter nicht anwendbar, die Search-Analytics-Filter schon.
    let mut no_inspection = cfg.clone();
    no_inspection.api.gsc.url_inspection = false;
    let counts = views::filter_counts_in(
        &tabs::configured(&no_inspection),
        &f.fx.file,
        &no_inspection,
    )
    .unwrap();
    let count = |filter: &str| {
        counts
            .iter()
            .find(|c| c.tab == "Search Console" && c.filter == filter)
            .unwrap()
            .count
    };
    for filter in expected.keys() {
        assert_eq!(count(filter), None, "{filter}");
    }
    assert_eq!(count("Clicks Above 0"), Some(5));
}

// spec: INTEG-116
#[test]
fn tab_defines_all_eleven_filters_of_the_register_without_stubs() {
    let tab = tabs::all()
        .into_iter()
        .find(|t| t.id == "Search Console")
        .unwrap();
    assert!(!tab.is_stub());
    let ids: Vec<&str> = tab.filters.iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        [
            "All",
            "Clicks Above 0",
            "No Search Analytics Data",
            "Non-Indexable with Search Analytics Data",
            "Orphan URLs",
            "URL Is Not on Google",
            "Indexable URL Not Indexed",
            "URL is on Google, But Has Issues",
            "User-Declared Canonical Not Selected",
            "Page Is Not Mobile Friendly",
            "AMP URL Is Invalid",
            "Rich Result Invalid",
        ]
    );
    assert!(tab.filters.iter().all(|f| !f.is_stub()));
    // Nur der Orphan-Filter beachtet issues.ignore_non_indexable (CRAWL-089).
    let ignoring: Vec<&str> = tab
        .filters
        .iter()
        .filter(|f| f.ignore_non_indexable)
        .map(|f| f.id)
        .collect();
    assert_eq!(ignoring, ["Orphan URLs"]);
}
