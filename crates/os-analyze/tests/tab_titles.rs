//! Tab „Page Titles“ gegen echte Crawls der Fixture `onpage-titles` (und `onpage-duplicates` für die
//! Standardsortierung): Grundmenge, Spalten, Filter, Schwellen, `ignore_non_indexable` (M1-P22).

use os_model::FilterState;

const TAB: &str = "Page Titles";
const SCENARIO: &str = "onpage-titles";

// ---------------------------------------------------------------------------------------------------
// Gemeinsame Hilfen der Element-Tab-Tests (in jeder Testdatei des Pakets M1-P22 gleich: ein eigenes
// Hilfsmodul gehört nicht zum Besitz des Pakets).

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};
use os_store::CrawlFile;

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
        "/rcdata.html",
        "/svg-title.html",
        "/entities.html",
        "/missing-none.html",
        "/missing-empty.html",
        "/missing-spaces.html",
        "/missing-nbsp.html",
        "/empty-then-filled.html",
        "/multi-empty.html",
        "/multi-2.html",
        "/multi-3.html",
        "/multi-short-long.html",
        "/multi-body.html",
        "/multi-svg.html",
        "/len-58-padded.html",
        "/len-60.html",
        "/len-61.html",
        "/len-29.html",
        "/len-30.html",
        "/pixel-W70.html",
        "/noindex.html",
        "/noindex-long.html",
        "/quote.html?name=a,b",
    ])
}

// spec: ONPAGE-009
#[tokio::test]
async fn base_set_is_internal_html_with_2xx() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = sorted_addresses(&fx, &cfg, TAB, "All");
    assert_eq!(all, all_pages());

    // Nicht in der Grundmenge: 404 mit Titel, Weiterleitung, PDF (A-5b), externe HTML-Seite, robots.txt.
    for absent in ["/gone", "/moved", "/doc.pdf", "/robots.txt"] {
        assert!(
            !all.contains(&absent.to_string()),
            "{absent} gehört nicht in die Grundmenge"
        );
    }
    let external = format!("http://localhost:{}/basic.html", fx.site.port());
    assert!(
        !all.contains(&external),
        "externe HTML-Seiten fehlen in Element-Tabs"
    );
    // Die Zeilen existieren aber im Crawl (sonst wäre der Test wertlos).
    let rows: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE address IN (?1, ?2, ?3)",
            [
                format!("{}/gone", fx.site.base_url()),
                format!("{}/doc.pdf", fx.site.base_url()),
                external,
            ],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 3, "404-Seite, PDF und externe Seite wurden gecrawlt");
    // noindex-Seiten bleiben in „All“ (Indexability = Non-Indexable).
    let page = view(&fx, &cfg, TAB, "All");
    assert_eq!(
        cell(&fx, &page, "/noindex.html", "Indexability"),
        text("Non-Indexable")
    );
}

// spec: ONPAGE-009
#[tokio::test]
async fn every_filter_is_a_subset_of_all_and_counts_match_rows() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let all = sorted_addresses(&fx, &cfg, TAB, "All");
    let tab = os_analyze::tabs::page_titles::tab();
    for filter in &tab.filters {
        let rows = sorted_addresses(&fx, &cfg, TAB, filter.id);
        for a in &rows {
            assert!(
                all.contains(a),
                "{TAB}:{} ist keine Teilmenge von All ({a})",
                filter.id
            );
        }
        let (count, _) = counted(&fx, &cfg, TAB, filter.id);
        assert_eq!(
            count as usize,
            rows.len(),
            "Zähler = Zeilenzahl für {}",
            filter.id
        );
    }
    let counts = filter_counts(&fx.file, &cfg).unwrap();
    for c in counts.iter().filter(|c| c.tab == TAB) {
        assert_eq!(c.state, FilterState::Ready, "{}", c.filter);
    }
}

// spec: ONPAGE-090
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
            "Title 1",
            "Title 1 Length",
            "Title 1 Pixel Width",
            "Title 2",
            "Title 2 Length",
            "Title 2 Pixel Width",
            "Indexability",
            "Indexability Status"
        ]
    );
    // Standardansicht: ohne die Pixelbreiten (ONPAGE-191, OUT-020).
    let mut q = ViewQuery::new(TAB, "All");
    q.limit = 1;
    let default_page = query_view(&fx.file, &cfg, &q).unwrap();
    assert_eq!(
        default_page.columns,
        [
            "Address",
            "Occurrences",
            "Title 1",
            "Title 1 Length",
            "Title 2",
            "Title 2 Length",
            "Indexability",
            "Indexability Status"
        ]
    );

    // <title>Start</title> → Title 1 „Start“, Länge 5, Occurrences 1.
    assert_eq!(cell(&fx, &page, "/basic.html", "Title 1"), text("Start"));
    assert_eq!(
        cell(&fx, &page, "/basic.html", "Title 1 Length"),
        Value::Int(5)
    );
    assert_eq!(
        cell(&fx, &page, "/basic.html", "Occurrences"),
        Value::Int(1)
    );
    match cell(&fx, &page, "/basic.html", "Title 1 Pixel Width") {
        Value::Int(px) => assert!(px > 0, "Pixelbreite ist gesetzt"),
        other => panic!("Pixelbreite: {other:?}"),
    }
    // RCDATA: Tags im <title> bleiben Text.
    assert_eq!(
        cell(&fx, &page, "/rcdata.html", "Title 1"),
        text("A <b>B</b>")
    );
    // SVG-<title> im Body zählt nicht.
    assert_eq!(
        cell(&fx, &page, "/svg-title.html", "Occurrences"),
        Value::Int(1)
    );
    assert_eq!(cell(&fx, &page, "/svg-title.html", "Title 2"), Value::Null);
    // Entities und Leerraum werden normalisiert: „Shop & Mehr“, 11 Zeichen.
    assert_eq!(
        cell(&fx, &page, "/entities.html", "Title 1"),
        text("Shop & Mehr")
    );
    assert_eq!(
        cell(&fx, &page, "/entities.html", "Title 1 Length"),
        Value::Int(11)
    );
    // Drei <title>: Occurrences auf 2 gedeckelt, Title 1 und 2 gefüllt.
    assert_eq!(
        cell(&fx, &page, "/multi-3.html", "Occurrences"),
        Value::Int(2)
    );
    assert_ne!(cell(&fx, &page, "/multi-3.html", "Title 2"), Value::Null);
    // Seite ohne <title>: Occurrences 0, Werte leer, Länge 0.
    assert_eq!(
        cell(&fx, &page, "/missing-none.html", "Occurrences"),
        Value::Int(0)
    );
    assert_eq!(
        cell(&fx, &page, "/missing-none.html", "Title 1"),
        Value::Null
    );
    assert_eq!(
        cell(&fx, &page, "/missing-none.html", "Title 1 Length"),
        Value::Int(0)
    );
    // Leeres <title> zählt als Vorkommen mit Länge 0.
    assert_eq!(
        cell(&fx, &page, "/missing-empty.html", "Occurrences"),
        Value::Int(1)
    );
}

// spec: ONPAGE-091
#[tokio::test]
async fn missing_lists_pages_without_any_filled_title() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let missing = sorted_addresses(&fx, &cfg, TAB, "Missing");
    assert_eq!(
        missing,
        expect(&[
            "/missing-none.html",
            "/missing-empty.html",
            "/missing-spaces.html",
            "/missing-nbsp.html",
            "/multi-empty.html",
            "/quote.html?name=a,b",
        ])
    );
    // Leer + gefüllt → nicht Missing (aber Multiple); noindex.html fehlt wegen CRAWL-089.
    assert!(!missing.contains(&"/empty-then-filled.html".to_string()));
    assert!(!missing.contains(&"/noindex.html".to_string()));
}

// spec: ONPAGE-091
#[tokio::test]
async fn missing_golden_matches_all_columns_view() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // Auch von OUT-020 (Export „Page Titles:Missing“ mit allen Spalten) benutzt; enthält `quote.html?name=a,b`.
    assert_view_golden(&fx, &cfg, SCENARIO, TAB, "Missing");
    for filter in [
        "All",
        "Duplicate",
        "Over {title.max_chars} Characters",
        "Below {title.min_chars} Characters",
        "Multiple",
    ] {
        assert_view_golden(&fx, &cfg, SCENARIO, TAB, filter);
    }
}

// spec: ONPAGE-092
#[tokio::test]
async fn duplicate_filter_is_sorted_by_title_so_groups_stand_together() {
    let fx = crawl("onpage-duplicates", CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // Keine Sortierung angegeben: Standard des Tabs ist Title 1.
    let rows = addresses(&fx, &cfg, TAB, "Duplicate");
    assert_eq!(
        rows,
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
    let page = view(&fx, &cfg, TAB, "Duplicate");
    let titles: Vec<Value> = rows
        .iter()
        .map(|a| cell(&fx, &page, a, "Title 1"))
        .collect();
    assert_eq!(
        &titles[..4],
        &[text("Blog"), text("Blog"), text("Blog"), text("Blog")]
    );
    assert_eq!(
        &titles[4..],
        &[text("Startseite"), text("Startseite"), text("Startseite")]
    );
}

// spec: ONPAGE-092
#[tokio::test]
async fn duplicate_is_empty_when_all_titles_differ() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert!(addresses(&fx, &cfg, TAB, "Duplicate").is_empty());
    // Der Filter ist bereit (0 Treffer, kein Zustand): „nie 0 an Stelle eines Zustands“.
    assert_eq!(counted(&fx, &cfg, TAB, "Duplicate").0, 0);
}

// spec: ONPAGE-093
#[tokio::test]
async fn over_filter_uses_normalized_length_of_title_1_only() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let over = sorted_addresses(&fx, &cfg, TAB, "Over 60 Characters");
    // 61 Zeichen und 70× „W“; noindex-long (70) fällt unter CRAWL-089.
    assert_eq!(over, expect(&["/len-61.html", "/pixel-W70.html"]));
    // 60 → nein; 58 + Randleerzeichen → nein; Title 2 mit 80 Zeichen bei Title 1 mit 20 → nein.
    for not in [
        "/len-60.html",
        "/len-58-padded.html",
        "/multi-short-long.html",
    ] {
        assert!(!over.contains(&not.to_string()), "{not}");
    }
    // Grenze ist exklusiv, auch über den Namen mit Platzhalter erreichbar.
    assert_eq!(sorted_addresses(&fx, &cfg, TAB, "Over X Characters"), over);
}

// spec: ONPAGE-094
#[tokio::test]
async fn below_filter_needs_a_title_and_is_exclusive_at_the_limit() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let below = sorted_addresses(&fx, &cfg, TAB, "Below 30 Characters");
    assert_eq!(
        below,
        expect(&[
            "/basic.html",
            "/rcdata.html",
            "/entities.html",
            "/len-29.html",
            "/multi-short-long.html",
        ])
    );
    // 30 → nein; fehlende Titel (Länge 0) → nein.
    assert!(!below.contains(&"/len-30.html".to_string()));
    assert!(!below.contains(&"/missing-none.html".to_string()));
    assert!(!below.contains(&"/missing-empty.html".to_string()));
}

// spec: ONPAGE-098
#[tokio::test]
async fn multiple_counts_html_titles_only() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    let multiple = sorted_addresses(&fx, &cfg, TAB, "Multiple");
    assert_eq!(
        multiple,
        expect(&[
            "/multi-2.html",
            "/multi-3.html",
            "/multi-short-long.html",
            "/multi-body.html",
            "/multi-empty.html",
            "/empty-then-filled.html",
        ])
    );
    // Titel + SVG-Titel → nicht; zwei leere Titel → Multiple und Missing.
    assert!(!multiple.contains(&"/multi-svg.html".to_string()));
    assert!(!multiple.contains(&"/svg-title.html".to_string()));
    assert!(sorted_addresses(&fx, &cfg, TAB, "Missing").contains(&"/multi-empty.html".to_string()));
}

// spec: ONPAGE-002
#[tokio::test]
async fn thresholds_change_after_the_crawl_without_new_requests() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let stored = fx.file.config().unwrap();
    let requests = fx.site.total_requests();
    assert_eq!(
        counted(&fx, &stored, TAB, "Over {title.max_chars} Characters"),
        (2, "Over 60 Characters".to_string())
    );

    // title.max_chars 60 → 50: Treffermenge und Anzeigename ändern sich sofort.
    let mut cfg = stored.clone();
    cfg.title.max_chars = 50;
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over 50 Characters"),
        expect(&[
            "/len-58-padded.html",
            "/len-60.html",
            "/len-61.html",
            "/pixel-W70.html"
        ])
    );
    assert_eq!(
        counted(&fx, &cfg, TAB, "Over {title.max_chars} Characters"),
        (4, "Over 50 Characters".to_string())
    );
    // Der Platzhalter nimmt jede Zahl (und „X“): derselbe Filter, die aktuelle Schwelle gilt.
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Over X Characters"),
        sorted_addresses(&fx, &cfg, TAB, "Over 50 Characters")
    );

    // title.min_chars 30 → 40.
    cfg.title.min_chars = 40;
    let (n, label) = counted(&fx, &cfg, TAB, "Below {title.min_chars} Characters");
    assert_eq!(label, "Below 40 Characters");
    assert_eq!(n, 7);
    assert_eq!(
        sorted_addresses(&fx, &cfg, TAB, "Below 40 Characters"),
        expect(&[
            "/",
            "/basic.html",
            "/rcdata.html",
            "/entities.html",
            "/len-29.html",
            "/len-30.html",
            "/multi-short-long.html",
        ])
    );
    assert_eq!(
        fx.site.total_requests(),
        requests,
        "keine neuen HTTP-Anfragen"
    );
}

// spec: ONPAGE-002
#[tokio::test]
async fn thresholds_of_the_crawl_file_apply_when_reopened() {
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 50;
    cfg.title.min_chars = 10;
    let fx = crawl(SCENARIO, cfg).await;
    let reopened = CrawlFile::open(fx.file.path(), os_store::OpenMode::ReadOnly).unwrap();
    let stored = reopened.config().unwrap();
    assert_eq!((stored.title.max_chars, stored.title.min_chars), (50, 10));
    // Mit der gespeicherten Konfiguration gelten deren Werte (Label und Treffer).
    let counts = filter_counts(&reopened, &stored).unwrap();
    let over = counts
        .iter()
        .find(|c| c.tab == TAB && c.filter.starts_with("Over"))
        .unwrap();
    assert_eq!(
        (over.label.as_str(), over.count),
        ("Over 50 Characters", Some(4))
    );
    let below = counts
        .iter()
        .find(|c| c.tab == TAB && c.filter.starts_with("Below"))
        .unwrap();
    assert_eq!(
        (below.label.as_str(), below.count),
        ("Below 10 Characters", Some(1))
    );
}

// spec: ONPAGE-002
#[test]
fn defaults_and_validation_of_the_thresholds() {
    // Ohne Konfiguration gelten exakt die Defaults aus 0.3.
    let cfg = CrawlConfig::default();
    assert_eq!((cfg.title.max_chars, cfg.title.min_chars), (60, 30));
    assert_eq!(
        (
            cfg.meta_description.max_chars,
            cfg.meta_description.min_chars
        ),
        (155, 70)
    );
    assert_eq!(
        (cfg.h1.max_chars, cfg.h2.max_chars, cfg.url.max_chars),
        (70, 70, 115)
    );
    assert_eq!(
        os_analyze::names::label("Over {title.max_chars} Characters", &cfg),
        "Over 60 Characters"
    );

    // Ungültige Werte (negativ, min > max) lehnt die Konfigurationsprüfung (P13) ab.
    let mut bad = cfg.clone();
    assert!(bad.set_value("title.max_chars", "-1").is_err());
    assert_eq!(
        bad.title.max_chars, 60,
        "ein abgelehnter Wert ändert nichts"
    );
    bad.title.min_chars = 80;
    let report = bad.validate();
    assert!(!report.is_ok());
    assert!(report.errors.iter().any(|e| e.path == "title.min_chars"));
}

// spec: CRAWL-089
#[tokio::test]
async fn non_indexable_pages_leave_the_problem_filters_but_not_all() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let stored = fx.file.config().unwrap();
    assert!(stored.issues.ignore_non_indexable, "Standard: an");
    let requests = fx.site.total_requests();

    // An: noindex-Seite ohne Titel fehlt in „Missing“, steht aber in „All“ mit allen Spaltenwerten.
    assert!(!addresses(&fx, &stored, TAB, "Missing").contains(&"/noindex.html".to_string()));
    let all = view(&fx, &stored, TAB, "All");
    assert_eq!(
        cell(&fx, &all, "/noindex.html", "Indexability"),
        text("Non-Indexable")
    );
    assert_eq!(
        cell(&fx, &all, "/noindex.html", "Indexability Status"),
        text("noindex")
    );
    assert_eq!(
        cell(&fx, &all, "/noindex-long.html", "Title 1 Length"),
        Value::Int(70)
    );
    assert!(!addresses(&fx, &stored, TAB, "Over 60 Characters")
        .contains(&"/noindex-long.html".to_string()));

    // Aus: dieselbe Datei, nur die Einstellung ändert sich – sofort, ohne neue Anfragen.
    let mut off = stored.clone();
    off.issues.ignore_non_indexable = false;
    let missing = sorted_addresses(&fx, &off, TAB, "Missing");
    assert!(missing.contains(&"/noindex.html".to_string()));
    assert_eq!(missing.len(), 7);
    assert!(sorted_addresses(&fx, &off, TAB, "Over 60 Characters")
        .contains(&"/noindex-long.html".to_string()));
    assert_eq!(
        sorted_addresses(&fx, &off, TAB, "All"),
        sorted_addresses(&fx, &stored, TAB, "All"),
        "All bleibt unverändert"
    );
    // Wieder an: wie vorher.
    assert_eq!(counted(&fx, &stored, TAB, "Missing").0, 6);
    assert_eq!(fx.site.total_requests(), requests);
}

// spec: ONPAGE-090
#[tokio::test]
async fn tab_is_empty_when_titles_are_not_stored() {
    // Speicher-Schalter „Page Titles“ aus: nichts gespeichert, der Tab bleibt leer (statt jede Seite als
    // „Missing“ zu melden); die anderen Element-Tabs sind nicht betroffen.
    let mut cfg = CrawlConfig::default();
    cfg.extraction.page_titles = false;
    let fx = crawl(SCENARIO, cfg).await;
    let stored = fx.file.config().unwrap();
    assert!(addresses(&fx, &stored, TAB, "All").is_empty());
    assert_eq!(counted(&fx, &stored, TAB, "Missing").0, 0);
    assert_eq!(
        counted(&fx, &stored, "H1", "All").0 as usize,
        all_pages().len()
    );
    // Wieder eingeschaltet (nur Ansicht): die Grundmenge ist da, die Werte fehlen.
    let mut on = stored.clone();
    on.extraction.page_titles = true;
    assert_eq!(sorted_addresses(&fx, &on, TAB, "All"), all_pages());
}

// spec: ONPAGE-090 [M2]
#[test]
#[ignore = "M2: ONPAGE-090 Titel aus dem gerenderten DOM (js-title.html) braucht JavaScript-Rendering"]
fn rendering_mode_takes_the_title_from_the_rendered_dom() {}
