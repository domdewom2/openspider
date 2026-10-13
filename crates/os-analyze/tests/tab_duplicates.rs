//! Duplikat-Logik der Element-Tabs (ONPAGE-005) und `ignore_non_indexable` (CRAWL-089): Crawls der Fixture
//! `onpage-duplicates` für alle vier Tabs sowie Zeilentests auf einer Crawl-Datei mit direkt eingefügten
//! URL-Zeilen (Gruppierung, Schalter, Wertgleichheit, inkrementelles Verhalten).

use std::path::Path;

use os_analyze::views::PageStart;
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlMode, Timestamp};
use os_store::{CrawlInit, StorageProfile};

const SCENARIO: &str = "onpage-duplicates";

/// Tabs mit Duplicate-Filter: (Tab, Spalte des ersten Werts).
const TABS: [(&str, &str); 4] = [
    ("Page Titles", "Title 1"),
    ("Meta Description", "Meta Description 1"),
    ("H1", "H1-1"),
    ("H2", "H2-1"),
];

// ---------------------------------------------------------------------------------------------------
// Gemeinsame Hilfen der Element-Tab-Tests (in jeder Testdatei des Pakets M1-P22 gleich: ein eigenes
// Hilfsmodul gehört nicht zum Besitz des Pakets).

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, Value};
use os_store::CrawlFile;

/// Bis `M1-P24` (Indexierbarkeit, `os_model::indexability::evaluate`) liefert die Engine nur die
/// Statuscode-Fälle (STUB); `noindex` und `Canonicalised` setzt dieser Helfer nach dem Crawl aus den
/// gespeicherten Rohwerten (`effective_index`, `canonical_self`). Nach dem Merge von P24 ist er wirkungslos
/// (die Zeilen sind dann schon `Non-Indexable`).
fn emulate_indexability(file: &CrawlFile) {
    for (status, condition) in [
        ("noindex", "effective_index = 0"),
        ("Canonicalised", "canonical_self = 0"),
    ] {
        file.conn()
            .execute(
                &format!(
                    "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = ?1 \
                     WHERE indexability = 'Indexable' AND {condition}"
                ),
                [status],
            )
            .unwrap();
    }
}

/// Crawlt das Szenario vollständig (Testsite auf Loopback).
async fn crawl(scenario: &str, cfg: CrawlConfig) -> FixtureCrawl {
    let fx = crawl_fixture(scenario, cfg).await;
    emulate_indexability(&fx.file);
    fx
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

/// Die Seiten ohne Paginierung, die in jedem Tab Duplikate sind: a, b und e (nach N(x) gleich).
const PLAIN_DUPLICATES: [&str; 3] = ["/a.html", "/b.html", "/e.html"];
/// Paginierte Seiten, die unter `ignore_paginated` Duplikate bleiben: erste Seite (nur `next`), Archiv ohne
/// Serie, `prev` im Body (zählt nicht) und `prev` mit leerem Wert (zählt nicht).
const PAGINATION_KEPT: [&str; 4] = [
    "/pagination/archiv.html",
    "/pagination/blog/",
    "/pagination/blog/page/5/",
    "/pagination/blog/page/6/",
];
/// Paginierte Seiten: `prev` als `<link>` im Head (2, 3) bzw. nur als `Link`-Header (4).
const PAGINATION_DROPPED: [&str; 3] = [
    "/pagination/blog/page/2/",
    "/pagination/blog/page/3/",
    "/pagination/blog/page/4/",
];

fn with(parts: &[&[&str]]) -> Vec<String> {
    let all: Vec<&str> = parts.iter().flat_map(|p| p.iter().copied()).collect();
    expect(&all)
}

// spec: ONPAGE-005
#[tokio::test]
async fn two_indexable_pages_with_the_same_value_are_duplicates_in_every_tab() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for (tab, _) in TABS {
        let dup = sorted_addresses(&fx, &cfg, tab, "Duplicate");
        assert_eq!(dup, with(&[&PLAIN_DUPLICATES, &PAGINATION_KEPT]), "{tab}");
        // Zähler und Filterzustand: bereit, Zahl = Zeilen.
        assert_eq!(
            counted(&fx, &cfg, tab, "Duplicate").0 as usize,
            dup.len(),
            "{tab}"
        );
    }
}

// spec: ONPAGE-005
#[tokio::test]
async fn canonicalised_page_is_outside_the_comparison_set_by_default() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let stored = fx.file.config().unwrap();
    let all = view(&fx, &stored, "Page Titles", "All");
    assert_eq!(
        cell(&fx, &all, "/c.html", "Indexability"),
        text("Non-Indexable")
    );
    assert_eq!(
        cell(&fx, &all, "/c.html", "Indexability Status"),
        text("Canonicalised")
    );
    for (tab, _) in TABS {
        // Standard: c (canonicalisiert) steht nicht im Filter.
        let dup = sorted_addresses(&fx, &stored, tab, "Duplicate");
        assert!(!dup.contains(&"/c.html".to_string()), "{tab}");

        // only_indexable = false: c gehört zur Vergleichsmenge – steht wegen CRAWL-089 (ignore_non_indexable)
        // aber weiter nicht im Filter; a, b und e bleiben.
        let mut cfg = stored.clone();
        cfg.duplicates.only_indexable = false;
        assert_eq!(
            sorted_addresses(&fx, &cfg, tab, "Duplicate"),
            with(&[&PLAIN_DUPLICATES, &PAGINATION_KEPT]),
            "{tab}"
        );
        // Beide Schalter aus: c steht im Filter.
        cfg.issues.ignore_non_indexable = false;
        assert_eq!(
            sorted_addresses(&fx, &cfg, tab, "Duplicate"),
            with(&[&["/c.html"], &PLAIN_DUPLICATES, &PAGINATION_KEPT]),
            "{tab}"
        );
    }
}

// spec: ONPAGE-005
#[tokio::test]
async fn comparison_is_case_sensitive_on_normalized_values() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for (tab, first) in TABS {
        let dup = sorted_addresses(&fx, &cfg, tab, "Duplicate");
        // „Startseite“ vs. „startseite“ → kein Duplikat (A-4); mit Randleerzeichen → Duplikat.
        assert!(!dup.contains(&"/d.html".to_string()), "{tab}");
        assert!(dup.contains(&"/e.html".to_string()), "{tab}");
        let page = view(&fx, &cfg, tab, "All");
        assert_eq!(
            cell(&fx, &page, "/e.html", first),
            cell(&fx, &page, "/a.html", first),
            "{tab}"
        );
        assert_ne!(
            cell(&fx, &page, "/d.html", first),
            cell(&fx, &page, "/a.html", first),
            "{tab}"
        );
    }
}

// spec: ONPAGE-005
#[tokio::test]
async fn empty_values_and_second_values_never_form_a_group() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for (tab, _) in TABS {
        let dup = sorted_addresses(&fx, &cfg, tab, "Duplicate");
        // Leere Werte gehören zu „Missing“, nicht zu Duplicate.
        for empty in ["/empty-1.html", "/empty-2.html"] {
            assert!(!dup.contains(&empty.to_string()), "{tab} {empty}");
        }
        assert!(sorted_addresses(&fx, &cfg, tab, "Missing").contains(&"/empty-1.html".to_string()));
        // Nur der erste Wert zählt: second-only trägt „Startseite“ als zweiten Wert.
        assert!(!dup.contains(&"/second-only.html".to_string()), "{tab}");
        assert!(!dup.contains(&"/unique.html".to_string()), "{tab}");
    }
}

// spec: ONPAGE-005
#[tokio::test]
async fn paginated_pages_leave_the_comparison_set_when_ignore_paginated_is_on() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    assert!(cfg.duplicates.ignore_paginated, "Standard: an (A-50)");
    for (tab, _) in TABS {
        let dup = sorted_addresses(&fx, &cfg, tab, "Duplicate");
        // /blog/ (nur next) und /archiv bleiben, /blog/page/2/ und /3/ (prev im Head) fehlen.
        assert!(dup.contains(&"/pagination/blog/".to_string()), "{tab}");
        assert!(
            dup.contains(&"/pagination/archiv.html".to_string()),
            "{tab}"
        );
        for dropped in PAGINATION_DROPPED {
            assert!(!dup.contains(&dropped.to_string()), "{tab} {dropped}");
        }
        // prev im Body und prev ohne Wert zählen nicht als Paginierung: die Seiten bleiben Duplikate.
        assert!(
            dup.contains(&"/pagination/blog/page/5/".to_string()),
            "{tab}"
        );
        assert!(
            dup.contains(&"/pagination/blog/page/6/".to_string()),
            "{tab}"
        );
    }
}

// spec: ONPAGE-005
#[tokio::test]
async fn rel_prev_as_link_header_acts_like_the_html_element() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    // /blog/page/4/ hat rel="prev" nur im Link-Header (HTTP).
    let source: String = fx
        .file
        .conn()
        .query_row(
            "SELECT p.source FROM pagination p JOIN urls u ON u.id = p.url_id WHERE u.address LIKE '%/page/4/' AND p.rel = 'prev'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(source, "http");
    for (tab, _) in TABS {
        assert!(
            !sorted_addresses(&fx, &cfg, tab, "Duplicate")
                .contains(&"/pagination/blog/page/4/".to_string()),
            "{tab}"
        );
    }
    let mut off = cfg.clone();
    off.duplicates.ignore_paginated = false;
    assert!(sorted_addresses(&fx, &off, "Page Titles", "Duplicate")
        .contains(&"/pagination/blog/page/4/".to_string()));
}

// spec: ONPAGE-005
#[tokio::test]
async fn switching_ignore_paginated_off_lists_every_page_of_the_series() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let requests = fx.site.total_requests();
    let mut cfg = fx.file.config().unwrap();
    cfg.duplicates.ignore_paginated = false;
    for (tab, _) in TABS {
        assert_eq!(
            sorted_addresses(&fx, &cfg, tab, "Duplicate"),
            with(&[&PLAIN_DUPLICATES, &PAGINATION_KEPT, &PAGINATION_DROPPED]),
            "{tab}"
        );
    }
    // Umschalten nach dem Crawl: keine HTTP-Anfragen.
    assert_eq!(fx.site.total_requests(), requests);
    // Zurück auf „an“: wie vorher.
    cfg.duplicates.ignore_paginated = true;
    assert_eq!(sorted_addresses(&fx, &cfg, "H1", "Duplicate").len(), 7);
}

// spec: ONPAGE-005
#[tokio::test]
async fn duplicate_goldens() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    let cfg = fx.file.config().unwrap();
    for (tab, _) in TABS {
        assert_view_golden(&fx, &cfg, SCENARIO, tab, "Duplicate");
    }
}

// spec: CRAWL-089
#[tokio::test]
async fn ignore_non_indexable_applies_to_every_problem_filter_but_not_to_all() {
    let fx = crawl(SCENARIO, CrawlConfig::default()).await;
    for (tab, _) in TABS {
        let def = os_analyze::names::find_tab(&os_analyze::tabs::all(), tab)
            .map(|t| t.filters.len())
            .unwrap();
        assert!(def >= 5, "{tab}");
    }
    for t in os_analyze::tabs::all()
        .iter()
        .filter(|t| TABS.iter().any(|(n, _)| *n == t.id))
    {
        for f in &t.filters {
            assert_eq!(f.ignore_non_indexable, f.id != "All", "{}:{}", t.id, f.id);
        }
    }
    // Der Schalter wirkt sofort auf die Filter (Neuberechnung), All bleibt gleich.
    let stored = fx.file.config().unwrap();
    let mut off = stored.clone();
    off.duplicates.only_indexable = false;
    off.issues.ignore_non_indexable = false;
    assert_eq!(
        sorted_addresses(&fx, &off, "Page Titles", "All"),
        sorted_addresses(&fx, &stored, "Page Titles", "All")
    );
}

// ---------------------------------------------------------------------------------------------------
// Zeilentests: URL-Zeilen direkt in die Crawl-Datei (kein Crawl).

fn created_at() -> Timestamp {
    Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap()
}

fn empty_file(dir: &Path) -> CrawlFile {
    let init = CrawlInit {
        crawl_id: SeededIds::new(5).new_crawl_id(created_at()),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "Duplikate".into(),
        config: CrawlConfig::default(),
        created_at: created_at(),
    };
    CrawlFile::create(&dir.join("dup.ospider"), &init, StorageProfile::Balanced).unwrap()
}

/// Eine interne 2xx-HTML-Seite mit gegebenem Titel (und, falls nicht leer, gleichem Wert in allen Elementen).
fn page(file: &CrawlFile, id: i64, title: Option<&str>, indexable: bool, prev: bool) {
    let value = title.map(str::to_string);
    let len = value.as_ref().map(|v| v.chars().count() as i64);
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
             folder_depth, url_length, status_code, content_class, \
             title_count, title_1, title_1_len, meta_desc_count, meta_desc_1, meta_desc_1_len, \
             h1_count, h1_1, h1_1_len, h2_count, h2_1, h2_1_len, \
             indexability, has_rel_prev) \
             VALUES (?1, ?2, ?2, 'https://site.test', 1, 1, 'done', 'Link', 0, 10, 200, 'HTML', \
             ?3, ?4, ?5, ?3, ?4, ?5, ?3, ?4, ?5, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                format!("https://site.test/p{id}"),
                i64::from(value.is_some()),
                value,
                len,
                if indexable { "Indexable" } else { "Non-Indexable" },
                i64::from(prev),
            ],
        )
        .unwrap();
}

fn dup_ids(file: &CrawlFile, cfg: &CrawlConfig, tab: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut start = PageStart::Offset(0);
    loop {
        let mut q = ViewQuery::new(tab, "Duplicate");
        q.limit = 2;
        q.start = start;
        let p = query_view(file, cfg, &q).unwrap();
        ids.extend(p.row_keys.iter().map(|k| k.0));
        match p.next_cursor {
            Some(c) => start = PageStart::Cursor(c),
            None => break,
        }
    }
    ids.sort_unstable();
    ids
}

// spec: ONPAGE-005
#[test]
fn duplicates_appear_as_soon_as_the_second_page_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let file = empty_file(dir.path());
    let cfg = CrawlConfig::default();
    page(&file, 1, Some("Startseite"), true, false);
    for (tab, _) in TABS {
        assert!(
            dup_ids(&file, &cfg, tab).is_empty(),
            "{tab}: eine Seite allein ist kein Duplikat"
        );
    }
    page(&file, 2, Some("Startseite"), true, false);
    page(&file, 3, Some("Anders"), true, false);
    for (tab, _) in TABS {
        assert_eq!(
            dup_ids(&file, &cfg, tab),
            [1, 2],
            "{tab}: beide Seiten erscheinen sofort"
        );
    }
    // Eine dritte Seite mit anderem Wert ändert nichts, mit gleichem Wert kommt sie dazu.
    page(&file, 4, Some("Startseite"), true, false);
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1, 2, 4]);
}

// spec: ONPAGE-005
#[test]
fn only_indexable_and_ignore_non_indexable_combine_as_specified() {
    let dir = tempfile::tempdir().unwrap();
    let file = empty_file(dir.path());
    page(&file, 1, Some("Startseite"), true, false);
    page(&file, 2, Some("Startseite"), false, false); // canonicalisiert / noindex
    let mut cfg = CrawlConfig::default();
    // Standard: indexierbar + nicht indexierbar → keine Seite im Filter.
    assert!(dup_ids(&file, &cfg, "Page Titles").is_empty());
    // only_indexable = false: beide gleich, Seite 2 fällt unter CRAWL-089 heraus → nur Seite 1.
    cfg.duplicates.only_indexable = false;
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1]);
    // Beides aus → beide.
    cfg.issues.ignore_non_indexable = false;
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1, 2]);
    // Nur ignore_non_indexable aus, only_indexable an → wieder keine (Seite 2 nicht in der Vergleichsmenge).
    cfg.duplicates.only_indexable = true;
    assert!(dup_ids(&file, &cfg, "Page Titles").is_empty());
}

// spec: ONPAGE-005
#[test]
fn exact_values_empty_and_null_and_foreign_pages() {
    let dir = tempfile::tempdir().unwrap();
    let file = empty_file(dir.path());
    let cfg = CrawlConfig::default();
    page(&file, 1, Some("Startseite"), true, false);
    page(&file, 2, Some("startseite"), true, false); // andere Schreibweise
    page(&file, 3, Some(""), true, false); // leer
    page(&file, 4, Some(""), true, false);
    page(&file, 5, None, true, false); // fehlt
    page(&file, 6, None, true, false);
    // Zeilen außerhalb der Grundmenge (extern, 404, PDF) bilden keine Gruppe mit Seite 1.
    page(&file, 7, Some("Startseite"), true, false);
    file.conn()
        .execute("UPDATE urls SET is_internal = 0 WHERE id = 7", [])
        .unwrap();
    page(&file, 8, Some("Startseite"), true, false);
    file.conn()
        .execute("UPDATE urls SET status_code = 404 WHERE id = 8", [])
        .unwrap();
    page(&file, 9, Some("Startseite"), true, false);
    file.conn()
        .execute("UPDATE urls SET content_class = 'PDF' WHERE id = 9", [])
        .unwrap();
    for (tab, _) in TABS {
        assert!(dup_ids(&file, &cfg, tab).is_empty(), "{tab}");
    }
    page(&file, 10, Some("Startseite"), true, false);
    assert_eq!(dup_ids(&file, &cfg, "H2"), [1, 10]);
}

// spec: ONPAGE-005
#[test]
fn ignore_paginated_uses_rel_prev_from_head_or_header_only() {
    let dir = tempfile::tempdir().unwrap();
    let file = empty_file(dir.path());
    page(&file, 1, Some("Blog"), true, false);
    page(&file, 2, Some("Blog"), true, true);
    page(&file, 3, Some("Blog"), true, true);
    let pagination = |id: i64, source: &str, raw: &str, in_head: i64| {
        file.conn()
            .execute(
                "INSERT INTO pagination (url_id, rel, source, position, raw, url, target_id, in_head) \
                 VALUES (?1, 'prev', ?2, 0, ?3, ?3, NULL, ?4)",
                rusqlite::params![id, source, raw, in_head],
            )
            .unwrap();
    };
    pagination(2, "html", "/blog/", 1); // im Head, mit Wert → Paginierung
    pagination(3, "html", "/blog/", 0); // im Body → zählt nicht
    let mut cfg = CrawlConfig::default();
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1, 3]);
    page(&file, 4, Some("Blog"), true, true);
    pagination(4, "http", "/blog/page/3/", 0); // Link-Header → Paginierung
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1, 3]);
    cfg.duplicates.ignore_paginated = false;
    assert_eq!(dup_ids(&file, &cfg, "Page Titles"), [1, 2, 3, 4]);
}

// spec: ONPAGE-005
#[test]
fn groups_are_found_through_an_index_on_the_value_column() {
    let dir = tempfile::tempdir().unwrap();
    let file = empty_file(dir.path());
    let cfg = CrawlConfig::default();
    page(&file, 1, Some("Startseite"), true, false);
    // Der Plan der Duplikat-Abfrage nutzt für die Gruppierung den Index auf der Wertspalte.
    for (tab, index) in [
        ("Page Titles", "urls_title1"),
        ("Meta Description", "urls_md1"),
        ("H1", "urls_h1"),
        ("H2", "urls_h2"),
    ] {
        let plan = os_analyze::views::plan(&ViewQuery::new(tab, "Duplicate"), &cfg).unwrap();
        let sql = format!(
            "EXPLAIN QUERY PLAN SELECT u.id FROM urls u WHERE {} AND {}",
            plan.population_sql, plan.filter_sql
        );
        let mut stmt = file.conn().prepare(&sql).unwrap();
        let mut params = Vec::new();
        for (name, value) in &plan.params {
            if stmt.parameter_index(name).unwrap().is_some() {
                params.push((
                    name.clone(),
                    match value {
                        Value::Int(i) => rusqlite::types::Value::Integer(*i),
                        other => panic!("{other:?}"),
                    },
                ));
            }
        }
        let named: Vec<(&str, &dyn rusqlite::ToSql)> = params
            .iter()
            .map(|(n, v)| (n.as_str(), v as &dyn rusqlite::ToSql))
            .collect();
        let details: Vec<String> = stmt
            .query_map(named.as_slice(), |r| r.get::<_, String>(3))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(
            details.iter().any(|d| d.contains(index)),
            "{tab}: {details:?}"
        );
    }
}
