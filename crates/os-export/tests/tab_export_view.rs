//! Tab-Export (OUT-020, ONPAGE-191): Spalten (sichtbar/alle, Erweiterungsspalten), Sortierung und Suche aus
//! der Abfrage, Zahlen- und Zellformate, Dateiname und Datumsvorschlag, Warnungen. Eigene Test-Tabs.

#[path = "links_support.rs"]
mod support;

use calamine::{open_workbook, Data, Reader, Xlsx};
use os_analyze::views::{filter_counts_in, ColumnSelection, SearchSpec, ViewQuery};
use os_export::tab_export::{
    canonical_name, export_tab_in, suggest_file_name, suggest_tab_file_name_in,
};
use os_export::Format;
use os_model::{CrawlConfig, Timestamp};
use os_store::CrawlFile;
use support::*;
use time::UtcOffset;

#[allow(clippy::type_complexity)]
fn page_file(dir: &std::path::Path) -> CrawlFile {
    let file = new_file(dir, "pages.ospider");
    let rows: [(i64, &str, &str, i64, i64, i64, i64, f64, &str); 5] = [
        (
            1,
            "https://site.test/",
            "Start",
            200,
            120,
            300,
            512,
            12.5,
            "abc123",
        ),
        (
            2,
            "https://site.test/blog/a",
            "Blog A",
            200,
            900,
            410,
            1234,
            30.25,
            "def456",
        ),
        (
            3,
            "https://site.test/blog/b",
            "Blog B",
            200,
            450,
            380,
            80,
            8.0,
            "0123",
        ),
        (
            4,
            "https://site.test/missing",
            "Fehlt",
            404,
            10,
            100,
            40,
            1.0,
            "ff00",
        ),
        (
            5,
            "https://site.test/other",
            "Sonstiges",
            200,
            450,
            200,
            99,
            2.0,
            "aa11",
        ),
    ];
    for (id, addr, title, status, words, px, ms, ratio, hash) in rows {
        insert_url(
            &file,
            id,
            addr,
            true,
            &[
                ("title_1", &lit(title)),
                ("title_1_len", &title.len().to_string()),
                ("status_code", &status.to_string()),
                ("word_count", &words.to_string()),
                ("title_1_px", &px.to_string()),
                ("response_time_ms", &ms.to_string()),
                ("text_ratio", &ratio.to_string()),
                ("hash_md5", &lit(hash)),
                ("crawl_depth", "1"),
            ],
        );
    }
    insert_url(
        &file,
        6,
        "https://other.test/",
        false,
        &[("status_code", "200")],
    );
    exec(
        &file,
        "INSERT INTO canonicals (url_id, source, position, raw, in_head) VALUES \
         (2, 'html', 0, 'https://site.test/blog/a', 1), (2, 'http', 1, 'https://site.test/blog/a2', 0)",
    );
    file
}

fn run(
    file: &CrawlFile,
    out: &std::path::Path,
    q: &ViewQuery,
    format: Format,
) -> os_export::ExportFileInfo {
    export_tab_in(
        &[page_titles_tab()],
        file,
        &CrawlConfig::default(),
        q,
        out,
        format,
    )
    .unwrap()
}

// spec: OUT-020
#[test]
fn standard_export_has_the_visible_columns_all_columns_adds_the_hidden_ones() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    let q = ViewQuery::new("Page Titles", "All");

    let info = run(&file, &dir.path().join("std"), &q, Format::Csv);
    let std_csv = read_csv(&info.path);
    assert_eq!(
        std_csv.lines().next().unwrap(),
        "Address,Title 1,Status Code,Word Count",
        "ohne die Option fehlen ausgeblendete Spalten und Erweiterungen"
    );

    let mut all = q.clone();
    all.columns = ColumnSelection::All;
    let info = run(&file, &dir.path().join("all"), &all, Format::Csv);
    let all_csv = read_csv(&info.path);
    assert_eq!(
        all_csv.lines().next().unwrap(),
        "Address,Title 1,Status Code,Word Count,Title 1 Pixel Width,Response Time,Text Ratio,Hash,\
         Canonical Link Element 1,Canonical Link Element 2,Ext Tiefe",
        "mit der Option auch Title 1 Pixel Width; die Erweiterungsspalte steht hinter den Referenzspalten"
    );
    // Mehrfachspalte: zweite Spalte nur bei der URL mit zwei Canonicals gefüllt
    let blog_a = all_csv.lines().find(|l| l.contains("/blog/a")).unwrap();
    assert!(
        blog_a.contains("https://site.test/blog/a,https://site.test/blog/a2"),
        "{blog_a}"
    );
}

// spec: ONPAGE-191
#[test]
fn export_of_the_current_filter_view_uses_all_columns_on_request() {
    // Das Kriterium „Page Titles:Missing mit ‚Alle Spalten‘ enthält Title 1 Pixel Width, ohne nicht“ an
    // einer URL mit fehlendem Titel; der Golden-Vergleich mit der echten Fixture onpage-titles folgt in P26.
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    insert_url(
        &file,
        7,
        "https://site.test/ohne-titel",
        true,
        &[("status_code", "200"), ("title_1_px", "0")],
    );
    let mut q = ViewQuery::new("Page Titles", "Missing");
    let std = read_csv(&run(&file, &dir.path().join("a"), &q, Format::Csv).path);
    assert!(!std.contains("Title 1 Pixel Width"));
    q.columns = ColumnSelection::All;
    let all = read_csv(&run(&file, &dir.path().join("b"), &q, Format::Csv).path);
    assert!(all.lines().next().unwrap().contains("Title 1 Pixel Width"));
    assert_eq!(all.lines().count(), 2, "nur die URL ohne Titel");
    assert!(all
        .lines()
        .nth(1)
        .unwrap()
        .starts_with("https://site.test/ohne-titel,"));
}

// spec: ONPAGE-191
#[test]
#[ignore = "M1-P26"]
fn golden_page_titles_missing_with_all_columns() {
    // P26: Fixture onpage-titles (P22) crawlen, `Page Titles:Missing` mit ColumnSelection::All als CSV
    // exportieren und gegen tests/golden/onpage-titles/page_titles__missing.csv (Normalisierung nach
    // 06 §0) prüfen. Erst möglich, wenn P22 Tab, Fixture und Golden liefert.
    let golden = repo_tests_dir().join("golden/onpage-titles/page_titles__missing.csv");
    assert!(golden.exists(), "Golden von P22 fehlt");
}

// spec: OUT-020
#[test]
fn sorting_and_search_come_from_the_view_query() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());

    // Sortierung nach Word Count absteigend: erste Zeile = höchste Wortzahl
    let mut q = ViewQuery::new("Page Titles", "All");
    q.sort = vec![sort("Word Count", true)];
    let csv = read_csv(&run(&file, &dir.path().join("s"), &q, Format::Csv).path);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[1], "https://site.test/blog/a,Blog A,200,900");
    // Gleichstand (450): Address aufsteigend, dann UrlId
    assert_eq!(lines[2], "https://site.test/blog/b,Blog B,200,450");
    assert_eq!(lines[3], "https://site.test/other,Sonstiges,200,450");

    // Suche /blog/ → nur passende Zeilen
    let mut q = ViewQuery::new("Page Titles", "All");
    q.search = Some(SearchSpec::Simple("/blog/".into()));
    let info = run(&file, &dir.path().join("q"), &q, Format::Csv);
    assert_eq!(info.rows, 2);
    let csv = read_csv(&info.path);
    assert!(csv.lines().skip(1).all(|l| l.contains("/blog/")));
}

// spec: OUT-020
#[test]
fn filter_export_has_exactly_the_rows_counted_for_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    let cfg = CrawlConfig::default();
    let tabs = [page_titles_tab()];
    let counts = filter_counts_in(&tabs, &file, &cfg).unwrap();
    let c = counts
        .iter()
        .find(|c| c.filter == "Client Error (4XX)")
        .unwrap();
    let info = run(
        &file,
        dir.path(),
        &ViewQuery::new("Page Titles", "Client Error (4xx)"),
        Format::Csv,
    );
    assert_eq!(c.count, Some(info.rows));
    assert_eq!(info.rows, 1);
    // schwellwertabhängige Filter heißen in der Datei mit dem aktuellen Wert
    assert_eq!(
        canonical_name(&tabs, &cfg, "page titles", "over x characters").unwrap(),
        "Page Titles:Over 60 Characters"
    );
    let info = run(
        &file,
        dir.path(),
        &ViewQuery::new("Page Titles", "Over 60 Characters"),
        Format::Csv,
    );
    assert_eq!(
        info.path.file_name().unwrap(),
        "page_titles_over_60_characters.csv"
    );
}

// spec: OUT-020
#[test]
fn numbers_use_a_decimal_point_and_xlsx_cells_are_typed() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    let mut q = ViewQuery::new("Page Titles", "All");
    q.columns = ColumnSelection::Named(vec![
        "Address".into(),
        "Status Code".into(),
        "Response Time".into(),
        "Text Ratio".into(),
        "Hash".into(),
    ]);
    let csv_info = run(&file, &dir.path().join("c"), &q, Format::Csv);
    let csv = read_csv(&csv_info.path);
    assert!(
        csv.contains("https://site.test/,200,0.512,12.50,abc123"),
        "Response Time mit Punkt, nie 0,512: {csv}"
    );

    let xlsx_info = run(&file, &dir.path().join("x"), &q, Format::Xlsx);
    let mut wb: Xlsx<_> = open_workbook(&xlsx_info.path).unwrap();
    let range = wb.worksheet_range_at(0).unwrap().unwrap();
    let cell = |r: usize, c: usize| range.get((r, c)).unwrap().clone();
    // Zeile 1 = erste Datenzeile (https://site.test/)
    assert!(
        matches!(cell(1, 1), Data::Int(200) | Data::Float(_)),
        "Status Code ist eine Zahl"
    );
    assert!(
        matches!(cell(1, 4), Data::String(ref s) if s == "abc123"),
        "Hash ist Text"
    );
    // `0123` bleibt Text und verliert die führende Null nicht
    assert!(matches!(cell(3, 4), Data::String(ref s) if s == "0123"));
    assert!(wb.sheet_names()[0].starts_with("Page Titles All"));
}

// spec: OUT-020
#[test]
fn warnings_of_the_writers_reach_the_caller() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    // Titel mit 40 000 Zeichen: Excel kürzt bei 32 767 (OUT-003) und meldet es
    let long = "x".repeat(40_000);
    exec(
        &file,
        &format!("UPDATE urls SET title_1 = {} WHERE id = 1", lit(&long)),
    );
    let q = ViewQuery::new("Page Titles", "All");
    let xlsx = run(&file, &dir.path().join("x"), &q, Format::Xlsx);
    assert!(
        !xlsx.warnings.is_empty(),
        "OUT-003-Warnung wird weitergereicht"
    );
    let csv = run(&file, &dir.path().join("c"), &q, Format::Csv);
    assert!(csv.warnings.is_empty());
}

// spec: OUT-020
#[test]
fn suggested_file_name_uses_the_local_start_date_of_the_crawl() {
    // Beispiel der Spec: gestartet 2026-10-13 23:30 Ortszeit (21:30 UTC, +02:00), exportiert am Tag danach.
    let started = Timestamp::from_db("2026-10-13T21:30:00.000Z").unwrap();
    let plus2 = UtcOffset::from_hms(2, 0, 0).unwrap();
    assert_eq!(
        suggest_file_name(
            "Response Codes:Client Error (4xx)",
            None,
            Some(started),
            Some(plus2),
            Format::Csv
        ),
        "response_codes_client_error_4xx_2026-10-13.csv"
    );
    // eine Stunde später beginnt in +02:00 schon der nächste Tag
    let later = Timestamp::from_db("2026-10-13T22:30:00.000Z").unwrap();
    assert_eq!(
        suggest_file_name("Internal:All", None, Some(later), Some(plus2), Format::Xlsx),
        "internal_all_2026-10-14.xlsx"
    );
    // westlich von UTC liegt der Start noch am Vortag
    let early = Timestamp::from_db("2026-10-14T01:30:00.000Z").unwrap();
    let minus5 = UtcOffset::from_hms(-5, 0, 0).unwrap();
    assert_eq!(
        suggest_file_name("Internal:All", None, Some(early), Some(minus5), Format::Csv),
        "internal_all_2026-10-13.csv"
    );
    // Segmentsuffix vor dem Datum
    assert_eq!(
        suggest_file_name(
            "Internal:All",
            Some("Blog"),
            Some(started),
            Some(plus2),
            Format::Csv
        ),
        "internal_all_segment_blog_2026-10-13.csv"
    );
    // ohne Startzeit kein Datum
    assert_eq!(
        suggest_file_name("Internal:All", None, None, None, Format::Csv),
        "internal_all.csv"
    );
}

// spec: OUT-020
#[test]
fn suggested_name_for_a_crawl_file_reads_the_start_time_from_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = page_file(dir.path());
    let cfg = CrawlConfig::default();
    let tabs = [page_titles_tab()];
    let plus2 = UtcOffset::from_hms(2, 0, 0).unwrap();
    // noch nicht gestartet: ohne Datum
    assert_eq!(
        suggest_tab_file_name_in(
            &tabs,
            &file,
            &cfg,
            "Page Titles",
            "Missing",
            None,
            Some(plus2),
            Format::Csv
        )
        .unwrap(),
        "page_titles_missing.csv"
    );
    file.set_meta("started_at", "2026-10-13T21:30:00.000Z")
        .unwrap();
    assert_eq!(
        suggest_tab_file_name_in(
            &tabs,
            &file,
            &cfg,
            "Page Titles",
            "Missing",
            None,
            Some(plus2),
            Format::Csv
        )
        .unwrap(),
        "page_titles_missing_2026-10-13.csv"
    );
}
