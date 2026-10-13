//! Tab „URL“ (M1-P21): Spalten und die URL-Hygiene-Filter gegen einen echten Crawl der Fixture `onpage-url`
//! (Testsite, Hostname `site.test`) und gegen direkt in die Crawl-Datei geschriebene Zeilen für Grenzfälle.
//!
//! Die Adresslisten je Filter liegen als Goldens in `tests/golden/onpage-url/url__<filter>.csv` (aus der Spec
//! abgeleitet, sortiert nach Address, `{BASE}` = `http://site.test`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_analyze::{tabs, ColumnKind, SortKey};
use os_engine::testing::crawl_fixture_from;
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

const BASE: &str = "http://site.test";
const TAB: &str = "URL";
const OVER: &str = "Over {url.max_chars} Characters";
const FILTERS: [&str; 8] = [
    "All",
    "Non ASCII Characters",
    "Underscores",
    "Uppercase",
    "Multiple Slashes",
    "Contains A Space",
    "Parameters",
    OVER,
];

/// Schwelle für den dynamischen Längenfilter im zweiten Durchlauf ohne neuen Crawl.
const ALT_MAX: u32 = 56;

struct Dump {
    /// Filter-Kennung → Adressen (sortiert nach Address, `{BASE}` statt `http://site.test`).
    filters: BTreeMap<String, Vec<String>>,
    /// Alle Spalten der Ansicht „All“ und ihre Zeilen (sortiert nach Address).
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    default_columns: Vec<String>,
    /// Treffer des Längenfilters mit `url.max_chars = ALT_MAX` auf derselben Crawl-Datei (ohne Recrawl).
    over_alt: Vec<String>,
    /// Anzeigename des Längenfilters mit Standardschwelle und mit `ALT_MAX`.
    label_default: String,
    label_alt: String,
}

fn normalize(address: &str) -> String {
    address.replace(BASE, "{BASE}")
}

fn text(v: &Value) -> Option<&str> {
    match v {
        Value::Text(s) => Some(s),
        _ => None,
    }
}

fn by_address() -> Vec<SortKey> {
    vec![SortKey {
        column: "Address".to_string(),
        descending: false,
    }]
}

fn addresses(page: &views::ViewPage) -> Vec<String> {
    page.rows
        .iter()
        .map(|r| normalize(text(&r[0]).expect("Address ist Text")))
        .collect()
}

fn dump() -> &'static Dump {
    static DUMP: OnceLock<Dump> = OnceLock::new();
    DUMP.get_or_init(|| {
        std::thread::spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Runtime")
                .block_on(async {
                    let cfg = CrawlConfig::default();
                    let fx =
                        crawl_fixture_from("onpage-url", "http://site.test/", cfg.clone()).await;
                    let mut filters = BTreeMap::new();
                    for f in FILTERS {
                        let mut q = ViewQuery::new(TAB, f);
                        q.limit = 500;
                        q.sort = by_address();
                        q.columns = ColumnSelection::Named(vec!["Address".to_string()]);
                        let page = views::query_view(&fx.file, &cfg, &q).expect("Ansicht");
                        assert_eq!(page.total as usize, page.rows.len(), "{f}");
                        filters.insert(f.to_string(), addresses(&page));
                    }
                    let mut q = ViewQuery::new(TAB, "All");
                    q.limit = 500;
                    q.sort = by_address();
                    q.columns = ColumnSelection::All;
                    let all = views::query_view(&fx.file, &cfg, &q).expect("Ansicht All");
                    let default_columns =
                        views::query_view(&fx.file, &cfg, &ViewQuery::new(TAB, "All"))
                            .expect("Standardspalten")
                            .columns;

                    let mut alt = cfg.clone();
                    alt.url.max_chars = ALT_MAX;
                    let mut q = ViewQuery::new(TAB, OVER);
                    q.limit = 500;
                    q.sort = by_address();
                    q.columns = ColumnSelection::Named(vec!["Address".to_string()]);
                    let over_alt = addresses(&views::query_view(&fx.file, &alt, &q).expect("alt"));
                    let label = |c: &CrawlConfig| {
                        views::filter_counts(&fx.file, c)
                            .expect("Zähler")
                            .into_iter()
                            .find(|f| f.tab == TAB && f.filter == OVER)
                            .expect("Filter")
                            .label
                    };
                    Dump {
                        filters,
                        columns: all.columns,
                        rows: all.rows,
                        default_columns,
                        over_alt,
                        label_default: label(&cfg),
                        label_alt: label(&alt),
                    }
                })
        })
        .join()
        .expect("Crawl-Thread")
    })
}

fn golden(filter: &str) -> Vec<String> {
    let slug = if filter == OVER {
        "over_115_characters".to_string()
    } else {
        filter
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/onpage-url")
        .join(format!("url__{slug}.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("Address"), "{}", path.display());
    lines.map(str::to_string).collect()
}

fn list(filter: &str) -> &'static [String] {
    dump()
        .filters
        .get(filter)
        .unwrap_or_else(|| panic!("Filter {filter} fehlt"))
}

fn has(filter: &str, address: &str) -> bool {
    list(filter).iter().any(|a| a == address)
}

fn cell<'a>(d: &'a Dump, address: &str, column: &str) -> &'a Value {
    let col = d
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column} fehlt in {:?}", d.columns));
    let row = d
        .rows
        .iter()
        .find(|r| text(&r[0]).map(normalize).as_deref() == Some(address))
        .unwrap_or_else(|| panic!("Zeile {address} fehlt"));
    &row[col]
}

// ---------------------------------------------------------------------------------------------------
// Direkt in die Crawl-Datei geschriebene Zeilen (Grenzfälle ohne Crawl)

fn adhoc(dir: &Path, rows: &[(&str, bool)]) -> (CrawlFile, CrawlConfig) {
    let created = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let cfg = CrawlConfig::default();
    let init = CrawlInit {
        crawl_id: SeededIds::new(7).new_crawl_id(created),
        mode: CrawlMode::Spider,
        start_url: Some("http://h.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "URL-Grenzfälle".into(),
        config: cfg.clone(),
        created_at: created,
    };
    let file =
        CrawlFile::create(&dir.join("adhoc.ospider"), &init, StorageProfile::Balanced).unwrap();
    for (i, (address, internal)) in rows.iter().enumerate() {
        file.conn()
            .execute(
                "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
                 url_length, status_code) VALUES (?1, ?2, ?3, 'http://h.test', ?4, 'done', 'link', 0, 0, 200)",
                rusqlite::params![i as i64 + 1, format!("enc:{address}"), address, i64::from(*internal)],
            )
            .unwrap();
    }
    (file, cfg)
}

fn adhoc_filter(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new(TAB, filter);
    q.limit = 500;
    q.sort = by_address();
    q.columns = ColumnSelection::Named(vec!["Address".to_string()]);
    views::query_view(file, cfg, &q)
        .unwrap()
        .rows
        .iter()
        .map(|r| text(&r[0]).unwrap().to_string())
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-070

// spec: ONPAGE-070
#[test]
fn columns_follow_the_spec_order_and_have_the_right_kinds() {
    let d = dump();
    assert_eq!(
        d.default_columns,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Indexability",
            "Indexability Status",
            "Hash",
            "Length",
            "Canonical Link Element 1",
            "URL Encoded Address"
        ]
    );
    assert_eq!(d.columns, d.default_columns, "keine weiteren Spalten");
    let tab = tabs::url::tab();
    let kind = |key: &str| tab.columns.iter().find(|c| c.key == key).unwrap().kind;
    for key in ["Address", "URL Encoded Address", "Canonical Link Element 1"] {
        assert_eq!(kind(key), ColumnKind::Url, "{key}");
    }
    assert_eq!(kind("Content"), ColumnKind::Text);
    assert_eq!(kind("Status"), ColumnKind::Text);
    assert_eq!(kind("Hash"), ColumnKind::Hash);
    assert_eq!(kind("Length"), ColumnKind::Int);
    assert!(tab.columns.iter().all(|c| !c.extension));
}

// spec: ONPAGE-070
#[test]
fn length_counts_codepoints_of_the_display_address() {
    let d = dump();
    // „http://site.test“ (16) + „/abc“-artige Pfade: ein „ü“ zählt als ein Zeichen
    let uber = "{BASE}/über-uns";
    assert_eq!(cell(d, uber, "Length"), &Value::Int(25));
    assert_eq!(cell(d, "{BASE}/ueber-uns", "Length"), &Value::Int(26));
    assert_eq!(cell(d, "{BASE}/", "Length"), &Value::Int(17));
    // die kodierte Form ist länger, ihre Länge zählt nicht
    assert_eq!(
        text(cell(d, uber, "URL Encoded Address")),
        Some("http://site.test/%C3%BCber-uns")
    );
    assert_eq!(
        text(cell(d, "{BASE}/ueber-uns", "URL Encoded Address")),
        Some("http://site.test/ueber-uns")
    );
}

// spec: ONPAGE-070
#[test]
fn length_of_a_plain_address_is_25_for_http_127_0_0_1_8080_abc() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://127.0.0.1:8080/abc", true),
            ("http://h.test/ä", true),
        ],
    );
    let mut q = ViewQuery::new(TAB, "All");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Length".into()]);
    let page = views::query_view(&file, &cfg, &q).unwrap();
    assert_eq!(page.rows[0][1], Value::Int(25));
    assert_eq!(page.rows[1][1], Value::Int(15), "ä zählt als ein Zeichen");
}

// spec: ONPAGE-070
#[test]
fn status_content_hash_and_canonical_columns_are_filled() {
    let d = dump();
    let page = "{BASE}/canon-demo";
    assert_eq!(cell(d, page, "Status Code"), &Value::Int(200));
    assert_eq!(text(cell(d, page, "Status")), Some("OK"));
    assert_eq!(
        text(cell(d, page, "Content")),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(text(cell(d, page, "Indexability")), Some("Indexable"));
    assert_eq!(cell(d, page, "Indexability Status"), &Value::Null);
    assert!(
        text(cell(d, page, "Hash")).is_some_and(|h| h.len() == 32),
        "MD5 als 32 Hexziffern"
    );
    assert_eq!(
        text(cell(d, page, "Canonical Link Element 1")),
        Some("http://site.test/Produkte")
    );
    assert_eq!(
        cell(d, "{BASE}/ueber-uns", "Canonical Link Element 1"),
        &Value::Null
    );
    // eine interne 404-Seite gehört zur Grundmenge (alle internen URLs)
    assert_eq!(cell(d, "{BASE}/Nicht_Da", "Status Code"), &Value::Int(404));
}

// spec: ONPAGE-070
#[test]
fn population_is_internal_urls_with_a_response_only() {
    assert_eq!(list("All"), golden("All"));
    assert!(
        !list("All").iter().any(|a| a.contains("other.test")),
        "externe URLs gehören nicht zum URL-Tab"
    );
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://h.test/intern", true),
            ("http://other.test/extern", false),
        ],
    );
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
             url_length) VALUES (9, 'enc:ohne-antwort', 'http://h.test/ohne-antwort', 'http://h.test', 1, \
             'queued', 'link', 0, 0)",
            [],
        )
        .unwrap();
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
             folder_depth, url_length, status_code) VALUES (10, 'enc:sitemap', 'http://h.test/sitemap.xml', \
             'http://h.test', 1, 0, 'done', 'sitemap', 0, 0, 200)",
            [],
        )
        .unwrap();
    assert_eq!(
        adhoc_filter(&file, &cfg, "All"),
        ["http://h.test/intern"],
        "extern, ohne Antwort und ohne Tabellen-Sichtbarkeit fehlen"
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-071

// spec: ONPAGE-071
#[test]
fn non_ascii_counts_decoded_and_percent_encoded_characters() {
    assert_eq!(list("Non ASCII Characters"), golden("Non ASCII Characters"));
    // /über-uns, /%C3%BCber-mich (A-22), /ÜBER und /caf%C3%A9 (Anzeige /café)
    for a in [
        "{BASE}/über-uns",
        "{BASE}/über-mich",
        "{BASE}/ÜBER",
        "{BASE}/café",
    ] {
        assert!(has("Non ASCII Characters", a), "{a}");
    }
    // /ueber-uns und /a%20b nicht: das Leerzeichen ist ASCII
    for a in ["{BASE}/ueber-uns", "{BASE}/a%20b", "{BASE}/mein%20artikel"] {
        assert!(!has("Non ASCII Characters", a), "{a}");
    }
}

// spec: ONPAGE-071
#[test]
fn non_ascii_in_idn_hosts_query_and_leftover_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://bücher.test/x", true),
            ("http://h.test/q?name=ä", true),
            ("http://h.test/caf%FF", true),
            ("http://h.test/a%C2%A0b", true),
            ("http://h.test/plain%20url", true),
            ("http://h.test/%25FF", true),
        ],
    );
    assert_eq!(
        adhoc_filter(&file, &cfg, "Non ASCII Characters"),
        [
            "http://bücher.test/x",
            "http://h.test/a%C2%A0b",
            "http://h.test/caf%FF",
            "http://h.test/q?name=ä",
        ]
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-072

// spec: ONPAGE-072
#[test]
fn underscores_count_in_the_path_only() {
    assert_eq!(list("Underscores"), golden("Underscores"));
    assert!(has("Underscores", "{BASE}/mein_artikel"));
    assert!(!has("Underscores", "{BASE}/mein-artikel"));
    // A-23: /seite?sort_by=x nicht (Unterstrich nur in der Query)
    assert!(!has("Underscores", "{BASE}/seite?sort_by=x"));
    assert!(has("Parameters", "{BASE}/seite?sort_by=x"));
    // eine 404-Seite gehört dazu
    assert!(has("Underscores", "{BASE}/Nicht_Da"));
}

// spec: ONPAGE-072
#[test]
fn underscores_ignore_host_query_and_fragment() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://my_host.test/ok", true),
            ("http://h.test/a_b?x_y=1", true),
            ("http://h.test/?utm_source=x", true),
            ("http://h.test/p#frag_ment", true),
            ("http://h.test/a/b_c/d", true),
        ],
    );
    assert_eq!(
        adhoc_filter(&file, &cfg, "Underscores"),
        ["http://h.test/a/b_c/d", "http://h.test/a_b?x_y=1"]
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-073

// spec: ONPAGE-073
#[test]
fn uppercase_finds_capitals_in_path_and_query_but_not_in_scheme_host_or_hex() {
    assert_eq!(list("Uppercase"), golden("Uppercase"));
    for a in [
        "{BASE}/Produkte",
        "{BASE}/produkte?Farbe=rot",
        "{BASE}/ÜBER",
    ] {
        assert!(has("Uppercase", a), "{a}");
    }
    // HTTP://SITE.TEST/schema-gross: Schema und Host werden normalisiert
    assert!(has("All", "{BASE}/schema-gross"));
    assert!(!has("Uppercase", "{BASE}/schema-gross"));
    // /caf%C3%A9 (Anzeige /café): Hex-Ziffern zählen nicht
    assert!(!has("Uppercase", "{BASE}/café"));
    assert!(!has("Uppercase", "{BASE}/mein-artikel"));
}

// spec: ONPAGE-073
#[test]
fn uppercase_uses_unicode_category_lu_and_ignores_escape_digits() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://h.test/Σigma", true),
            ("http://h.test/Привет", true),
            ("http://h.test/ǅ", true),
            ("http://h.test/Ⅰ", true),
            ("http://h.test/ß", true),
            ("http://h.test/ä-ö-ü-é-ÿ", true),
            ("http://h.test/Ä", true),
            ("http://h.test/Ÿ", true),
            ("http://h.test/a×b", true),
            ("http://h.test/a%C3%A9%FF%3C%7E", true),
            ("http://h.test/q?A=1", true),
            ("http://h.test/p#Frag", true),
            ("http://user:Pw@h.test/x", true),
            ("http://h.test/100%Zebra", true),
            ("http://h.test/100%Dog", true),
            ("http://h.test/%e9", true),
        ],
    );
    assert_eq!(
        adhoc_filter(&file, &cfg, "Uppercase"),
        [
            // „%Dog“ ist kein Escape (nach „%D“ folgt kein Hex-Zeichen): das „D“ zählt
            "http://h.test/100%Dog",
            // „%Z…“ ist kein Escape
            "http://h.test/100%Zebra",
            "http://h.test/p#Frag",
            "http://h.test/q?A=1",
            "http://h.test/Ä",
            "http://h.test/Ÿ",
            "http://h.test/Σigma",
            "http://h.test/Привет",
        ],
        "Lu: Ä, Ÿ, griechisches Sigma und Kyrillisch ja; Titlecase (ǅ), römische Ziffer (Ⅰ, Nl), ß, ×, \
lateinische Kleinbuchstaben nein"
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-074

// spec: ONPAGE-074
#[test]
fn multiple_slashes_count_in_the_path_only() {
    assert_eq!(list("Multiple Slashes"), golden("Multiple Slashes"));
    assert!(has("Multiple Slashes", "{BASE}/a//b"));
    // „//“ am Pfadanfang einer absoluten URL
    assert!(has("Multiple Slashes", "{BASE}//start"));
    // „//“ in der Query und nach dem Schema zählen nicht
    assert!(has("All", "{BASE}/a/b?next=http://x"));
    assert!(!has("Multiple Slashes", "{BASE}/a/b?next=http://x"));
    assert!(!has("Multiple Slashes", "{BASE}/"));
}

// spec: ONPAGE-074
#[test]
fn the_crawl_does_not_collapse_slashes_in_paths() {
    // Die Normalisierung darf „//“ nicht zusammenfassen, sonst wäre der Filter immer leer: beide Varianten
    // bleiben getrennte Zeilen.
    let d = dump();
    let all = &d.filters["All"];
    assert!(all.contains(&"{BASE}/a//b".to_string()));
    assert!(all.contains(&"{BASE}//start".to_string()));
    assert!(!all.contains(&"{BASE}/a/b".to_string()));
    assert!(!all.contains(&"{BASE}/start".to_string()));
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-076

// spec: ONPAGE-076
#[test]
fn space_counts_literal_spaces_and_percent_20_but_not_plus() {
    assert_eq!(list("Contains A Space"), golden("Contains A Space"));
    // href="/mein artikel" (angefragt als %20) und href="/zweiter%20artikel"
    assert!(has("Contains A Space", "{BASE}/mein%20artikel"));
    assert!(has("Contains A Space", "{BASE}/zweiter%20artikel"));
    assert!(has("Contains A Space", "{BASE}/a%20b"));
    // href="/suche?q=a+b": das Plus zählt nicht
    assert!(has("All", "{BASE}/suche?q=a+b"));
    assert!(!has("Contains A Space", "{BASE}/suche?q=a+b"));
}

// spec: ONPAGE-076
#[test]
fn space_detects_literal_spaces_in_stored_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://h.test/mein artikel", true),
            ("http://h.test/mein%20artikel", true),
            ("http://h.test/a+b", true),
            ("http://h.test/a%2520b", true),
            ("http://h.test/s?q=a%20b", true),
        ],
    );
    assert_eq!(
        adhoc_filter(&file, &cfg, "Contains A Space"),
        [
            "http://h.test/mein artikel",
            "http://h.test/mein%20artikel",
            "http://h.test/s?q=a%20b"
        ]
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-078

// spec: ONPAGE-078
#[test]
fn parameters_list_question_marks_and_ampersands_outside_the_fragment() {
    assert_eq!(list("Parameters"), golden("Parameters"));
    assert!(has("Parameters", "{BASE}/p?id=1"));
    // A-27: die leere Query „?“ zählt
    assert!(has("Parameters", "{BASE}/p?"));
    // Ampersand im Pfad
    assert!(has("Parameters", "{BASE}/a&b"));
    // /p#x?y: das Fragment wird nicht gespeichert, übrig bleibt /p ohne Parameter
    assert!(has("All", "{BASE}/p"));
    assert!(!has("Parameters", "{BASE}/p"));
}

// spec: ONPAGE-078
#[test]
fn parameters_ignore_question_marks_in_the_fragment() {
    let dir = tempfile::tempdir().unwrap();
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            ("http://h.test/p#x?y", true),
            ("http://h.test/p#a&b", true),
            ("http://h.test/p?a=1#frag", true),
            ("http://h.test/plain", true),
        ],
    );
    assert_eq!(
        adhoc_filter(&file, &cfg, "Parameters"),
        ["http://h.test/p?a=1#frag"]
    );
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-081

// spec: ONPAGE-081
#[test]
fn over_115_characters_lists_urls_longer_than_the_default_threshold() {
    let d = dump();
    assert_eq!(list(OVER), golden(OVER));
    // „http://site.test“ (16) + 99 Zeichen Pfad = 115 → nicht; + 100 Zeichen = 116 → ja
    let c99 = format!("{{BASE}}/{}", "d".repeat(98));
    let c100 = format!("{{BASE}}/{}", "c".repeat(99));
    assert_eq!(cell(d, &c99, "Length"), &Value::Int(115));
    assert_eq!(cell(d, &c100, "Length"), &Value::Int(116));
    assert!(!has(OVER, &c99));
    assert!(has(OVER, &c100));
    assert_eq!(d.label_default, "Over 115 Characters");
}

// spec: ONPAGE-081
#[test]
fn over_x_characters_follows_the_threshold_without_recrawl() {
    let d = dump();
    // Pfade mit 40 und 41 Zeichen: Adresse 56 bzw. 57 Zeichen; Schwelle 56 → nur der 41er
    let p40 = format!("{{BASE}}/{}", "a".repeat(39));
    let p41 = format!("{{BASE}}/{}", "b".repeat(40));
    assert_eq!(cell(d, &p40, "Length"), &Value::Int(56));
    assert_eq!(cell(d, &p41, "Length"), &Value::Int(57));
    assert!(d.over_alt.contains(&p41));
    assert!(!d.over_alt.contains(&p40));
    // alle Treffer sind länger als die Schwelle, alle längeren URLs sind Treffer
    let longer: Vec<String> = d.filters["All"]
        .iter()
        .filter(|a| a.replace("{BASE}", BASE).chars().count() > ALT_MAX as usize)
        .cloned()
        .collect();
    assert_eq!(d.over_alt, longer);
    // Treffer und Filtername ändern sich mit der Konfiguration, ohne den Crawl zu wiederholen
    assert_ne!(d.over_alt, list(OVER));
    assert_eq!(d.label_alt, "Over 56 Characters");
}

// spec: ONPAGE-081
#[test]
fn exactly_115_characters_is_not_over_and_116_is() {
    let dir = tempfile::tempdir().unwrap();
    let u115 = format!("http://h.test/{}", "x".repeat(101));
    let u116 = format!("http://h.test/{}", "x".repeat(102));
    let umlaut116 = format!("http://h.test/{}", "ä".repeat(102));
    let umlaut115 = format!("http://h.test/{}", "ä".repeat(101));
    assert_eq!(u115.chars().count(), 115);
    assert!(umlaut116.len() > 116, "Bytes zählen nicht");
    let (file, cfg) = adhoc(
        dir.path(),
        &[
            (&u115, true),
            (&u116, true),
            (&umlaut115, true),
            (&umlaut116, true),
            ("http://h.test/extern", false),
        ],
    );
    let mut expected = vec![u116.clone(), umlaut116.clone()];
    expected.sort();
    assert_eq!(adhoc_filter(&file, &cfg, OVER), expected);
}

// ---------------------------------------------------------------------------------------------------
// Rahmen

// spec: ONPAGE-070
#[test]
fn every_filter_is_a_subset_of_all() {
    let all = list("All");
    for f in FILTERS {
        for a in list(f) {
            assert!(all.contains(a), "{f}: {a} fehlt in All");
        }
    }
    let ids: Vec<_> = tabs::url::tab().filters.iter().map(|f| f.id).collect();
    assert_eq!(ids, FILTERS);
}
