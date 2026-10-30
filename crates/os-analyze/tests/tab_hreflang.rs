//! Tab „Hreflang“ (M1-P25): Population und Spalten, Live-Filter, Codeprüfung, Konfigurationsläufe
//! (Store aus / Store an + Crawl aus / Store an + Crawl an). Fixtures: `tests/fixtures/tech/hreflang` und
//! `tests/fixtures/tech/hreflang-codes`, Goldens unter `tests/golden/tech/{hreflang,hreflang-codes}`.
//!
//! Die Erwartungen der Adresslisten stehen als explizite Assertions im Test (aus der Spec abgeleitet); die
//! Golden-Dateien halten dieselben Listen und die Filterzahlen je Lauf fest.

use std::path::{Path, PathBuf};

use os_analyze::views::{filter_counts, query_view, ColumnSelection, ViewPage, ViewQuery};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, FilterState, StoreCrawl, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::golden;

const BASE: &str = "http://site.test";

fn golden_dir(scenario: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/tech")
        .join(scenario)
}

fn text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn page(file: &CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str, all: bool) -> ViewPage {
    let mut q = ViewQuery::new(tab, filter);
    q.limit = 500;
    if all {
        q.columns = ColumnSelection::All;
    }
    query_view(file, cfg, &q).unwrap()
}

/// Adressen eines Filters ohne Host, in Tab-Reihenfolge (Address aufsteigend, bytewise).
fn list(file: &CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    page(file, cfg, tab, filter, false)
        .rows
        .iter()
        .map(|r| text(&r[0]).replace(BASE, ""))
        .collect()
}

/// Prüft die Adressliste gegen die spec-abgeleitete Erwartung und gegen die Golden-Datei.
fn check(fx: &FixtureCrawl, cfg: &CrawlConfig, golden_sub: &str, filter: &str, expected: &[&str]) {
    let got = list(&fx.file, cfg, "Hreflang", filter);
    assert_eq!(got, expected, "Hreflang:{filter}");
    let mut body = String::from("Address\n");
    for a in &got {
        body.push_str(a);
        body.push('\n');
    }
    golden::assert_golden(
        golden_dir("hreflang")
            .join(golden_sub)
            .join(format!("hreflang__{}.csv", slug(filter))),
        &body,
    );
}

/// Filterzahlen des Tabs als Text (eine Zeile je Filter).
fn counts(file: &CrawlFile, cfg: &CrawlConfig) -> String {
    let mut out = String::new();
    for c in filter_counts(file, cfg)
        .unwrap()
        .into_iter()
        .filter(|c| c.tab == "Hreflang")
    {
        let value = match (&c.state, c.count) {
            (FilterState::Ready, Some(n)) => n.to_string(),
            (state, _) => state.as_str().to_string(),
        };
        out.push_str(&format!("{}: {value}\n", c.filter));
    }
    out
}

fn cfg_run(store: bool, crawl: bool, sitemap: bool) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.spider.hreflang = StoreCrawl { store, crawl };
    if sitemap {
        cfg.sitemaps.urls = vec![format!("{BASE}/sitemap-hreflang.xml")];
    }
    cfg
}

async fn run(cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from("tech/hreflang", &format!("{BASE}/"), cfg).await
}

fn cell(page: &ViewPage, address: &str, column: &str) -> String {
    let col = page
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column:?} fehlt in {:?}", page.columns));
    let row = page
        .rows
        .iter()
        .find(|r| text(&r[0]) == format!("{BASE}{address}"))
        .unwrap_or_else(|| panic!("Zeile {address:?} fehlt"));
    text(&row[col])
}

// spec: TECH-071, TECH-070
#[tokio::test]
async fn population_columns_and_occurrences_over_all_sources() {
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    let all = page(&fx.file, &cfg, "Hreflang", "All", true);

    // Grundmenge: interne HTML-200-Seiten, URLs mit HTTP- oder Sitemap-hreflang, gecrawlte Ziele (/it/ 404,
    // /fr-old/ 301). Sitemap-Dateien und robots.txt gehören nicht dazu.
    let addresses: Vec<String> = all
        .rows
        .iter()
        .map(|r| text(&r[0]).replace(BASE, ""))
        .collect();
    assert_eq!(
        addresses,
        [
            "/",
            "/about.html",
            "/badcode/",
            "/de/",
            "/en/",
            "/es/",
            "/fr/",
            "/nl/",
        ]
    );

    // Occurrences = alle gespeicherten Annotationen über alle drei Quellen.
    assert_eq!(cell(&all, "/en/", "Occurrences"), "7");
    assert_eq!(cell(&all, "/de/", "Occurrences"), "3");
    assert_eq!(cell(&all, "/es/", "Occurrences"), "2"); // nur Sitemap
    assert_eq!(cell(&all, "/", "Occurrences"), "0");
    // /fr/: 1 × HTML, 2 × Sitemap, davon `fr` → /fr/ identisch zur HTML-Angabe: zählt zweimal.
    assert_eq!(cell(&all, "/fr/", "Occurrences"), "3");

    // HTML-Spalten: Code in Originalschreibweise (`fr-CA`), URL aufgelöst; Quellreihenfolge.
    let en_codes: Vec<String> = (1..=7)
        .map(|n| cell(&all, "/en/", &format!("HTML hreflang {n}")))
        .collect();
    assert_eq!(en_codes, ["en", "de", "fr", "fr-CA", "es", "it", "nl"]);
    assert_eq!(
        cell(&all, "/en/", "HTML hreflang 1 URL"),
        format!("{BASE}/en/")
    );
    assert_eq!(
        cell(&all, "/en/", "HTML hreflang 4 URL"),
        format!("{BASE}/fr-old/")
    );
    assert_eq!(cell(&all, "/en/", "Title 1"), "English");
    // Reihenfolge der Spalten nach TECH-071: je Index Code, dann URL; erst HTML, dann HTTP, dann Sitemap.
    let hreflang_cols: Vec<&str> = all
        .columns
        .iter()
        .map(String::as_str)
        .filter(|c| c.contains("hreflang"))
        .collect();
    assert_eq!(
        &hreflang_cols[..4],
        [
            "HTML hreflang 1",
            "HTML hreflang 1 URL",
            "HTML hreflang 2",
            "HTML hreflang 2 URL"
        ]
    );
    let first_http = hreflang_cols
        .iter()
        .position(|c| c.starts_with("HTTP"))
        .unwrap();
    assert_eq!(hreflang_cols[first_http - 2], "HTML hreflang 7");
    assert_eq!(hreflang_cols[first_http - 1], "HTML hreflang 7 URL");
    assert_eq!(
        &hreflang_cols[first_http..first_http + 2],
        ["HTTP hreflang 1", "HTTP hreflang 1 URL"]
    );
    assert!(hreflang_cols.contains(&"Sitemap hreflang 2 URL"));
    assert!(
        !hreflang_cols.iter().any(|c| c.contains("hreflang URL")),
        "alte Form entfällt"
    );
    // HTTP- und Sitemap-Spalten.
    assert_eq!(cell(&all, "/es/", "Sitemap hreflang 1"), "es");
    assert_eq!(cell(&all, "/es/", "Sitemap hreflang 2"), "en");
    assert_eq!(
        cell(&all, "/es/", "Sitemap hreflang 2 URL"),
        format!("{BASE}/en/")
    );
    assert_eq!(cell(&all, "/en/", "Sitemap hreflang 1"), "");

    // Quellzeilen: Sitemap-Annotationen verweisen auf ihre Sitemap-Datei (TECH-070, K-41e).
    let conn = fx.file.conn();
    let (n, url): (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), MIN(s.url) FROM hreflang h JOIN sitemaps s ON s.id = h.sitemap_id \
             WHERE h.source = 'sitemap'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(n, 6, "2 + 2 + 2 Sitemap-Annotationen");
    assert_eq!(url, format!("{BASE}/sitemap-hreflang.xml"));
    let http: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM hreflang WHERE source = 'http' AND in_head = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(http, 1);

    golden::assert_golden(
        golden_dir("hreflang").join("counts_store-on_crawl-on.txt"),
        &counts(&fx.file, &cfg),
    );
}

// spec: TECH-073
#[tokio::test]
async fn contains_hreflang_includes_urls_with_sitemap_annotations_only() {
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-on",
        "Contains Hreflang",
        &["/badcode/", "/de/", "/en/", "/es/", "/fr/"],
    );
    // /es/ hat nur Sitemap-Annotationen und ist trotzdem enthalten; /nl/ hat keine.
    let all = list(&fx.file, &cfg, "Hreflang", "Contains Hreflang");
    assert!(all.contains(&"/es/".to_string()));
    assert!(!all.contains(&"/nl/".to_string()));
}

// spec: TECH-074
#[tokio::test]
async fn non_200_lists_pages_with_redirecting_or_missing_targets_and_ignores_unfetched() {
    // /en/ → /it/ (404) und → /fr-old/ (301)
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-on",
        "Non-200 Hreflang URLs",
        &["/en/"],
    );

    // Ohne Crawl hreflang, aber mit Sitemap: /it/ wird über die Sitemap abgerufen (404) → /en/ gelistet.
    let cfg = cfg_run(true, false, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off",
        "Non-200 Hreflang URLs",
        &["/en/"],
    );

    // Ohne Crawl hreflang und ohne Sitemap: /it/ und /fr-old/ wurden nie abgerufen → nichts gemeldet.
    let cfg = cfg_run(true, false, false);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off_no-sitemap",
        "Non-200 Hreflang URLs",
        &[],
    );
}

// spec: TECH-076
#[tokio::test]
async fn missing_return_links_are_live_and_accept_return_links_from_any_source() {
    // Crawl an: /nl/ wird abgerufen und verweist nicht zurück → /en/. /fr/ verlinkt /de/ nicht → /de/.
    // Rücklinks von /fr/ und /es/ an /en/ stehen nur in der Sitemap und zählen.
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-on",
        "Missing Return Links",
        &["/de/", "/en/"],
    );

    // Store an, Crawl aus: /nl/ ist nie abgerufen → /en/ erzeugt keinen Treffer wegen /nl/; /de/ (verlinkt)
    // ohne Rücklink von /fr/ schon.
    let cfg = cfg_run(true, false, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off",
        "Missing Return Links",
        &["/de/"],
    );

    // Ohne Sitemap fehlen die Rücklinks von /fr/ und /es/ an /en/ → /en/ ist wieder gelistet (Positivtest
    // des Sitemap-Rücklinks).
    let cfg = cfg_run(true, false, false);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off_no-sitemap",
        "Missing Return Links",
        &["/de/", "/en/"],
    );
}

// spec: TECH-076
#[test]
fn return_link_to_the_http_variant_is_a_missing_return_link() {
    // P = https://site.test/p/, Q = https://site.test/q/ verweist zurück auf http://site.test/p/ (andere URL,
    // die ihrerseits auf Q zurückverweist). A ↔ B verweisen korrekt aufeinander. Verglichen wird exakt
    // (URL-Kennung): nur P ist gelistet.
    let dir = tempfile::tempdir().unwrap();
    let file = synthetic_file(dir.path());
    let pages = [
        (1, "https://site.test/p/"),
        (2, "https://site.test/q/"),
        (3, "http://site.test/p/"),
        (4, "https://site.test/a/"),
        (5, "https://site.test/b/"),
    ];
    for (id, url) in pages {
        file.conn()
            .execute(
                "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
                 url_length, status_code, content_class) \
                 VALUES (?1, ?2, ?2, 'https://site.test', 1, 'done', 'link', 1, 20, 200, 'HTML')",
                rusqlite::params![id, url],
            )
            .unwrap();
    }
    let annotations = [(1, 2), (2, 3), (3, 2), (4, 5), (5, 4)];
    for (n, (from, to)) in annotations.into_iter().enumerate() {
        file.conn()
            .execute(
                "INSERT INTO hreflang (url_id, source, position, code_raw, code_norm, code_valid, href_raw, \
                 url, target_id, in_head) VALUES (?1, 'html', ?2, 'en', 'en', 1, 'x', 'x', ?3, 1)",
                rusqlite::params![from, n as i64, to],
            )
            .unwrap();
    }
    let cfg = CrawlConfig::default();
    let got = list(&file, &cfg, "Hreflang", "Missing Return Links");
    assert_eq!(got, ["https://site.test/p/"]);
}

fn synthetic_file(dir: &Path) -> CrawlFile {
    let created = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(1).new_crawl_id(created),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "hreflang".into(),
        config: CrawlConfig::default(),
        created_at: created,
    };
    CrawlFile::create(&dir.join("c.ospider"), &init, StorageProfile::Balanced).unwrap()
}

// spec: TECH-085
#[tokio::test]
async fn missing_lists_internal_html_200_pages_without_any_annotation() {
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    // /es/ hat nur Sitemap-Annotationen und ist deshalb nicht „Missing“; /fr-old/ (301) ist keine HTML-200-Seite.
    check(
        &fx,
        &cfg,
        "store-on_crawl-on",
        "Missing",
        &["/", "/about.html", "/nl/"],
    );

    // Store an, Crawl aus: /nl/ ist nicht abgerufen und fehlt in der Grundmenge.
    let cfg = cfg_run(true, false, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off",
        "Missing",
        &["/", "/about.html"],
    );
}

// spec: TECH-071, TECH-085
#[test]
fn population_and_missing_use_the_status_class_2xx() {
    // K-56 (Befund B-04): eine 201-Seite gehört zur Grundmenge und steht in Missing; eine 404-Seite und eine
    // Weiterleitung nicht. „Non-200 Hreflang URLs“ behält die wörtliche Bedingung (Status ≠ 200).
    let dir = tempfile::tempdir().unwrap();
    let file = synthetic_file(dir.path());
    for (id, url, status) in [
        (1, "https://site.test/ok", 200),
        (2, "https://site.test/created", 201),
        (3, "https://site.test/gone", 404),
        (4, "https://site.test/moved", 301),
    ] {
        file.conn()
            .execute(
                "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                 folder_depth, url_length, status_code, content_class) \
                 VALUES (?1, ?2, ?2, 'https://site.test', 1, 1, 'done', 'link', 1, 20, ?3, 'HTML')",
                rusqlite::params![id, url, status],
            )
            .unwrap();
    }
    let cfg = CrawlConfig::default();
    let mut all = list(&file, &cfg, "Hreflang", "All");
    all.sort();
    assert_eq!(all, ["https://site.test/created", "https://site.test/ok"]);
    let mut missing = list(&file, &cfg, "Hreflang", "Missing");
    missing.sort();
    assert_eq!(missing, all);
    // eine Annotation von /ok auf die 201-Seite: nicht in Non-200 Hreflang URLs (201 ≠ 200 gilt wörtlich)
    file.conn()
        .execute(
            "INSERT INTO hreflang (url_id, source, position, code_raw, code_norm, code_valid, href_raw, url, \
             target_id, in_head) VALUES (1, 'html', 0, 'en', 'en', 1, 'x', 'x', 2, 1)",
            [],
        )
        .unwrap();
    assert_eq!(
        list(&file, &cfg, "Hreflang", "Non-200 Hreflang URLs"),
        ["https://site.test/ok"]
    );
}

// spec: TECH-071
#[tokio::test]
async fn store_off_leaves_the_tab_and_all_filters_empty() {
    let cfg = cfg_run(false, false, true);
    let fx = run(cfg.clone()).await;
    for filter in [
        "All",
        "Contains Hreflang",
        "Non-200 Hreflang URLs",
        "Missing Return Links",
        "Incorrect Language & Region Codes",
        "Missing",
    ] {
        check(&fx, &cfg, "store-off", filter, &[]);
    }
    let n: i64 = fx
        .file
        .conn()
        .query_row("SELECT COUNT(*) FROM hreflang", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "nichts gespeichert");
    golden::assert_golden(
        golden_dir("hreflang").join("counts_store-off.txt"),
        &counts(&fx.file, &cfg),
    );
}

// spec: TECH-071
#[tokio::test]
async fn store_on_crawl_off_fills_the_tab_with_the_filters_that_need_only_the_own_page() {
    // OS-Verhalten (Paritätsabweichung): bereits mit „Store“ allein ist der Tab befüllt.
    let cfg = cfg_run(true, false, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-off",
        "All",
        &[
            "/",
            "/about.html",
            "/badcode/",
            "/de/",
            "/en/",
            "/es/",
            "/fr/",
        ],
    );
    check(
        &fx,
        &cfg,
        "store-on_crawl-off",
        "Incorrect Language & Region Codes",
        &["/badcode/"],
    );
    golden::assert_golden(
        golden_dir("hreflang").join("counts_store-on_crawl-off.txt"),
        &counts(&fx.file, &cfg),
    );
}

// spec: TECH-080
#[tokio::test]
async fn incorrect_codes_lists_pages_with_an_invalid_annotation() {
    let cfg = cfg_run(true, true, true);
    let fx = run(cfg.clone()).await;
    check(
        &fx,
        &cfg,
        "store-on_crawl-on",
        "Incorrect Language & Region Codes",
        &["/badcode/"],
    );
}

// --- hreflang-codes (TECH-072, TECH-080, TECH-070) ----------------------------------------------------

async fn codes_run(cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from("tech/hreflang-codes", &format!("{BASE}/"), cfg).await
}

/// `code_raw,code_valid,invalid_reason` aller Annotationen einer Seite in Quelltextreihenfolge.
fn code_rows(file: &CrawlFile, page_path: &str) -> Vec<(String, bool, String)> {
    let mut stmt = file
        .conn()
        .prepare(
            "SELECT h.code_raw, h.code_valid, COALESCE(h.invalid_reason, '') FROM hreflang h \
             JOIN urls u ON u.id = h.url_id WHERE u.url = ?1 AND h.source = 'html' ORDER BY h.position",
        )
        .unwrap();
    stmt.query_map([format!("{BASE}{page_path}")], |r| {
        Ok((r.get(0)?, r.get::<_, i64>(1)? != 0, r.get(2)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

// spec: TECH-072, TECH-070
#[tokio::test]
async fn code_validation_matches_the_examples_of_the_spec() {
    let cfg = CrawlConfig::default();
    let fx = codes_run(cfg).await;

    let valid = code_rows(&fx.file, "/valid/");
    let invalid = code_rows(&fx.file, "/invalid/");

    // Gültig (case-insensitiv): x-default, ll, ll-RR, ll-Ssss, ll-Ssss-RR; `be` ist Belarussisch, `eu` Baskisch.
    let valid_codes: Vec<&str> = valid.iter().map(|(c, _, _)| c.as_str()).collect();
    assert_eq!(
        valid_codes,
        [
            "x-default",
            "en",
            "EN-gb",
            "de",
            "pt-BR",
            "zh-Hant",
            "zh-Hant-TW",
            "X-Default",
            "be",
            "eu"
        ]
    );
    assert!(valid.iter().all(|(_, ok, reason)| *ok && reason.is_empty()));

    // Ungültig: en-UK, en_GB, eng, jp, es-419, EU (allein, groß), „de-“, x-default-de, zurückgezogene iw/in/ji.
    // Der leere hreflang-Wert ist keine Annotation (TECH-070) und fehlt hier.
    let expected = [
        ("en-UK", "invalid_region"),
        ("en_GB", "invalid_format"),
        ("eng", "invalid_language"),
        ("jp", "invalid_language"),
        ("es-419", "invalid_region"),
        ("EU", "invalid_language"),
        ("de-", "invalid_format"),
        ("x-default-de", "invalid_format"),
        ("iw", "invalid_language"),
        ("in", "invalid_language"),
        ("ji", "invalid_language"),
    ];
    let got: Vec<(&str, &str)> = invalid
        .iter()
        .map(|(c, ok, reason)| {
            assert!(!ok, "{c} muss ungültig sein");
            (c.as_str(), reason.as_str())
        })
        .collect();
    assert_eq!(got, expected);

    // Golden: code_valid je Code
    let mut body = String::from("code,valid,reason\n");
    for (code, ok, reason) in valid.iter().chain(invalid.iter()) {
        body.push_str(&format!("{code},{ok},{reason}\n"));
    }
    golden::assert_golden(golden_dir("hreflang-codes").join("code_valid.csv"), &body);
}

// spec: TECH-080, TECH-072
#[tokio::test]
async fn incorrect_codes_filter_follows_the_script_subtag_option_without_recrawl() {
    let cfg = CrawlConfig::default();
    let fx = codes_run(cfg.clone()).await;
    let filter = "Incorrect Language & Region Codes";
    assert_eq!(list(&fx.file, &cfg, "Hreflang", filter), ["/invalid/"]);

    // Script-Subtag ausschalten: `zh-Hant` und `zh-Hant-TW` auf /valid/ gelten nun als ungültig – die
    // Neuberechnung geschieht ohne Recrawl (die gespeicherten Zeilen sind mit „erlaubt“ geprüft).
    let mut off = cfg.clone();
    off.hreflang.allow_script_subtag = false;
    fx.file.set_config(&off).unwrap();
    assert_eq!(
        list(&fx.file, &off, "Hreflang", filter),
        ["/invalid/", "/valid/"]
    );
    golden::assert_golden(
        golden_dir("hreflang-codes")
            .join("hreflang__incorrect_language_region_codes_script-off.csv"),
        &format!(
            "Address\n{}\n",
            list(&fx.file, &off, "Hreflang", filter).join("\n")
        ),
    );
    // zurück
    fx.file.set_config(&cfg).unwrap();
    assert_eq!(list(&fx.file, &cfg, "Hreflang", filter), ["/invalid/"]);
}

// spec: TECH-072
#[tokio::test]
async fn script_subtag_off_at_crawl_time_marks_script_codes_invalid() {
    let mut cfg = CrawlConfig::default();
    cfg.hreflang.allow_script_subtag = false;
    let fx = codes_run(cfg.clone()).await;
    let valid = code_rows(&fx.file, "/valid/");
    let bad: Vec<&str> = valid
        .iter()
        .filter(|(_, ok, _)| !ok)
        .map(|(c, _, _)| c.as_str())
        .collect();
    assert_eq!(bad, ["zh-Hant", "zh-Hant-TW"]);
    assert_eq!(
        list(
            &fx.file,
            &cfg,
            "Hreflang",
            "Incorrect Language & Region Codes"
        ),
        ["/invalid/", "/valid/"]
    );
}
