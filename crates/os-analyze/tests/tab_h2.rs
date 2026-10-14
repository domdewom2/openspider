//! Tab „H2“ gegen echte Crawls der Fixture `onpage-h2` (und `onpage-duplicates` für die Standardsortierung):
//! Extraktion, Filter, Schwellen, `ignore_non_indexable` (M1-P22).

use os_model::FilterState;

const TAB: &str = "H2";
const SCENARIO: &str = "onpage-h2";

// ---------------------------------------------------------------------------------------------------
// Gemeinsame Hilfen der Element-Tab-Tests (in jeder Testdatei des Pakets M1-P22 gleich: ein eigenes
// Hilfsmodul gehört nicht zum Besitz des Pakets).

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};

/// Crawlt das Szenario vollständig (Testsite auf Loopback).
async fn crawl(scenario: &str, cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture(scenario, cfg).await
}

/// Alle Zeilen einer Ansicht mit allen Spalten.
fn view(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter: &str) -> ViewPage {
    let mut q = ViewQuery::new(tab, filter);
    q.columns = ColumnSelection::All;
    q.limit = 500;
    query_view(&fx.file, cfg, &q).unwrap_or_else(|e| panic!("{tab}:{filter}: {e}"))
}

/// Adresse ohne Basis-URL (`/basic.html`; externe Adressen bleiben vollständig).
fn short(fx: &FixtureCrawl, address: &str) -> String {
    address
        .strip_prefix(&fx.site.base_url())
        .unwrap_or(address)
        .to_string()
}

/// Adressen einer Ansicht in Anzeigereihenfolge.
fn addresses(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    view(fx, cfg, tab, filter)
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(a) => short(fx, a),
            other => panic!("Address ist kein Text: {other:?}"),
        })
        .collect()
}

/// Adressen einer Ansicht, alphabetisch (Mengenvergleich).
fn sorted_addresses(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    let mut v = addresses(fx, cfg, tab, filter);
    v.sort();
    v
}

/// Erwartete Adressen, alphabetisch.
fn expect(list: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|s| (*s).to_string()).collect();
    v.sort();
    v
}

/// Wert einer Zelle der Zeile mit der Adresse `path`.
fn cell(fx: &FixtureCrawl, page: &ViewPage, path: &str, column: &str) -> Value {
    let col = page
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column} fehlt in {:?}", page.columns));
    let row = page
        .rows
        .iter()
        .find(|r| matches!(&r[0], Value::Text(a) if short(fx, a) == path))
        .unwrap_or_else(|| panic!("Zeile {path} fehlt"));
    row[col].clone()
}

fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

/// Zähler (Treffer, Anzeigename) eines Filters aus der Übersicht.
fn counted(fx: &FixtureCrawl, cfg: &CrawlConfig, tab: &str, filter_id: &str) -> (u64, String) {
    let counts = filter_counts(&fx.file, cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.tab == tab && c.filter == filter_id)
        .unwrap_or_else(|| panic!("{tab}:{filter_id} fehlt in der Übersicht"));
    (c.count.expect("Filter ist bereit"), c.label.clone())
}

/// CSV-Wiedergabe einer Ansicht mit allen Spalten (Dateikopf, RFC 4180, UTF-8 mit BOM, `{BASE}`).
fn render_csv(fx: &FixtureCrawl, page: &ViewPage) -> String {
    fn field(v: &Value) -> String {
        let s = match v {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Text(t) => t.clone(),
        };
        if s.contains([',', '"', '\r', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s
        }
    }
    let mut out = String::from('\u{feff}');
    out.push_str(&page.columns.join(","));
    out.push('\n');
    for row in &page.rows {
        let cells: Vec<String> = row.iter().map(field).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    os_testsite::golden::normalize(&out, &[&fx.site.base_url()])
}

/// Dateiname-Teil: klein, Buchstaben und Ziffern, sonst `_` (`Over 60 Characters` → `over_60_characters`).
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// Vergleicht die Ansicht mit `tests/golden/<szenario>/<tab>__<filter>.csv` (`UPDATE_GOLDEN=1` schreibt).
fn assert_view_golden(
    fx: &FixtureCrawl,
    cfg: &CrawlConfig,
    scenario: &str,
    tab: &str,
    filter_id: &str,
) {
    let label = os_analyze::names::label(filter_id, cfg);
    let page = view(fx, cfg, tab, filter_id);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(scenario)
        .join(format!("{}__{}.csv", slug(tab), slug(&label)));
    os_testsite::golden::assert_golden(path, &render_csv(fx, &page));
}

/// Alle internen 2xx-HTML-Seiten der Fixture (ONPAGE-009), alphabetisch.
fn all_pages() -> Vec<String> {
    expect(&[
        "/",
        "/basic.html",
        "/nested.html",
        "/br.html",
        "/missing-none.html",
        "/missing-empty.html",
        "/missing-spaces.html",
        "/empty-then-filled.html",
        "/len-70.html",
        "/len-71.html",
        "/multi-two.html",
        "/noindex-missing.html",
        "/noindex-long.html",
        "/twelve.html",
        "/one.html",
    ])
}

// spec: ONPAGE-009
#[tokio::test]
async fn base_set_and_filters_are_consistent() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = sorted_addresses(&fx, &cfg, TAB, "All");
    assert_eq!(all, all_pages());
    let tab = os_analyze::tabs::h2::tab();
    for filter in &tab.filters {
        let rows = sorted_addresses(&fx, &cfg, TAB, filter.id);
        assert!(
            rows.iter().all(|a| all.contains(a)),
            "{TAB}:{} ⊆ All",
            filter.id
        );
        assert_eq!(counted(&fx, &cfg, TAB, filter.id).0 as usize, rows.len());
    }
    for c in filter_counts(&fx.file, &cfg)
        .unwrap()
        .iter()
        .filter(|c| c.tab == TAB)
    {
        assert_eq!(c.state, FilterState::Ready, "{}", c.filter);
    }
}

// spec: ONPAGE-130
#[tokio::test]
async fn columns_and_extraction_values() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(
        page.columns,
        [
            "Address",
            "Occurrences",
            "H2-1",
            "H2-1 Length",
            "H2-2",
            "H2-2 Length",
            "Indexability",
            "Indexability Status"
        ]
    );
    // Zwölf H2: Occurrences 12 (ungedeckelt), H2-1 und H2-2 gefüllt.
    assert_eq!(
        cell(&fx, &page, "/twelve.html", "Occurrences"),
        Value::Int(12)
    );
    assert_ne!(cell(&fx, &page, "/twelve.html", "H2-1"), Value::Null);
    assert_ne!(cell(&fx, &page, "/twelve.html", "H2-2"), Value::Null);
    // Verschachtelter Text wird wie bei H1 zusammengesetzt.
    assert_eq!(cell(&fx, &page, "/nested.html", "H2-1"), text("Hallo Welt"));
    assert_eq!(cell(&fx, &page, "/br.html", "H2-1"), text("A B"));
}

// spec: ONPAGE-131
#[tokio::test]
async fn missing_lists_pages_without_any_filled_h2() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let missing = sorted_addresses(&fx, &cfg, TAB, "Missing");
    // Ohne H2, <h2> </h2>, <h2></h2> → ja; mit Text → nein.
    assert_eq!(
        missing,
        expect(&[
            "/missing-none.html",
            "/missing-empty.html",
            "/missing-spaces.html"
        ])
    );
    let mut off = cfg.clone();
    off.issues.ignore_non_indexable = false;
    assert!(
        sorted_addresses(&fx, &off, TAB, "Missing").contains(&"/noindex-missing.html".to_string())
    );
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Missing");
}

// spec: ONPAGE-132
#[tokio::test]
async fn duplicate_compares_the_first_h2_and_sorts_by_it() {
    let fx = crawl("onpage-duplicates", CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert_eq!(
        addresses(&fx, &cfg, TAB, "Duplicate"),
        [
            "/pagination/archiv.html",
            "/pagination/blog/",
            "/pagination/blog/page/5/",
            "/pagination/blog/page/6/",
            "/a.html",
            "/b.html",
            "/d.html",
            "/e.html",
        ]
    );
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert!(addresses(&fx, &cfg, TAB, "Duplicate").is_empty());
}

// spec: ONPAGE-133
#[tokio::test]
async fn over_filter_70_looks_at_h2_1_only() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over 70 Characters"),
        expect(&["/len-71.html"])
    );
    let requests = fx.site.total_requests();
    let mut cfg = cfg;
    cfg.h2.max_chars = 40;
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over 40 Characters"),
        expect(&["/len-70.html", "/len-71.html"])
    );
    assert_eq!(
        counted(&fx, &cfg, TAB, "Over {h2.max_chars} Characters").1,
        "Over 40 Characters"
    );
    assert_eq!(fx.site.total_requests(), requests);
}

// spec: ONPAGE-134
#[tokio::test]
async fn multiple_lists_pages_with_more_than_one_h2() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let multiple = sorted_addresses(&fx, &cfg, TAB, "Multiple");
    assert_eq!(
        multiple,
        expect(&["/twelve.html", "/multi-two.html", "/empty-then-filled.html"])
    );
    assert!(!multiple.contains(&"/one.html".to_string()));
}

// spec: ONPAGE-002
#[tokio::test]
async fn goldens_of_all_filters() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for filter in [
        "All",
        "Duplicate",
        "Over {h2.max_chars} Characters",
        "Multiple",
    ] {
        assert_view_golden(&fx, &cfg, SCENARIO, TAB, filter);
    }
}
