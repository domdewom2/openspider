//! Tab „Meta Description“ gegen echte Crawls der Fixture `onpage-meta` (und `onpage-duplicates` für die
//! Standardsortierung): Spalten, Filter, Schwellen, `ignore_non_indexable` (M1-P22).

use os_model::FilterState;

const TAB: &str = "Meta Description";
const SCENARIO: &str = "onpage-meta";

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
        "/case.html",
        "/og-only.html",
        "/no-content.html",
        "/entities.html",
        "/missing-none.html",
        "/missing-empty.html",
        "/missing-spaces.html",
        "/empty-then-filled.html",
        "/len-155.html",
        "/len-156.html",
        "/len-69.html",
        "/len-70.html",
        "/short.html",
        "/multi.html",
        "/with-og.html",
        "/noindex-missing.html",
        "/noindex-long.html",
    ])
}

// spec: ONPAGE-009
#[tokio::test]
async fn base_set_and_filters_are_consistent() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = sorted_addresses(&fx, &cfg, TAB, "All");
    assert_eq!(all, all_pages());
    // noindex-Seiten bleiben in „All“; jeder Filter ist eine Teilmenge davon, Zähler = Zeilenzahl.
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(
        cell(&fx, &page, "/noindex-long.html", "Indexability"),
        text("Non-Indexable")
    );
    let tab = os_analyze::tabs::meta_description::tab();
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

// spec: ONPAGE-100
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
            "Meta Description 1",
            "Meta Description 1 Length",
            "Meta Description 1 Pixel Width",
            "Meta Description 2",
            "Meta Description 2 Length",
            "Meta Description 2 Pixel Width",
            "Indexability",
            "Indexability Status"
        ]
    );
    let mut q = ViewQuery::new(TAB, "All");
    q.limit = 1;
    let default_columns = query_view(&fx.file, &cfg, &q).unwrap().columns;
    assert!(
        !default_columns.iter().any(|c| c.contains("Pixel")),
        "{default_columns:?}"
    );

    // name="Description" (Name case-insensitiv) wird erkannt.
    assert_eq!(cell(&fx, &page, "/case.html", "Occurrences"), Value::Int(1));
    assert_eq!(
        cell(&fx, &page, "/case.html", "Meta Description 1 Length"),
        Value::Int(100)
    );
    // og:description allein zählt nicht.
    assert_eq!(
        cell(&fx, &page, "/og-only.html", "Occurrences"),
        Value::Int(0)
    );
    assert_eq!(
        cell(&fx, &page, "/og-only.html", "Meta Description 1"),
        Value::Null
    );
    // <meta name="description"> ohne content: Occurrences 1, Wert leer.
    assert_eq!(
        cell(&fx, &page, "/no-content.html", "Occurrences"),
        Value::Int(1)
    );
    assert_eq!(
        cell(&fx, &page, "/no-content.html", "Meta Description 1"),
        text("")
    );
    // Entities im content werden dekodiert (&quot; → ").
    match cell(&fx, &page, "/entities.html", "Meta Description 1") {
        Value::Text(t) => assert!(t.starts_with("Sie sagte \"Hallo\" & ging"), "{t}"),
        other => panic!("{other:?}"),
    }
    // Zwei Elemente: Occurrences 2, beide Spalten gefüllt, Pixelbreite gesetzt.
    assert_eq!(
        cell(&fx, &page, "/multi.html", "Occurrences"),
        Value::Int(2)
    );
    assert_ne!(
        cell(&fx, &page, "/multi.html", "Meta Description 2"),
        Value::Null
    );
    assert!(
        matches!(cell(&fx, &page, "/multi.html", "Meta Description 1 Pixel Width"), Value::Int(px) if px > 0)
    );
}

// spec: ONPAGE-101
#[tokio::test]
async fn missing_lists_pages_without_any_filled_description() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let missing = sorted_addresses(&fx, &cfg, TAB, "Missing");
    assert_eq!(
        missing,
        expect(&[
            "/missing-none.html",
            "/missing-empty.html",
            "/missing-spaces.html",
            "/og-only.html",
            "/no-content.html",
        ])
    );
    // Leeres erstes + gefülltes zweites Vorkommen → nicht Missing.
    assert!(!missing.contains(&"/empty-then-filled.html".to_string()));
    // Mit ignore_non_indexable = false kommt die noindex-Seite ohne Beschreibung dazu (CRAWL-089).
    let mut off = cfg.clone();
    off.issues.ignore_non_indexable = false;
    assert!(
        sorted_addresses(&fx, &off, TAB, "Missing").contains(&"/noindex-missing.html".to_string())
    );
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Missing");
}

// spec: ONPAGE-102
#[tokio::test]
async fn duplicate_compares_the_first_description_and_sorts_by_it() {
    let fx = crawl("onpage-duplicates", CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // Standardsortierung nach Meta Description 1: gleiche Werte stehen zusammen.
    assert_eq!(
        addresses(&fx, &cfg, TAB, "Duplicate"),
        [
            "/pagination/archiv.html",
            "/pagination/blog/",
            "/pagination/blog/page/5/",
            "/pagination/blog/page/6/",
            "/a.html",
            "/b.html",
            "/e.html",
        ]
    );
    // Die Fixture onpage-meta hat nur verschiedene Beschreibungen.
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert!(addresses(&fx, &cfg, TAB, "Duplicate").is_empty());
}

// spec: ONPAGE-103
#[tokio::test]
async fn over_filter_155_is_inclusive_below_the_limit() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // 155 → nein, 156 → ja; noindex-long (200) fällt unter CRAWL-089.
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over 155 Characters"),
        expect(&["/len-156.html"])
    );
    let mut off = cfg.clone();
    off.issues.ignore_non_indexable = false;
    assert_eq!(
        sorted_addresses(&fx, &off, TAB, "Over 155 Characters"),
        expect(&["/len-156.html", "/noindex-long.html"])
    );
}

// spec: ONPAGE-104
#[tokio::test]
async fn below_filter_70_needs_a_description() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // 69 → ja, 70 → nein, fehlend (Länge 0) → nein.
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Below 70 Characters"),
        expect(&["/len-69.html", "/short.html"])
    );
}

// spec: ONPAGE-107
#[tokio::test]
async fn multiple_counts_description_elements_only() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // Zwei Elemente im Head → Multiple; name + og:description → nicht.
    let multiple = sorted_addresses(&fx, &cfg, TAB, "Multiple");
    assert_eq!(
        multiple,
        expect(&["/multi.html", "/empty-then-filled.html"])
    );
    assert!(!multiple.contains(&"/with-og.html".to_string()));
}

// spec: ONPAGE-002
#[tokio::test]
async fn thresholds_change_after_the_crawl_without_new_requests() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let stored = fx.file.config().unwrap();
    let requests = fx.site.total_requests();
    let over_id = "Over {meta_description.max_chars} Characters";
    let below_id = "Below {meta_description.min_chars} Characters";
    assert_eq!(
        counted(&fx, &stored, TAB, over_id),
        (1, "Over 155 Characters".to_string())
    );
    assert_eq!(
        counted(&fx, &stored, TAB, below_id),
        (2, "Below 70 Characters".to_string())
    );

    let mut cfg = stored.clone();
    cfg.meta_description.max_chars = 150;
    cfg.meta_description.min_chars = 100;
    assert_eq!(
        counted(&fx, &cfg, TAB, over_id),
        (2, "Over 150 Characters".to_string())
    );
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over 150 Characters"),
        expect(&["/len-155.html", "/len-156.html"])
    );
    assert_eq!(
        counted(&fx, &cfg, TAB, below_id),
        (4, "Below 100 Characters".to_string())
    );
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Below 100 Characters"),
        expect(&[
            "/len-69.html",
            "/len-70.html",
            "/short.html",
            "/entities.html"
        ])
    );
    assert_eq!(
        fx.site.total_requests(),
        requests,
        "keine neuen HTTP-Anfragen"
    );

    // Ungültig: min > max wird von der Konfigurationsprüfung abgelehnt.
    cfg.meta_description.min_chars = 200;
    assert!(cfg
        .validate()
        .errors
        .iter()
        .any(|e| e.path == "meta_description.min_chars"));
}

// spec: ONPAGE-002
#[tokio::test]
async fn goldens_of_all_filters() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for filter in [
        "All",
        "Duplicate",
        "Over {meta_description.max_chars} Characters",
        "Below {meta_description.min_chars} Characters",
        "Multiple",
    ] {
        assert_view_golden(&fx, &cfg, SCENARIO, TAB, filter);
    }
}
