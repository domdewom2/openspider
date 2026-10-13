//! Export-Pipeline (OUT-001): Kopfzeile immer, Standardreihenfolge der Datenquelle, Streaming, Hinweise,
//! Registry-Durchlauf über alle Tabs und Filter. Tab-Definitionen: eigene Test-Tabs (`links_support.rs`)
//! und die Platzhalter-Tabs von P00b.

#[path = "links_support.rs"]
mod support;

use calamine::{open_workbook, Reader, Xlsx};
use os_analyze::tabs;
use os_analyze::views::{query_view_in, ViewQuery};
use os_export::catalog::{self, ExportKind};
use os_export::tab_export::{canonical_name, export_tab, export_tab_in};
use os_export::{naming, ExportError, Format};
use os_model::{CrawlConfig, Value};
use os_store::CrawlFile;
use support::*;

fn titles_file(dir: &std::path::Path) -> CrawlFile {
    let file = new_file(dir, "titles.ospider");
    // Absichtlich nicht in Adressreihenfolge: die Standardreihenfolge ist die URL-Kennung.
    insert_url(
        &file,
        1,
        "https://site.test/z",
        true,
        &[("title_1", "'Zeta'"), ("status_code", "200")],
    );
    insert_url(
        &file,
        2,
        "https://site.test/a",
        true,
        &[("title_1", "'Alpha'"), ("status_code", "200")],
    );
    insert_url(
        &file,
        3,
        "https://site.test/m",
        true,
        &[("title_1", "'Mu'"), ("status_code", "404")],
    );
    file
}

// spec: OUT-001
#[test]
fn header_is_written_even_when_the_view_has_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    let out = dir.path().join("out");
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "Missing");
    let info = export_tab_in(&tabs, &file, &CrawlConfig::default(), &q, &out, Format::Csv).unwrap();
    assert_eq!(info.rows, 0);
    assert!(info.warnings.is_empty());
    assert_eq!(info.path, out.join("page_titles_missing.csv"));
    let bytes = std::fs::read(&info.path).unwrap();
    assert_eq!(
        bytes,
        b"\xEF\xBB\xBFAddress,Title 1,Status Code,Word Count\r\n".to_vec()
    );

    // XLSX: genau die Kopfzeile
    let info = export_tab_in(
        &tabs,
        &file,
        &CrawlConfig::default(),
        &q,
        &out,
        Format::Xlsx,
    )
    .unwrap();
    let mut wb: Xlsx<_> = open_workbook(&info.path).unwrap();
    let sheet = wb.sheet_names()[0].clone();
    let range = wb.worksheet_range(&sheet).unwrap();
    assert_eq!(range.height(), 1);
    assert_eq!(range.width(), 4);
}

// spec: OUT-001
#[test]
fn default_order_is_ascending_url_id_and_two_exports_are_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    let cfg = CrawlConfig::default();
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "All");

    let a = export_tab_in(&tabs, &file, &cfg, &q, &dir.path().join("a"), Format::Csv).unwrap();
    let b = export_tab_in(&tabs, &file, &cfg, &q, &dir.path().join("b"), Format::Csv).unwrap();
    let (ta, tb) = (
        std::fs::read(&a.path).unwrap(),
        std::fs::read(&b.path).unwrap(),
    );
    assert_eq!(ta, tb, "gleiche Auswahl, byte-identische Dateien");

    let text = read_csv(&a.path);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Address,Title 1,Status Code,Word Count");
    assert_eq!(
        &lines[1..],
        [
            "https://site.test/z,Zeta,200,",
            "https://site.test/a,Alpha,200,",
            "https://site.test/m,Mu,404,"
        ],
        "Crawl-Reihenfolge = aufsteigende URL-Kennung"
    );
    assert_eq!(a.rows, 3);

    // dieselben Zeilen liefert die Ansicht selbst
    let page = query_view_in(&tabs, &file, &cfg, &q).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows[0][0], Value::Text("https://site.test/z".into()));
}

// spec: OUT-001
#[test]
fn rows_arrive_one_by_one_and_all_of_them_are_written() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "viele.ospider");
    // 20 000 URLs per SQL erzeugen
    exec(
        &file,
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 20000) \
         INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length, title_1, status_code) \
         SELECT i, 'https://site.test/p' || i, 'https://site.test/p' || i, 'https://site.test', 1, 'done', 'link', 0, 20, 'Titel ' || i, 200 FROM n",
    );
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "All");
    let info = export_tab_in(
        &tabs,
        &file,
        &CrawlConfig::default(),
        &q,
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info.rows, 20_000);
    let text = read_csv(&info.path);
    assert_eq!(text.lines().count(), 20_001);
    assert!(text
        .lines()
        .nth(20_000)
        .unwrap()
        .starts_with("https://site.test/p20000,Titel 20000,200"));
}

// spec: OUT-001
#[test]
fn filter_waiting_for_the_crawl_analysis_exports_the_header_and_a_hint() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "Needs Analysis");
    let info = export_tab_in(
        &tabs,
        &file,
        &CrawlConfig::default(),
        &q,
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info.rows, 0);
    assert_eq!(info.warnings.len(), 1);
    assert!(
        info.warnings[0].contains("Crawl-Analyse ausführen"),
        "{:?}",
        info.warnings
    );
    assert!(read_csv(&info.path).starts_with("Address,Title 1,Status Code,Word Count\n"));
}

// spec: OUT-001
#[test]
fn write_errors_are_reported_not_swallowed() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    // `out` ist eine Datei: darunter kann kein Ordner entstehen.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "All");
    let err = export_tab_in(
        &tabs,
        &file,
        &CrawlConfig::default(),
        &q,
        &blocker,
        Format::Csv,
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::Io { .. }), "{err:?}");
}

// spec: OUT-001
#[test]
fn unknown_names_give_suggestions() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    let tabs = [page_titles_tab()];
    let q = ViewQuery::new("Page Titles", "Mising");
    let err = export_tab_in(
        &tabs,
        &file,
        &CrawlConfig::default(),
        &q,
        dir.path(),
        Format::Csv,
    )
    .unwrap_err();
    match err {
        ExportError::Name(os_analyze::NameError::Unknown { suggestions, .. }) => {
            assert!(
                suggestions.iter().any(|s| s == "Page Titles:Missing"),
                "{suggestions:?}"
            );
        }
        other => panic!("{other:?}"),
    }
}

// spec: OUT-020
#[test]
fn every_tab_and_filter_of_the_registry_exports_without_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = titles_file(dir.path());
    let cfg = CrawlConfig::default();
    let all = tabs::all();
    let mut expected = 0usize;
    let mut names = Vec::new();
    for tab in &all {
        for f in &tab.filters {
            expected += 1;
            let q = ViewQuery::new(tab.id, f.id);
            let info = export_tab(&file, &cfg, &q, dir.path(), Format::Csv)
                .unwrap_or_else(|e| panic!("{}:{}: {e}", tab.id, f.id));
            assert!(info.path.exists());
            names.push(
                info.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), expected, "jeder Filter hat seine eigene Datei");
}

// spec: OUT-008
#[test]
fn no_two_registered_exports_share_a_file_name() {
    let cfg = CrawlConfig::default();
    let all = tabs::all();
    let mut canonical: Vec<String> = Vec::new();
    for tab in &all {
        for f in &tab.filters {
            canonical.push(canonical_name(&all, &cfg, tab.id, f.id).unwrap());
        }
    }
    let tab_count = canonical.len();
    canonical.extend(catalog::list(ExportKind::Bulk));
    assert!(
        canonical.len() > tab_count + 30,
        "Link-Bulk-Exporte sind registriert"
    );
    let clashes = naming::collisions(canonical.iter().map(String::as_str));
    assert!(clashes.is_empty(), "Kollisionen: {clashes:?}");
}

// spec: OUT-020
#[test]
fn registered_tab_names_resolve_to_themselves() {
    for name in catalog::list(ExportKind::Tab) {
        match catalog::resolve(ExportKind::Tab, &name).unwrap() {
            catalog::ExportRef::Tab { tab, filter } => {
                assert!(
                    name.to_lowercase().starts_with(&tab.to_lowercase()),
                    "{name}"
                );
                assert!(!filter.is_empty());
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}
