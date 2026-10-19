//! Tab „External“ (M1-P20, ONPAGE-025, ONPAGE-026): echte Crawls der Fixture `onpage-external`. Der Host
//! `localhost` ist für den Crawler extern (F-HOST2), `blog.site.test` eine fremde Subdomain.

use std::path::PathBuf;

use os_analyze::tabs;
use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_analyze::SortKey;
use os_engine::testing::{crawl_fixture, crawl_fixture_from, FixtureCrawl};
use os_model::{CrawlConfig, StoreCrawl, Value};
use os_testsite::golden;

// ---------------------------------------------------------------------------------------------------
// Hilfen

/// Ergebnis einer Ansicht.
struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

fn query(
    fx: &FixtureCrawl,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
    cols: ColumnSelection,
) -> Table {
    let mut q = ViewQuery::new(tab, filter);
    q.columns = cols;
    q.limit = 500;
    let page = views::query_view(&fx.file, cfg, &q).expect("Ansicht");
    Table {
        columns: page.columns,
        rows: page.rows,
    }
}

impl Table {
    fn col(&self, name: &str) -> usize {
        self.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("Spalte {name:?} fehlt in {:?}", self.columns))
    }

    fn addresses(&self) -> Vec<String> {
        let a = self.col("Address");
        self.rows
            .iter()
            .map(|r| match &r[a] {
                Value::Text(s) => s.clone(),
                other => panic!("Address ist kein Text: {other:?}"),
            })
            .collect()
    }

    /// Wert der Zelle in der Zeile mit genau dieser Adresse.
    fn get(&self, address: &str, col: &str) -> Value {
        let a = self.col("Address");
        let c = self.col(col);
        self.rows
            .iter()
            .find(|r| r[a] == Value::Text(address.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "keine Zeile für {address}; vorhanden: {:?}",
                    self.addresses()
                )
            })[c]
            .clone()
    }
}

/// Externer Host der Fixture: `http://localhost:<Port der Testsite>`.
fn ext(fx: &FixtureCrawl) -> String {
    format!("http://localhost:{}", fx.site.port())
}

fn golden_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/onpage-external")
        .join(file)
}

/// Adressen eines Filters, sortiert nach Address, mit `{BASE}` (interner Host) und `{EXT}` (localhost).
fn normalized_addresses(fx: &FixtureCrawl, tab: &str, filter: &str) -> String {
    let mut q = ViewQuery::new(tab, filter);
    q.columns = ColumnSelection::Named(vec!["Address".to_string()]);
    q.sort = vec![SortKey {
        column: "Address".to_string(),
        descending: false,
    }];
    q.limit = 500;
    let page = views::query_view(&fx.file, &CrawlConfig::default(), &q).expect("Ansicht");
    let mut text = String::from("Address\n");
    for row in &page.rows {
        if let Value::Text(s) = &row[0] {
            text.push_str(s);
            text.push('\n');
        }
    }
    let text = text.replace(&ext(fx), "{EXT}");
    let bases = fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    golden::normalize(&text, &bases)
}

fn assert_golden(fx: &FixtureCrawl, tab: &str, filter: &str, file: &str) {
    golden::assert_golden(golden_path(file), &normalized_addresses(fx, tab, filter));
}

async fn external_site() -> FixtureCrawl {
    crawl_fixture("onpage-external", CrawlConfig::default()).await
}

fn external_addresses(fx: &FixtureCrawl, filter: &str) -> Vec<String> {
    let t = query(
        fx,
        &CrawlConfig::default(),
        "External",
        filter,
        ColumnSelection::Named(vec!["Address".into()]),
    );
    let ext = ext(fx);
    t.addresses()
        .into_iter()
        .map(|a| a.strip_prefix(&ext).unwrap_or(&a).to_string())
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-025 Definition und Spalten

// spec: ONPAGE-025
#[test]
fn default_layout_has_exactly_the_six_parity_columns_and_hides_the_extensions() {
    let tab = tabs::external::tab();
    let visible: Vec<&str> = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .map(|c| c.key)
        .collect();
    assert_eq!(
        visible,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Crawl Depth",
            "Inlinks"
        ]
    );
    let hidden: Vec<(&str, bool)> = tab
        .columns
        .iter()
        .filter(|c| !c.default_visible)
        .map(|c| (c.key, c.extension))
        .collect();
    assert_eq!(
        hidden,
        [
            ("Response Time", true),
            ("Redirect URL", true),
            ("Redirect Type", true)
        ]
    );
}

// spec: ONPAGE-025
#[tokio::test]
async fn default_view_shows_six_columns_and_export_of_all_columns_appends_the_extensions() {
    let fx = external_site().await;
    let cfg = CrawlConfig::default();
    let default = query(&fx, &cfg, "External", "All", ColumnSelection::Default);
    assert_eq!(
        default.columns,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Crawl Depth",
            "Inlinks"
        ]
    );
    let all = query(&fx, &cfg, "External", "All", ColumnSelection::All);
    assert_eq!(
        all.columns,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Crawl Depth",
            "Inlinks",
            "Response Time",
            "Redirect URL",
            "Redirect Type"
        ]
    );
    // Ausblenden von Spalten ändert die Zeilenmenge nicht
    assert_eq!(default.addresses(), all.addresses());
}

// spec: ONPAGE-025
#[tokio::test]
async fn a_link_to_localhost_is_external_and_not_internal() {
    let fx = external_site().await;
    let ext = ext(&fx);
    let external = external_addresses(&fx, "All");
    assert!(external.contains(&"/ext.html".to_string()), "{external:?}");
    let internal = query(
        &fx,
        &CrawlConfig::default(),
        "Internal",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert!(
        !internal.iter().any(|a| a.starts_with(&ext)),
        "{internal:?}"
    );
    // interne Seiten erscheinen nicht in External
    assert!(!external
        .iter()
        .any(|a| a == &format!("{}/page2.html", fx.site.base_url())));
    assert!(internal.contains(&format!("{}/page2.html", fx.site.base_url())));
    assert_golden(&fx, "External", "All", "external__all.csv");
}

// spec: ONPAGE-025
#[tokio::test]
async fn an_external_page_is_requested_once_and_its_links_are_not_followed() {
    let fx = external_site().await;
    assert_eq!(fx.site.count("localhost", "/ext.html"), 1);
    assert_eq!(fx.site.count("localhost", "/never-seen.html"), 0);
    assert_eq!(fx.site.count("127.0.0.1", "/never-seen.html"), 0);
    let external = external_addresses(&fx, "All");
    assert!(!external.contains(&"/never-seen.html".to_string()));
    let all = query(
        &fx,
        &CrawlConfig::default(),
        "Internal",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert!(!all.iter().any(|a| a.ends_with("/never-seen.html")));
}

// spec: ONPAGE-025
#[tokio::test]
async fn crawl_depth_is_the_depth_of_the_linking_page_plus_one() {
    let fx = external_site().await;
    let t = query(
        &fx,
        &CrawlConfig::default(),
        "External",
        "All",
        ColumnSelection::All,
    );
    let ext = ext(&fx);
    // index (Tiefe 0) → ext.html
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Crawl Depth"),
        Value::Int(1)
    );
    // index → deep.html (Tiefe 1) → ext-deep.html
    assert_eq!(
        t.get(&format!("{ext}/ext-deep.html"), "Crawl Depth"),
        Value::Int(2)
    );
}

// spec: ONPAGE-025
#[tokio::test]
async fn inlinks_count_the_link_instances_from_internal_pages() {
    let fx = external_site().await;
    let t = query(
        &fx,
        &CrawlConfig::default(),
        "External",
        "All",
        ColumnSelection::All,
    );
    let ext = ext(&fx);
    // zwei Seiten mit je zwei Links → 4
    assert_eq!(t.get(&format!("{ext}/ext.html"), "Inlinks"), Value::Int(4));
    // eine Ressourcen-Referenz (<img src>) ist keine Hyperlink-Instanz (K-61)
    assert_eq!(t.get(&format!("{ext}/ext.png"), "Inlinks"), Value::Int(0));
    assert_eq!(
        t.get(&format!("{ext}/ext-deep.html"), "Inlinks"),
        Value::Int(1)
    );
}

// spec: ONPAGE-025
#[tokio::test]
async fn status_and_response_data_are_those_of_the_single_check() {
    let fx = external_site().await;
    let t = query(
        &fx,
        &CrawlConfig::default(),
        "External",
        "All",
        ColumnSelection::All,
    );
    let ext = ext(&fx);
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Status Code"),
        Value::Int(200)
    );
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Status"),
        Value::Text("OK".into())
    );
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Content"),
        Value::Text("text/html; charset=utf-8".into())
    );
    assert_eq!(
        t.get(&format!("{ext}/ext-404"), "Status Code"),
        Value::Int(404)
    );
    assert_eq!(
        t.get(&format!("{ext}/ext-404"), "Status"),
        Value::Text("Not Found".into())
    );
    // Erweiterungsspalten sind gefüllt, wo es Werte gibt
    assert!(matches!(
        t.get(&format!("{ext}/ext.html"), "Response Time"),
        Value::Float(_)
    ));
}

// spec: ONPAGE-025
#[tokio::test]
async fn an_external_subdomain_is_external_while_all_subdomains_is_off() {
    let fx = crawl_fixture_from(
        "onpage-external",
        "http://www.site.test/sub.html",
        CrawlConfig::default(),
    )
    .await;
    let cfg = CrawlConfig::default();
    let external = query(
        &fx,
        &cfg,
        "External",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert_eq!(external, ["http://blog.site.test/blog.html"]);
    let internal = query(
        &fx,
        &cfg,
        "Internal",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert!(
        !internal.iter().any(|a| a.contains("blog.site.test")),
        "{internal:?}"
    );

    // „alle Subdomains crawlen“: die Subdomain ist intern und fehlt in External
    let mut cfg = CrawlConfig::default();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from(
        "onpage-external",
        "http://www.site.test/sub.html",
        cfg.clone(),
    )
    .await;
    let external = query(
        &fx,
        &cfg,
        "External",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert!(external.is_empty(), "{external:?}");
    let internal = query(
        &fx,
        &cfg,
        "Internal",
        "All",
        ColumnSelection::Named(vec!["Address".into()]),
    )
    .addresses();
    assert!(internal.contains(&"http://blog.site.test/blog.html".to_string()));
}

// spec: ONPAGE-025
#[tokio::test]
async fn with_external_checks_off_the_urls_are_listed_but_not_requested() {
    let mut cfg = CrawlConfig::default();
    cfg.spider.external = StoreCrawl::STORE_ONLY;
    let fx = crawl_fixture("onpage-external", cfg.clone()).await;
    let t = query(&fx, &cfg, "External", "All", ColumnSelection::All);
    let ext = ext(&fx);
    assert!(t.addresses().contains(&format!("{ext}/ext.html")));
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Status Code"),
        Value::Null
    );
    // A-11: ohne Abruf steht „Not Checked“ als Status
    assert_eq!(
        t.get(&format!("{ext}/ext.html"), "Status"),
        Value::Text("Not Checked".into())
    );
    assert_eq!(fx.site.count("localhost", "/ext.html"), 0, "kein Abruf");
    // Inlinks bleiben gezählt
    assert_eq!(t.get(&format!("{ext}/ext.html"), "Inlinks"), Value::Int(4));
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-026 Inhaltstyp-Filter

// spec: ONPAGE-026
#[tokio::test]
async fn class_filters_assign_external_urls_by_content_type() {
    let fx = external_site().await;
    let html = external_addresses(&fx, "HTML");
    assert_eq!(html.len(), 3, "{html:?}");
    for want in ["/ext.html", "/ext-404", "/ext-deep.html"] {
        assert!(html.contains(&want.to_string()), "{want}");
    }
    assert_eq!(external_addresses(&fx, "JavaScript"), ["/ext.js"]);
    assert_eq!(external_addresses(&fx, "CSS"), ["/ext.css"]);
    assert_eq!(external_addresses(&fx, "PDF"), ["/ext.pdf"]);
    assert_eq!(external_addresses(&fx, "Flash"), ["/ext.swf"]);
    // Parität (A-49): Medien, Schriften und XML stehen unter Other
    let mut other = external_addresses(&fx, "Other");
    other.sort();
    assert_eq!(other, ["/ext.json", "/ext.mp4", "/ext.xml"]);
    for (filter, file) in [
        ("HTML", "external__html.csv"),
        ("JavaScript", "external__javascript.csv"),
        ("CSS", "external__css.csv"),
        ("Images", "external__images.csv"),
        ("PDF", "external__pdf.csv"),
        ("Flash", "external__flash.csv"),
        ("Other", "external__other.csv"),
        ("Unknown", "external__unknown.csv"),
    ] {
        assert_golden(&fx, "External", filter, file);
    }
}

// spec: ONPAGE-026
#[tokio::test]
async fn an_external_image_from_img_src_is_in_the_images_filter() {
    let fx = external_site().await;
    assert_eq!(external_addresses(&fx, "Images"), ["/ext.png"]);
}

// spec: ONPAGE-026, CRAWL-101
#[tokio::test]
async fn unanswered_external_urls_are_unknown_and_robots_blocks_are_hidden_by_default() {
    let fx = external_site().await;
    let unknown = external_addresses(&fx, "Unknown");
    // CRAWL-101: durch robots.txt blockierte externe URLs sind standardmäßig ausgeblendet.
    assert!(
        !unknown.contains(&"/ext-blocked/x.html".to_string()),
        "{unknown:?}"
    );
    // ohne Content-Type
    assert!(unknown.contains(&"/ext-none".to_string()), "{unknown:?}");
    let t = query(
        &fx,
        &CrawlConfig::default(),
        "External",
        "Unknown",
        ColumnSelection::All,
    );
    assert!(!t
        .addresses()
        .contains(&format!("{}/ext-blocked/x.html", ext(&fx))));
    assert_eq!(
        fx.site.count("localhost", "/ext-blocked/x.html"),
        0,
        "gesperrt: nie angefragt"
    );
}

// spec: ONPAGE-026, CRAWL-100, CRAWL-101
#[tokio::test]
async fn an_external_url_with_unreachable_robots_is_hidden_by_default() {
    let fx = crawl_fixture_from("onpage-external", "/refused.html", CrawlConfig::default()).await;
    let unknown = external_addresses(&fx, "Unknown");
    // Die unerreichbare robots.txt sperrt den Host; externe Sperren sind ausgeblendet.
    assert!(unknown.is_empty());
    let t = query(
        &fx,
        &CrawlConfig::default(),
        "External",
        "Unknown",
        ColumnSelection::All,
    );
    assert!(t.rows.is_empty());
}

// spec: ONPAGE-026
#[tokio::test]
async fn the_class_filters_partition_external_all() {
    let fx = external_site().await;
    let all = external_addresses(&fx, "All");
    let mut union: Vec<String> = Vec::new();
    for filter in [
        "HTML",
        "JavaScript",
        "CSS",
        "Images",
        "PDF",
        "Flash",
        "Other",
        "Unknown",
    ] {
        for a in external_addresses(&fx, filter) {
            assert!(!union.contains(&a), "{a} in mehr als einem Klassenfilter");
            union.push(a);
        }
    }
    union.sort();
    let mut expected = all.clone();
    expected.sort();
    assert_eq!(union, expected, "Summe aller Klassenfilter = External All");
}

// spec: ONPAGE-025
#[tokio::test]
async fn inlinks_count_only_hyperlinks_not_redirects_or_resource_references() {
    // `/inlink-types.html`: Hyperlink auf `ext.test/inlinks-both.html`, dazu ein interner 301 auf dieselbe URL;
    // ein Bild nur per `<img src>`; eine externe URL nur per `<area href>`.
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-external", "/inlink-types.html", cfg.clone()).await;
    let t = query(&fx, &cfg, "External", "All", ColumnSelection::All);
    // Hyperlink + 301-Ziel → 1 (die Weiterleitung zählt nicht)
    assert_eq!(
        t.get("http://ext.test/inlinks-both.html", "Inlinks"),
        Value::Int(1)
    );
    // nur Bild-Referenz → 0
    assert_eq!(
        t.get("http://ext.test/inlinks-img-only.png", "Inlinks"),
        Value::Int(0)
    );
    // `<area href>` ist ein Hyperlink
    assert_eq!(
        t.get("http://ext.test/inlinks-area.html", "Inlinks"),
        Value::Int(1)
    );
}
