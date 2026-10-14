//! Tests für den XLSX-Schreiber (OUT-003): Zellarten, Formeln als Text, Kürzung, Überlauf, Blattnamen.
//! Gelesen wird mit `calamine`.

use std::path::Path;

use calamine::{open_workbook, Data, Reader, Xlsx};
use os_analyze::ColumnKind;
use os_export::csv::utf8_lossy;
use os_export::output::open_writer;
use os_export::xlsx::{MAX_CELL_CHARS, TRUNCATION_MARKER};
use os_export::{Format, WriterOptions};
use os_model::Value;
use os_testsite::client::get_once;
use os_testsite::TestSite;

fn open(path: &Path) -> Xlsx<std::io::BufReader<std::fs::File>> {
    open_workbook(path).unwrap()
}

fn num(d: &Data) -> f64 {
    match d {
        Data::Float(f) => *f,
        Data::Int(i) => *i as f64,
        other => panic!("keine Zahl: {other:?}"),
    }
}

fn text(d: &Data) -> &str {
    match d {
        Data::String(s) => s,
        other => panic!("kein Text: {other:?}"),
    }
}

async fn title_of(site: &TestSite, page: &str) -> String {
    let body = get_once(site.addr(), page).await.unwrap().body;
    let find = |h: &[u8], n: &[u8]| h.windows(n.len()).position(|w| w == n).unwrap();
    let s = find(&body, b"<title>") + 7;
    let e = s + find(&body[s..], b"</title>");
    utf8_lossy(&body[s..e])
}

// spec: OUT-003
#[test]
fn cell_types_follow_column_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("cells.xlsx");
    let mut w = open_writer(&path, Format::Xlsx, "Internal", &WriterOptions::default()).unwrap();
    let cols: Vec<String> = [
        "Address",
        "Status Code",
        "Size",
        "Pct",
        "Response Time",
        "Crawl Timestamp",
        "Hash",
        "Indexable",
        "Empty",
    ]
    .map(String::from)
    .to_vec();
    w.header(&cols).unwrap();
    let kinds = [
        ColumnKind::Url,
        ColumnKind::Int,
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Seconds3,
        ColumnKind::Timestamp,
        ColumnKind::Hash,
        ColumnKind::Bool,
        ColumnKind::Text,
    ];
    w.row(
        &kinds,
        &[
            Value::Text("https://example.com/".into()),
            Value::Int(200),
            Value::Int(123_456_789_012),
            Value::Float(12.3456),
            Value::Float(0.812),
            Value::Text("2026-10-13T09:35:00+02:00".into()),
            // sieht aus wie eine Zahl, ist aber ein Hash: immer Text, führende Nullen bleiben
            Value::Text("00123456".into()),
            Value::Bool(true),
            Value::Null,
        ],
    )
    .unwrap();
    // echte 0 bleibt 0, nicht leer
    w.row(
        &kinds,
        &[
            Value::Text("https://example.com/x".into()),
            Value::Int(0),
            Value::Int(0),
            Value::Float(0.0),
            Value::Int(0),
            Value::Null,
            Value::Int(42),
            Value::Bool(false),
            Value::Text("".into()),
        ],
    )
    .unwrap();
    let info = w.finish().unwrap();
    assert_eq!(info.rows, 2);
    assert_eq!(info.bytes, std::fs::metadata(&path).unwrap().len());

    let mut wb = open(&path);
    assert_eq!(wb.sheet_names(), vec!["Internal"]);
    let range = wb.worksheet_range("Internal").unwrap();
    assert_eq!(range.height(), 3, "Kopfzeile + 2 Datenzeilen");
    for (i, c) in cols.iter().enumerate() {
        assert_eq!(text(range.get_value((0, i as u32)).unwrap()), c);
    }
    let r1 = |c: u32| range.get_value((1, c)).unwrap();
    assert_eq!(text(r1(0)), "https://example.com/");
    assert_eq!(num(r1(1)), 200.0);
    assert!(
        matches!(r1(1), Data::Int(_) | Data::Float(_)),
        "Zahlenzelle"
    );
    assert_eq!(num(r1(2)), 123_456_789_012.0);
    assert_eq!(num(r1(3)), 12.35, "auf die Nachkommastellen gerundet");
    assert_eq!(
        num(r1(4)),
        0.812,
        "Seconds3 = Sekunden, keine Umrechnung im Schreiber"
    );
    // Datumszelle: Seriennummer von 2026-10-13 09:35:00 (46308 + 9:35 h)
    match r1(5) {
        Data::DateTime(dt) => {
            let expected = 46308.0 + (9.0 * 3600.0 + 35.0 * 60.0) / 86400.0;
            assert!((dt.as_f64() - expected).abs() < 1e-6, "{}", dt.as_f64());
            assert!(dt.is_datetime());
        }
        other => panic!("keine Datumszelle: {other:?}"),
    }
    assert_eq!(text(r1(6)), "00123456", "Hash immer Text");
    assert_eq!(r1(7), &Data::Bool(true));
    assert!(
        range.get_value((1, 8)).is_none_or(|d| d == &Data::Empty),
        "Null = leere Zelle"
    );

    let r2 = |c: u32| range.get_value((2, c)).unwrap();
    assert_eq!(num(r2(1)), 0.0);
    assert_eq!(num(r2(2)), 0.0);
    assert_eq!(num(r2(4)), 0.0);
    assert!(matches!(r2(5), Data::Empty));
    assert_eq!(text(r2(6)), "42", "Hash-Spalte mit Zahlenwert wird Text");
    assert_eq!(r2(7), &Data::Bool(false));
}

// spec: OUT-003
#[tokio::test]
async fn formulas_are_never_written_titles_stay_text() {
    let site = TestSite::start("out-csv-edge").await.unwrap();
    let formula = title_of(&site, "/formula.html").await;
    assert_eq!(formula, "=SUM(A1)");
    let comma = title_of(&site, "/comma.html").await;
    let multi = title_of(&site, "/multiline.html").await;
    let uni = title_of(&site, "/unicode.html").await;
    let quote = title_of(&site, "/quote.html").await;

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("t.xlsx");
    let mut w = open_writer(
        &path,
        Format::Xlsx,
        "Internal: All",
        &WriterOptions::default(),
    )
    .unwrap();
    w.header(&["Title 1".into()]).unwrap();
    let titles = [
        formula.clone(),
        "=1+1".into(),
        "+SUM(A1)".into(),
        "@SUM(A1)".into(),
        "-2+3".into(),
        comma.clone(),
        multi.clone(),
        uni.clone(),
        quote.clone(),
    ];
    for t in &titles {
        w.row(&[ColumnKind::Text], &[Value::Text(t.clone())])
            .unwrap();
    }
    w.finish().unwrap();

    let mut wb = open(&path);
    let name = wb.sheet_names()[0].clone();
    let range = wb.worksheet_range(&name).unwrap();
    for (i, t) in titles.iter().enumerate() {
        assert_eq!(
            text(range.get_value((i as u32 + 1, 0)).unwrap()),
            t,
            "Zeile {i}"
        );
    }
    let formulas = wb.worksheet_formula(&name).unwrap();
    assert!(
        formulas.rows().all(|r| r.iter().all(|f| f.is_empty())),
        "es darf keine Formel in der Datei stehen"
    );
}

// spec: OUT-003
#[test]
fn long_texts_are_truncated_with_marker() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("long.xlsx");
    let mut w = open_writer(&path, Format::Xlsx, "Long", &WriterOptions::default()).unwrap();
    w.header(&["Text".into()]).unwrap();
    let exact = "e".repeat(MAX_CELL_CHARS);
    let over = "o".repeat(MAX_CELL_CHARS + 1);
    let huge = "ü".repeat(100_000);
    // Emoji zählen in Excel als zwei Einheiten; 20 000 Stück = 40 000 Einheiten
    let emoji = "🕷".repeat(20_000);
    for t in [&exact, &over, &huge, &emoji] {
        w.row(&[ColumnKind::Text], &[Value::Text(t.clone())])
            .unwrap();
    }
    let info = w.finish().unwrap();
    // drei gekürzte Zellen (over, huge, emoji) stehen als Hinweis in der Rückgabe
    assert_eq!(info.warnings.len(), 1, "{:?}", info.warnings);
    assert!(
        info.warnings[0].starts_with("3 Zelle(n)") && info.warnings[0].contains("32767"),
        "{:?}",
        info.warnings
    );

    let mut wb = open(&path);
    let range = wb.worksheet_range("Long").unwrap();
    let cell = |r: u32| text(range.get_value((r, 0)).unwrap()).to_string();
    assert_eq!(cell(1), exact, "genau 32 767 Zeichen bleiben unverändert");
    for r in 2..=3 {
        let c = cell(r);
        assert_eq!(c.chars().count(), MAX_CELL_CHARS, "Zeile {r}");
        assert!(c.ends_with(TRUNCATION_MARKER), "Zeile {r}");
    }
    assert!(cell(2).starts_with("ooo"));
    let e = cell(4);
    assert!(e.ends_with(TRUNCATION_MARKER));
    assert!(e.encode_utf16().count() <= MAX_CELL_CHARS);
}

// spec: OUT-003
#[test]
fn regular_file_has_no_warnings() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("plain.xlsx");
    let mut w = open_writer(&path, Format::Xlsx, "Plain", &WriterOptions::default()).unwrap();
    w.header(&["Text".into()]).unwrap();
    w.row(
        &[ColumnKind::Text],
        &[Value::Text("e".repeat(MAX_CELL_CHARS))],
    )
    .unwrap();
    let info = w.finish().unwrap();
    assert!(info.warnings.is_empty(), "{:?}", info.warnings);
}

// spec: OUT-003
#[test]
fn rows_overflow_to_following_sheets_with_header() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("overflow.xlsx");
    let opts = WriterOptions {
        max_rows_per_sheet: 3,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&path, Format::Xlsx, "Internal All", &opts).unwrap();
    w.header(&["Address".into(), "n".into()]).unwrap();
    for i in 0..8i64 {
        w.row(
            &[ColumnKind::Url, ColumnKind::Int],
            &[
                Value::Text(format!("https://example.com/{i}")),
                Value::Int(i),
            ],
        )
        .unwrap();
    }
    let info = w.finish().unwrap();
    assert_eq!(info.rows, 8);
    assert_eq!(info.warnings.len(), 1, "{:?}", info.warnings);
    assert!(
        info.warnings[0].contains("Internal All (3)"),
        "Warnung nennt die Folgeblätter: {:?}",
        info.warnings
    );

    let mut wb = open(&path);
    assert_eq!(
        wb.sheet_names(),
        vec!["Internal All", "Internal All (2)", "Internal All (3)"]
    );
    let sizes = [3u32, 3, 2];
    let mut next = 0i64;
    for (name, n) in wb.sheet_names().clone().iter().zip(sizes) {
        let range = wb.worksheet_range(name).unwrap();
        assert_eq!(
            range.height() as u32,
            n + 1,
            "{name}: Kopfzeile + {n} Zeilen"
        );
        assert_eq!(text(range.get_value((0, 0)).unwrap()), "Address", "{name}");
        assert_eq!(text(range.get_value((0, 1)).unwrap()), "n", "{name}");
        for r in 1..=n {
            assert_eq!(num(range.get_value((r, 1)).unwrap()), next as f64);
            next += 1;
        }
    }
    assert_eq!(next, 8, "keine Zeile geht verloren oder doppelt");
}

// spec: OUT-003
#[test]
fn exact_limit_does_not_create_an_empty_following_sheet() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("exact.xlsx");
    let opts = WriterOptions {
        max_rows_per_sheet: 4,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&path, Format::Xlsx, "S", &opts).unwrap();
    w.header(&["a".into()]).unwrap();
    for i in 0..4 {
        w.row(&[ColumnKind::Int], &[Value::Int(i)]).unwrap();
    }
    w.finish().unwrap();
    assert_eq!(open(&path).sheet_names(), vec!["S"]);
}

// spec: OUT-003
#[test]
fn sheet_names_are_valid_short_and_unique() {
    let tmp = tempfile::tempdir().unwrap();
    // verbotene Zeichen und Länge
    let path = tmp.path().join("names.xlsx");
    let mut w = open_writer(
        &path,
        Format::Xlsx,
        "Response Codes:Client Error (4xx) [a]*?/\\",
        &WriterOptions::default(),
    )
    .unwrap();
    w.header(&["a".into()]).unwrap();
    w.finish().unwrap();
    let names = open(&path).sheet_names();
    assert_eq!(names.len(), 1);
    assert!(names[0].chars().count() <= 31, "{names:?}");
    assert!(
        !names[0].contains(['[', ']', ':', '*', '?', '/', '\\']),
        "{names:?}"
    );
    assert!(names[0].starts_with("Response Codes"));

    // 31 Zeichen langer Name + Überlauf: Folgeblätter bleiben ≤ 31 und eindeutig
    let path = tmp.path().join("names2.xlsx");
    let long = "L".repeat(31);
    let opts = WriterOptions {
        max_rows_per_sheet: 1,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&path, Format::Xlsx, &long, &opts).unwrap();
    w.header(&["a".into()]).unwrap();
    for i in 0..12 {
        w.row(&[ColumnKind::Int], &[Value::Int(i)]).unwrap();
    }
    w.finish().unwrap();
    let names = open(&path).sheet_names();
    assert_eq!(names.len(), 12);
    let mut lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    lower.sort();
    lower.dedup();
    assert_eq!(
        lower.len(),
        12,
        "Blattnamen müssen eindeutig sein: {names:?}"
    );
    assert!(names.iter().all(|n| n.chars().count() <= 31), "{names:?}");
    assert!(
        names[1].ends_with(" (2)") && names[11].ends_with(" (12)"),
        "{names:?}"
    );

    // leerer Name
    let path = tmp.path().join("names3.xlsx");
    let w = open_writer(&path, Format::Xlsx, "   ", &WriterOptions::default()).unwrap();
    w.finish().unwrap();
    assert_eq!(open(&path).sheet_names(), vec!["Sheet"]);
}

// spec: OUT-003
#[test]
fn unique_sheet_names_for_consolidated_workbooks() {
    use os_export::xlsx::unique_sheet_name;
    let existing = vec!["Internal All".to_string()];
    assert_eq!(
        unique_sheet_name("internal all", &existing),
        "internal all (2)"
    );
    assert_eq!(
        unique_sheet_name("Internal: All", &existing),
        "Internal All (2)"
    );
    assert_eq!(unique_sheet_name("Other", &existing), "Other");
}

// spec: OUT-003
#[test]
#[ignore = "Benchmark (P28): 1,1 Mio. Zeilen, Blattüberlauf und Speicherbedarf mit constant_memory"]
fn default_limit_overflows_after_1048575_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("big.xlsx");
    let mut w = open_writer(
        &path,
        Format::Xlsx,
        "Internal All",
        &WriterOptions::default(),
    )
    .unwrap();
    w.header(&["Address".into(), "n".into()]).unwrap();
    for i in 0..1_100_000i64 {
        w.row(
            &[ColumnKind::Url, ColumnKind::Int],
            &[
                Value::Text(format!("https://example.com/{i}")),
                Value::Int(i),
            ],
        )
        .unwrap();
    }
    assert_eq!(w.finish().unwrap().rows, 1_100_000);
    let mut wb = open(&path);
    assert_eq!(wb.sheet_names(), vec!["Internal All", "Internal All (2)"]);
    let first = wb.worksheet_range("Internal All").unwrap();
    assert_eq!(
        first.height(),
        1_048_576,
        "Kopfzeile + 1 048 575 Datenzeilen"
    );
    let second = wb.worksheet_range("Internal All (2)").unwrap();
    assert_eq!(second.height(), 1 + (1_100_000 - 1_048_575));
    assert_eq!(text(second.get_value((0, 0)).unwrap()), "Address");
}
