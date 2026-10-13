//! CRAWL-021: Zeichensatz-Erkennung gegen die Fixture `crawl-charsets`. Der `Content-Type` jeder Route
//! stammt aus der `scenario.toml` der Fixture (derselbe Wert, den der Testserver ausliefert).

use os_parse::charset;
use os_parse::dom::Document;
use scraper::Selector;

const TITLE: &str = "Grüße ÄÖÜ ß";

fn fixture_dir() -> std::path::PathBuf {
    os_testsite::fixtures_root().join("crawl-charsets")
}

fn content_type_of(route: &str) -> Option<String> {
    let text = std::fs::read_to_string(fixture_dir().join("scenario.toml")).unwrap();
    let scenario = os_testsite::Scenario::parse(&text).unwrap();
    scenario
        .routes
        .into_iter()
        .find(|r| r.path == format!("/{route}"))
        .unwrap_or_else(|| panic!("keine Route /{route}"))
        .content_type
}

/// Dekodiert die Fixture-Datei so, wie die Engine die Antwort der Route dekodieren würde.
fn decode_route(route: &str) -> (String, Option<String>) {
    let body = std::fs::read(fixture_dir().join(route)).unwrap();
    let content_type = content_type_of(route);
    charset::decode(&body, content_type.as_deref(), &[])
}

fn title_of(html: &str) -> String {
    let doc = Document::parse(html);
    let sel = Selector::parse("title").unwrap();
    let title = doc.html().select(&sel).next().expect("kein <title>");
    title.text().collect::<String>()
}

// (Route, erwarteter Zeichensatz, erwarteter Titel)
const EXPECTED: [(&str, &str, &str); 16] = [
    // Stufe 1: BOM (UTF-8, UTF-16LE, UTF-16BE); BOM schlägt den Header
    ("utf8-bom", "UTF-8", TITLE),
    ("utf16le-bom", "UTF-16LE", TITLE),
    ("utf16be-bom", "UTF-16BE", TITLE),
    ("bom-vs-header", "UTF-8", TITLE),
    // Stufe 2: Header; ISO-8859-1 und latin1 sind WHATWG-Aliase von windows-1252; Header schlägt Meta
    ("latin1-header", "windows-1252", TITLE),
    ("latin1-alias", "windows-1252", TITLE),
    ("header-vs-meta", "UTF-8", TITLE),
    // Stufe 3: <meta charset> und <meta http-equiv> in den ersten 1024 Bytes
    ("meta-sjis", "Shift_JIS", "日本語のタイトル"),
    ("meta-euc-kr", "EUC-KR", "한국어 제목"),
    ("meta-gb18030", "gb18030", "中文标题"),
    ("meta-http-equiv", "windows-1252", TITLE),
    // Unbekannter Name -> nächste Stufe
    ("unknown-charset", "windows-1252", TITLE), // Header unbekannt -> Meta iso-8859-1
    ("unknown-charset-no-meta", "UTF-8", TITLE), // Header unbekannt, kein Meta -> UTF-8
    ("unknown-meta", "UTF-8", TITLE),           // Meta unbekannt -> UTF-8
    // Meta erst nach 1024 Bytes bzw. auskommentiert zählt nicht
    ("meta-late", "UTF-8", TITLE),
    ("meta-commented", "UTF-8", TITLE),
];

// spec: CRAWL-021
#[test]
fn every_stage_decodes_the_expected_title_and_reports_the_charset() {
    for (route, charset_name, title) in EXPECTED {
        let (html, detected) = decode_route(route);
        assert_eq!(
            detected.as_deref(),
            Some(charset_name),
            "{route}: Zeichensatz"
        );
        assert_eq!(title_of(&html), title, "{route}: Titel");
    }
}

// spec: CRAWL-021
#[test]
fn undeclared_utf8_and_windows_1252_fallback() {
    let (html, detected) = decode_route("plain-utf8");
    assert_eq!(detected.as_deref(), Some("UTF-8"));
    assert_eq!(title_of(&html), TITLE);

    // Keine Angabe, aber kein gültiges UTF-8 -> Windows-1252
    let (html, detected) = decode_route("invalid-bytes-undeclared");
    assert_eq!(detected.as_deref(), Some("windows-1252"));
    assert_eq!(title_of(&html), TITLE);
}

// spec: CRAWL-021
#[test]
fn invalid_bytes_in_a_declared_encoding_become_replacement_characters() {
    // Header erklärt UTF-8, der Body enthält Latin-1-Bytes: U+FFFD statt Absturz.
    let (html, detected) = decode_route("invalid-bytes");
    assert_eq!(detected.as_deref(), Some("UTF-8"));
    let title = title_of(&html);
    assert_eq!(
        title.chars().filter(|c| *c == '\u{FFFD}').count(),
        6,
        "{title}"
    );
    assert!(title.starts_with("Gr") && title.contains("e "));
}

// spec: CRAWL-021
#[test]
fn bom_beats_header_and_header_beats_meta_inline() {
    let ct = Some("text/html; charset=iso-8859-1");
    // UTF-8-BOM gewinnt gegen den Header
    let (text, name) = charset::decode(b"\xEF\xBB\xBF<p>\xC3\xA4", ct, &[]);
    assert_eq!(name.as_deref(), Some("UTF-8"));
    assert_eq!(text, "<p>ä");
    // Header gewinnt gegen Meta
    let (text, name) = charset::decode(b"<meta charset=utf-8><p>\xE4", ct, &[]);
    assert_eq!(name.as_deref(), Some("windows-1252"));
    assert!(text.ends_with("ä"));
    // Content-Type aus den Headern, wenn kein Parameter übergeben wird
    let headers = vec![(
        "Content-Type".to_string(),
        "text/html; charset=\"Shift_JIS\"".to_string(),
    )];
    let (_, name) = charset::decode(b"abc", None, &headers);
    assert_eq!(name.as_deref(), Some("Shift_JIS"));
}

// spec: CRAWL-021
#[test]
fn unknown_and_replacement_labels_fall_through() {
    let (_, name) = charset::decode(b"abc", Some("text/html; charset=foo"), &[]);
    assert_eq!(name.as_deref(), Some("UTF-8"));
    // `iso-2022-kr` ist im WHATWG-Standard das Ersatz-Encoding und wird nicht benutzt
    let (text, name) = charset::decode(b"abc", Some("text/html; charset=iso-2022-kr"), &[]);
    assert_eq!(name.as_deref(), Some("UTF-8"));
    assert_eq!(text, "abc");
    // leerer Body
    let (text, name) = charset::decode(b"", None, &[]);
    assert_eq!((text.as_str(), name.as_deref()), ("", Some("UTF-8")));
}

// spec: CRAWL-021
#[test]
fn parse_html_reports_the_detected_charset() {
    let body = std::fs::read(fixture_dir().join("meta-sjis")).unwrap();
    let url = url::Url::parse("https://site.test/meta-sjis").unwrap();
    let cfg = os_parse::ParseConfig::from_crawl_config(&os_model::CrawlConfig::default());
    let page = os_parse::parse_html(&os_parse::ParseInput {
        url: &url,
        body: &body,
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    assert_eq!(page.charset.as_deref(), Some("Shift_JIS"));
    // Linktext und Ziel stammen aus dem korrekt dekodierten Dokument
    assert_eq!(page.links[0].anchor.as_deref(), Some("日本語のタイトル"));
    assert_eq!(
        page.links[0].resolved.as_deref(),
        Some("https://site.test/ziel")
    );
}

// spec: CRAWL-021
#[tokio::test]
async fn served_responses_decode_like_the_files_on_disk() {
    // Ende-zu-Ende: der Testserver liefert die Rohbytes und den Content-Type der `scenario.toml` aus.
    let site = os_testsite::TestSite::start("crawl-charsets")
        .await
        .unwrap();
    for (route, charset_name, title) in EXPECTED {
        let response = os_testsite::client::get_once(site.addr(), &format!("/{route}"))
            .await
            .unwrap();
        assert_eq!(response.status, 200, "{route}");
        let (html, detected) = charset::decode(
            &response.body,
            response.header("content-type"),
            &response.headers,
        );
        assert_eq!(detected.as_deref(), Some(charset_name), "{route}");
        assert_eq!(title_of(&html), title, "{route}");
    }
}
