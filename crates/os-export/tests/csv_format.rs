//! Tests für den CSV-Schreiber (OUT-002) gegen die Fixture `out-csv-edge`.

use std::path::Path;

use os_analyze::ColumnKind;
use os_export::csv::utf8_lossy;
use os_export::output::open_writer;
use os_export::{Format, WriterOptions};
use os_model::{LineEnding, Timestamp, Value};
use os_testsite::client::get_once;
use os_testsite::TestSite;

/// Holt `/<page>` vom Testsite und liefert den Inhalt des `<title>` (ungültiges UTF-8 wird zu U+FFFD).
async fn title_of(site: &TestSite, page: &str) -> String {
    let resp = get_once(site.addr(), page).await.unwrap();
    assert_eq!(resp.status, 200, "{page}");
    let body = resp.body;
    let start = find(&body, b"<title>").expect("title") + 7;
    let end = start + find(&body[start..], b"</title>").expect("title end");
    utf8_lossy(&body[start..end])
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

const PAGES: [&str; 6] = [
    "/comma.html",
    "/quote.html",
    "/multiline.html",
    "/unicode.html",
    "/latin1.html",
    "/formula.html",
];

async fn write_titles(path: &Path, opts: &WriterOptions) -> Vec<(String, String)> {
    let site = TestSite::start("out-csv-edge").await.unwrap();
    let mut rows = Vec::new();
    for p in PAGES {
        rows.push((format!("{}{p}", site.base_url()), title_of(&site, p).await));
    }
    let mut w = open_writer(path, Format::Csv, "Internal", opts).unwrap();
    w.header(&["Address".to_string(), "Title 1".to_string()])
        .unwrap();
    for (a, t) in &rows {
        w.row(
            &[ColumnKind::Url, ColumnKind::Text],
            &[Value::Text(a.clone()), Value::Text(t.clone())],
        )
        .unwrap();
    }
    let info = w.finish().unwrap();
    assert_eq!(info.rows, PAGES.len() as u64);
    assert_eq!(info.bytes, std::fs::metadata(path).unwrap().len());
    rows
}

fn parse(bytes: &[u8]) -> Vec<Vec<String>> {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(body);
    rdr.records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

// spec: OUT-002
#[tokio::test]
async fn quoting_follows_rfc_4180_and_roundtrips() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("internal_all.csv");
    let rows = write_titles(&path, &WriterOptions::default()).await;
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();

    // Trennzeichen, Anführungszeichen und Zeilenumbruch erzwingen Quoting, `"` wird verdoppelt
    assert!(text.contains(",\"a,b\"\r\n"), "{text}");
    assert!(text.contains(",\"\"\"Zitat\"\"\"\r\n"), "{text}");
    assert!(text.contains(",\"Zeile1\nZeile2\"\r\n"), "{text}");
    // Felder ohne Sonderzeichen bleiben ohne Quotes
    assert!(text.contains("Address,Title 1\r\n"), "{text}");
    assert!(
        text.contains(",=SUM(A1)\r\n"),
        "CSV verändert Formel-Optik nicht: {text}"
    );

    // ein RFC-konformer Parser liest exakt die Eingabe zurück (auch der mehrzeilige Title)
    let parsed = parse(&bytes);
    assert_eq!(parsed.len(), rows.len() + 1);
    assert_eq!(parsed[0], vec!["Address", "Title 1"]);
    for (i, (addr, title)) in rows.iter().enumerate() {
        assert_eq!(&parsed[i + 1][0], addr);
        assert_eq!(&parsed[i + 1][1], title);
    }
    assert_eq!(parsed[3][1], "Zeile1\nZeile2");
}

// spec: OUT-002
#[tokio::test]
async fn bom_is_written_by_default_and_omitted_on_request() {
    let tmp = tempfile::tempdir().unwrap();
    let with = tmp.path().join("with.csv");
    write_titles(&with, &WriterOptions::default()).await;
    let b = std::fs::read(&with).unwrap();
    assert_eq!(&b[..3], &[0xEF, 0xBB, 0xBF]);
    assert_ne!(&b[3..6], &[0xEF, 0xBB, 0xBF], "BOM nur einmal");

    let without = tmp.path().join("without.csv");
    let opts = WriterOptions {
        utf8_bom: false,
        ..WriterOptions::default()
    };
    write_titles(&without, &opts).await;
    let b = std::fs::read(&without).unwrap();
    assert!(
        b.starts_with(b"Address,"),
        "ohne BOM beginnt die Datei mit der Kopfzeile"
    );
}

// spec: OUT-002
#[tokio::test]
async fn non_ascii_is_written_unchanged_as_utf8() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("u.csv");
    write_titles(&path, &WriterOptions::default()).await;
    let b = std::fs::read(&path).unwrap();
    let needle = "日本語 🕷".as_bytes();
    assert!(
        find(&b, needle).is_some(),
        "UTF-8-Bytes von 日本語 🕷 fehlen"
    );
    // zusätzlich ü
    let mut w = open_writer(
        &tmp.path().join("ue.csv"),
        Format::Csv,
        "x",
        &WriterOptions::default(),
    )
    .unwrap();
    w.header(&["Größe".into()]).unwrap();
    w.row(&[ColumnKind::Text], &[Value::Text("Jürgen".into())])
        .unwrap();
    w.finish().unwrap();
    let text = std::fs::read_to_string(tmp.path().join("ue.csv")).unwrap();
    assert!(text.contains("Größe\r\nJürgen\r\n"));
}

// spec: OUT-002
#[test]
fn numbers_are_unquoted_with_decimal_point_under_german_locale() {
    // Der Schreiber liest keine Locale-Variablen; der Test belegt es, indem er sie auf Deutsch stellt.
    std::env::set_var("LANG", "de_DE.UTF-8");
    std::env::set_var("LC_ALL", "de_DE.UTF-8");
    std::env::set_var("LC_NUMERIC", "de_DE.UTF-8");

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("n.csv");
    let opts = WriterOptions {
        utf8_bom: false,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&path, Format::Csv, "n", &opts).unwrap();
    w.header(&[
        "Int".into(),
        "Pct".into(),
        "Secs".into(),
        "Big".into(),
        "Empty".into(),
        "Zero".into(),
        "Flag".into(),
    ])
    .unwrap();
    let kinds = [
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Seconds3,
        ColumnKind::Float { decimals: 1 },
        ColumnKind::Int,
        ColumnKind::Int,
        ColumnKind::Bool,
    ];
    w.row(
        &kinds,
        &[
            Value::Int(1234567),
            Value::Float(0.512),
            Value::Int(1234),
            Value::Float(1234.5),
            Value::Null,
            Value::Int(0),
            Value::Bool(true),
        ],
    )
    .unwrap();
    w.row(
        &kinds,
        &[
            Value::Int(-3),
            Value::Float(100.0),
            Value::Float(250.0),
            Value::Float(0.05),
            Value::Null,
            Value::Int(0),
            Value::Bool(false),
        ],
    )
    .unwrap();
    w.finish().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    assert_eq!(lines[0], "Int,Pct,Secs,Big,Empty,Zero,Flag");
    // 0.512 mit zwei Stellen: 0.51; Millisekunden als Sekunden; leere Zelle für Null; echte 0 bleibt 0
    assert_eq!(lines[1], "1234567,0.51,1.234,1234.5,,0,true");
    assert_eq!(lines[2], "-3,100.00,0.250,0.1,,0,false");
    assert!(
        !text.contains('"'),
        "Zahlen stehen nie in Anführungszeichen"
    );
    assert!(!text.contains("0,51"), "kein Dezimalkomma");
}

// spec: OUT-002
#[tokio::test]
async fn invalid_utf8_becomes_replacement_character_without_aborting() {
    let site = TestSite::start("out-csv-edge").await.unwrap();
    let title = title_of(&site, "/latin1.html").await;
    assert!(title.contains('\u{FFFD}'), "{title:?}");
    assert!(title.starts_with("Caf") && title.ends_with("ende"));

    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("l.csv");
    let mut w = open_writer(&path, Format::Csv, "l", &WriterOptions::default()).unwrap();
    w.header(&["Title 1".into()]).unwrap();
    w.row(&[ColumnKind::Text], &[Value::Text(title.clone())])
        .unwrap();
    w.row(&[ColumnKind::Text], &[Value::Text("danach".into())])
        .unwrap();
    let info = w.finish().unwrap();
    assert_eq!(info.rows, 2, "Export bricht nicht ab");
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        find(&bytes, &[0xEF, 0xBF, 0xBD]).is_some(),
        "U+FFFD als UTF-8 erwartet"
    );
    assert!(String::from_utf8(bytes).is_ok(), "Datei ist gültiges UTF-8");
}

// spec: OUT-002
#[test]
fn line_ending_and_header_only_files() {
    let tmp = tempfile::tempdir().unwrap();
    let lf = tmp.path().join("lf.csv");
    let opts = WriterOptions {
        utf8_bom: false,
        line_ending: LineEnding::Lf,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&lf, Format::Csv, "x", &opts).unwrap();
    w.header(&["A".into(), "B".into()]).unwrap();
    w.row(
        &[ColumnKind::Text, ColumnKind::Text],
        &[Value::Text("1\r\n2".into()), Value::Text("x".into())],
    )
    .unwrap();
    w.finish().unwrap();
    assert_eq!(std::fs::read(&lf).unwrap(), b"A,B\n\"1\r\n2\",x\n");

    // Kopfzeile ohne Datenzeilen (OUT-001: Kopfzeile immer vorhanden)
    let hdr = tmp.path().join("h.csv");
    let mut w = open_writer(&hdr, Format::Csv, "x", &WriterOptions::default()).unwrap();
    w.header(&["Address".into()]).unwrap();
    let info = w.finish().unwrap();
    assert_eq!(info.rows, 0);
    assert_eq!(std::fs::read(&hdr).unwrap(), b"\xEF\xBB\xBFAddress\r\n");
}

// spec: OUT-002
#[test]
fn urls_and_timestamps_are_written_as_text_and_iso_8601() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("t.csv");
    let opts = WriterOptions {
        utf8_bom: false,
        ..WriterOptions::default()
    };
    let mut w = open_writer(&path, Format::Csv, "t", &opts).unwrap();
    w.header(&[
        "Address".into(),
        "Crawl Timestamp".into(),
        "Last Modified".into(),
        "Hash".into(),
    ])
    .unwrap();
    let utc = "2026-10-13T07:35:00.123Z";
    w.row(
        &[
            ColumnKind::Url,
            ColumnKind::Timestamp,
            ColumnKind::Timestamp,
            ColumnKind::Hash,
        ],
        &[
            Value::Text("https://example.com/a?x=1,2".into()),
            Value::Text(utc.into()),
            Value::Text("2026-10-13T09:35:00+02:00".into()),
            Value::Text("00123456789abcdef".into()),
        ],
    )
    .unwrap();
    w.finish().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let line = text.split("\r\n").nth(1).unwrap();
    let expected_local = Timestamp::from_db(utc).unwrap().to_export();
    assert_eq!(
        line,
        format!(
            "\"https://example.com/a?x=1,2\",{expected_local},2026-10-13T09:35:00+02:00,00123456789abcdef"
        )
    );
    // ISO 8601 mit Offset, ohne Sekundenbruchteile
    assert_eq!(expected_local.len(), 25);
    assert!(!expected_local.contains('.'));
}

// spec: OUT-002
#[test]
fn row_width_must_match_header() {
    let tmp = tempfile::tempdir().unwrap();
    let mut w = open_writer(
        &tmp.path().join("w.csv"),
        Format::Csv,
        "x",
        &WriterOptions::default(),
    )
    .unwrap();
    w.header(&["A".into(), "B".into()]).unwrap();
    assert!(w.row(&[ColumnKind::Text], &[Value::Null]).is_err());
    assert!(
        w.header(&["A".into()]).is_err(),
        "zweite Kopfzeile ist ein Fehler"
    );
}
