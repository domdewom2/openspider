//! Gemeinsame Hilfen der Export-Tests von P16 (per `#[path]` eingebunden; dieses Ziel hat keine eigenen Tests).
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use os_analyze::{ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, SortKey, TabDef};
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, StoreCrawl, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

// ---------------------------------------------------------------------------------------------------
// Dateien und Golden-Vergleich

/// Wurzel der Repository-Testdaten (`tests/`).
pub fn repo_tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests")
}

/// Liest eine CSV-Datei als Text: ohne BOM, Zeilenende `\n`.
pub fn read_csv(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let text = String::from_utf8(bytes).expect("UTF-8");
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    text.replace("\r\n", "\n")
}

/// Zerlegt CSV-Text in Zeilen und Zellen.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    rdr.records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

/// Vergleicht `actual` mit der Golden-Datei `tests/golden/<rel>`. Mit `UPDATE_GOLDEN=1` wird sie neu
/// geschrieben.
pub fn assert_golden(rel: &str, actual: &str) {
    let path = repo_tests_dir().join("golden").join(rel);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Golden {rel} fehlt ({e}); mit UPDATE_GOLDEN=1 erzeugen"));
    assert_eq!(actual, expected, "Golden {rel} weicht ab");
}

// ---------------------------------------------------------------------------------------------------
// Echte Crawls der Fixtures

/// Konfiguration der Link-Fixture: AMP-Links werden gespeichert (Standard: aus).
pub fn links_config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.spider.amp = StoreCrawl::ON;
    cfg
}

/// Crawlt `out-links` ab `http://www.site.test/`.
pub async fn crawl_links() -> FixtureCrawl {
    crawl_fixture_from("out-links", "http://www.site.test/", links_config()).await
}

/// Crawlt `out-response-codes` ab `http://www.site.test/`.
pub async fn crawl_codes() -> FixtureCrawl {
    crawl_fixture_from(
        "out-response-codes",
        "http://www.site.test/",
        CrawlConfig::default(),
    )
    .await
}

// ---------------------------------------------------------------------------------------------------
// Synthetische Crawl-Dateien (ohne Engine)

/// Legt eine leere Crawl-Datei an.
pub fn new_file(dir: &Path, name: &str) -> CrawlFile {
    let created = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(7).new_crawl_id(created),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "Exporttest".into(),
        config: CrawlConfig::default(),
        created_at: created,
    };
    CrawlFile::create(&dir.join(name), &init, StorageProfile::Balanced).unwrap()
}

/// SQL-Textliteral.
pub fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Führt SQL aus (Testdaten).
pub fn exec(file: &CrawlFile, sql: &str) {
    file.conn()
        .execute_batch(sql)
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

/// Fügt eine URL-Zeile ein. `cols` sind weitere Spalten mit fertigen SQL-Literalen (`("status_code", "404")`).
pub fn insert_url(file: &CrawlFile, id: i64, address: &str, internal: bool, cols: &[(&str, &str)]) {
    let mut names = vec![
        "id",
        "url",
        "address",
        "origin",
        "is_internal",
        "state",
        "discovered_via",
        "folder_depth",
        "url_length",
    ];
    let mut values = vec![
        id.to_string(),
        lit(address),
        lit(address),
        lit("https://site.test"),
        i64::from(internal).to_string(),
        lit("done"),
        lit("link"),
        "0".to_string(),
        address.chars().count().to_string(),
    ];
    for (c, v) in cols {
        names.push(c);
        values.push((*v).to_string());
    }
    exec(
        file,
        &format!(
            "INSERT INTO urls ({}) VALUES ({})",
            names.join(", "),
            values.join(", ")
        ),
    );
}

/// Fügt eine Link-Zeile ein (`target_id` und weitere Felder als SQL-Literale).
#[allow(clippy::too_many_arguments)]
pub fn insert_link(
    file: &CrawlFile,
    id: i64,
    source: i64,
    target: Option<i64>,
    target_raw: &str,
    link_type: &str,
    position: i64,
    cols: &[(&str, &str)],
) {
    let mut names = vec![
        "id",
        "source_id",
        "target_id",
        "target_raw",
        "link_type",
        "position",
        "follow",
    ];
    let mut values = vec![
        id.to_string(),
        source.to_string(),
        target.map_or("NULL".to_string(), |t| t.to_string()),
        lit(target_raw),
        lit(link_type),
        position.to_string(),
        "1".to_string(),
    ];
    for (c, v) in cols {
        if let Some(i) = names.iter().position(|n| n == c) {
            values[i] = (*v).to_string();
        } else {
            names.push(c);
            values.push((*v).to_string());
        }
    }
    exec(
        file,
        &format!(
            "INSERT INTO links ({}) VALUES ({})",
            names.join(", "),
            values.join(", ")
        ),
    );
}

// ---------------------------------------------------------------------------------------------------
// Test-Tabs

fn col(key: &'static str, sql: &'static str, kind: ColumnKind, visible: bool) -> ColumnDef {
    ColumnDef {
        key,
        sql: ColumnSql::Expr(sql),
        kind,
        default_visible: visible,
        extension: false,
        ..ColumnDef::META
    }
}

fn filter(id: &'static str, predicate: &'static str) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: Vec::new(),
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

/// Test-Tab „Page Titles“ mit sichtbaren und ausgeblendeten Spalten, einer Erweiterungsspalte **vor** einer
/// ausgeblendeten Referenzspalte, einer Mehrfachspalte und einem Nachlauf-Filter.
pub fn page_titles_tab() -> TabDef {
    TabDef {
        id: "Page Titles",
        population: "u.is_internal = 1",
        columns: vec![
            col("Address", "u.address", ColumnKind::Url, true),
            col("Title 1", "u.title_1", ColumnKind::Text, true),
            col("Status Code", "u.status_code", ColumnKind::Int, true),
            col("Word Count", "u.word_count", ColumnKind::Int, true),
            ColumnDef {
                key: "Ext Tiefe",
                sql: ColumnSql::Expr("u.crawl_depth"),
                kind: ColumnKind::Int,
                default_visible: false,
                extension: true,
                ..ColumnDef::META
            },
            col(
                "Title 1 Pixel Width",
                "u.title_1_px",
                ColumnKind::Int,
                false,
            ),
            col(
                "Response Time",
                "u.response_time_ms",
                ColumnKind::Seconds3,
                false,
            ),
            col(
                "Text Ratio",
                "u.text_ratio",
                ColumnKind::Float { decimals: 2 },
                false,
            ),
            col("Hash", "u.hash_md5", ColumnKind::Hash, false),
            ColumnDef {
                key: "Canonical Link Element",
                sql: ColumnSql::Multi {
                    base: "Canonical Link Element",
                    list_sql: "SELECT raw FROM canonicals WHERE url_id = ?1 ORDER BY position",
                },
                kind: ColumnKind::Text,
                default_visible: false,
                extension: false,
                ..ColumnDef::META
            },
        ],
        filters: vec![
            filter("All", "1"),
            filter("Missing", "(u.title_1 IS NULL OR u.title_1 = '')"),
            filter("Over {title.max_chars} Characters", "u.title_1_len > :max"),
            filter("Client Error (4XX)", "u.status_code BETWEEN 400 AND 499"),
            FilterDef {
                kind: FilterKind::PostCrawl(os_model::AnalysisStep::Canonicals),
                ..filter("Needs Analysis", "u.canonical_count > 0")
            },
        ],
        default_sort: Vec::new(),
        params: |cfg| vec![("max", Value::Int(i64::from(cfg.title.max_chars)))],
        hidden_population: None,
    }
}

pub fn sort(column: &str, descending: bool) -> SortKey {
    SortKey {
        column: column.to_string(),
        descending,
    }
}

/// Test-Tab „Response Codes“ mit echten Prädikaten für die Filter, die die Bulk-Exporte (OUT-050)
/// auswerten – anstelle der P00b-Platzhalter, bis P21 sie liefert. „Success (2XX)“ schließt Refresh-Ziele
/// aus (Refresh-Klassifikation hat Vorrang, OUT-050).
pub fn response_codes_tab() -> TabDef {
    let scopes: [(&str, &str); 3] = [
        ("", "1"),
        ("Internal ", "u.is_internal = 1"),
        ("External ", "u.is_internal = 0"),
    ];
    // Je Bereich dieselben Klassen; Filter-IDs müssen 'static sein.
    fn leak(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }
    let mut filters = Vec::new();
    for (prefix, scope) in scopes {
        let classes: [(&str, String); 10] = [
            ("All", "1".to_string()),
            ("Blocked by Robots.txt", "u.robots_blocked = 1".to_string()),
            (
                "No Response",
                "(u.state = 'done' AND u.status_code = 0 AND u.robots_blocked = 0)".to_string(),
            ),
            (
                "Success (2XX)",
                "(u.status_code BETWEEN 200 AND 299 AND u.redirect_type IS NULL)".to_string(),
            ),
            (
                "Redirection (3XX)",
                "u.status_code BETWEEN 300 AND 399".to_string(),
            ),
            (
                "Redirection (Meta Refresh)",
                "u.redirect_type = 'meta_refresh'".to_string(),
            ),
            ("Redirect Chain", "0".to_string()),
            ("Redirect Loop", "0".to_string()),
            (
                "Client Error (4XX)",
                "u.status_code BETWEEN 400 AND 499".to_string(),
            ),
            (
                "Server Error (5XX)",
                "u.status_code BETWEEN 500 AND 599".to_string(),
            ),
        ];
        for (name, predicate) in classes {
            let id = leak(format!("{prefix}{name}"));
            let pred = if name == "All" {
                leak(scope.to_string())
            } else {
                leak(format!("({scope}) AND ({predicate})"))
            };
            filters.push(FilterDef {
                id,
                predicate: pred,
                kind: FilterKind::Live,
                requires: Vec::new(),
                ignore_non_indexable: false,
                default_columns: &[],
            });
        }
    }
    TabDef {
        id: "Response Codes",
        population: "u.in_tables = 1",
        columns: vec![
            col("Address", "u.address", ColumnKind::Url, true),
            col("Status Code", "u.status_code", ColumnKind::Int, true),
        ],
        filters,
        default_sort: Vec::new(),
        params: |_| Vec::new(),
        hidden_population: None,
    }
}

// ---------------------------------------------------------------------------------------------------
// Speichermessung (Benchmarks)

/// Resident-Set-Size des Prozesses in MiB (Unix über `ps`); `None`, wo nicht messbar.
///
/// Linux liest `/proc/self/statm`; macOS und andere Unix-Systeme fragen `ps` (nur in `#[ignore]`-Benchmarks);
/// auf Windows ist der Wert nicht messbar (`None`, der Benchmark meldet das).
pub fn rss_mib() -> Option<f64> {
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let pages: f64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        return Some(pages * 4096.0 / (1024.0 * 1024.0));
    }
    if !cfg!(unix) {
        return None;
    }
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kib: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kib / 1024.0)
}

/// Misst den höchsten RSS-Wert während `work` (Abtastung alle 25 ms) und gibt (Ergebnis, Spitze) zurück.
pub fn peak_rss_during<T>(work: impl FnOnce() -> T) -> (T, Option<f64>) {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    let stop = Arc::new(AtomicBool::new(false));
    let peak = Arc::new(AtomicU64::new(0));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Some(m) = rss_mib() {
                    peak.fetch_max((m * 1024.0) as u64, Ordering::Relaxed);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        })
    };
    let result = work();
    stop.store(true, Ordering::Relaxed);
    sampler.join().unwrap();
    let kib = peak.load(Ordering::Relaxed);
    (result, (kib > 0).then(|| kib as f64 / 1024.0))
}
