//! Leseschnittstelle der Crawl-Datei im Zusammenspiel mit den Exporten (OUT-233, OUT-234) gegen `out-basic-site`:
//! `q_urls` ↔ `Internal:All`, `q_links` ↔ `Links:All Inlinks`, `user_agent` ↔ Request-Log, Golden aller `q_`-Sichten für
//! eine neu geschriebene und eine aus Schema v2 migrierte Datei, Beispielleser in Python (`scripts/ci/read_api_smoke.py`,
//! `sqlite3` im Modus `ro`, ohne OpenSpider-Code) für eine abgeschlossene Datei und neben einem laufenden Crawl.
//!
//! Die v2-Datei `tests/golden/out-m2-read-api/out-basic-site.v2.ospider` ist ein echter M1-Crawl derselben Site (mit dem
//! Stand `2f38c4a` vor Schema v3 geschrieben) und wird nicht neu erzeugt.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{read_csv, resolve, site, Env};
use os_model::Timestamp;
use os_store::{CrawlFile, OpenMode};
use os_testsite::golden::{assert_golden, normalize};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden_dir() -> PathBuf {
    repo().join("tests/golden/out-m2-read-api")
}

/// Crawl von `out-basic-site` über die CLI mit den beiden Exporten; liefert die Crawl-Datei.
async fn crawl_with_exports(env: &Env, site: &os_testsite::TestSite) -> PathBuf {
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            "http://site.test/".into(),
            "--resolve".into(),
            resolve(site),
            "--set".into(),
            "threads=1".into(),
            "--set".into(),
            "extraction.http_headers=true".into(),
            "--set".into(),
            "robots.retries=0".into(),
            "--export-tabs".into(),
            "Internal:All".into(),
            "--bulk-export".into(),
            "Links:All Inlinks".into(),
            "--output-folder".into(),
            env.out().to_string_lossy().into_owned(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    env.crawl_files().remove(0)
}

/// Jede Zeile einer Abfrage als Liste von Texten (`NULL` für fehlende Werte).
fn rows(file: &CrawlFile, sql: &str, columns: usize) -> Vec<Vec<String>> {
    let mut stmt = file.conn().prepare(sql).unwrap();
    stmt.query_map([], |r| {
        (0..columns)
            .map(|i| {
                r.get::<_, Option<String>>(i)
                    .map(|v| v.unwrap_or_else(|| "NULL".into()))
            })
            .collect::<Result<Vec<_>, _>>()
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

fn instant_s(text: &str) -> i64 {
    Timestamp::from_db(text)
        .unwrap_or_else(|e| panic!("{text}: {e}"))
        .unix_millis()
        .div_euclid(1000)
}

// spec: OUT-234
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn q_urls_equals_the_internal_all_export_in_every_required_column() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let path = crawl_with_exports(&env, &site).await;
    let csv = read_csv(&env.out().join("internal_all.csv"));
    let header = &csv[0];
    let at = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("Spalte {name} im Export"))
    };

    // Kanonischer Spaltenname des Exports ↔ Spalte von q_urls (snake_case, Einheiten als Suffix).
    let mapping: &[(&str, &str)] = &[
        ("Address", "address"),
        ("Content", "content_type"),
        ("Status Code", "status_code"),
        ("Status", "status"),
        ("Indexability", "indexability"),
        ("Indexability Status", "indexability_status"),
        ("Title 1", "title_1"),
        ("Meta Description 1", "meta_description_1"),
        ("H1-1", "h1_1"),
        ("H2-1", "h2_1"),
        ("Canonical Link Element 1", "canonical_link_element_1"),
        ("Word Count", "word_count"),
        ("Size (bytes)", "size_bytes"),
        ("Transferred (bytes)", "transferred_bytes"),
        ("Response Time", "response_time"),
        ("Crawl Depth", "crawl_depth"),
        ("Folder Depth", "folder_depth"),
        ("Inlinks", "inlinks"),
        ("Unique Inlinks", "unique_inlinks"),
        ("Outlinks", "outlinks"),
        ("Unique Outlinks", "unique_outlinks"),
        ("External Outlinks", "external_outlinks"),
        ("Redirect URL", "redirect_url"),
        ("Redirect Type", "redirect_type"),
        ("HTTP Version", "http_version"),
        ("Hash", "hash"),
        ("Last-Modified", "last_modified"),
        ("Crawl Timestamp", "crawl_timestamp"),
    ];

    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let cols: Vec<&str> = mapping.iter().map(|(_, q)| *q).collect();
    let select = cols
        .iter()
        .map(|c| format!("CAST({c} AS TEXT)"))
        .collect::<Vec<_>>()
        .join(", ");
    let q: BTreeMap<String, Vec<String>> = rows(
        &file,
        &format!("SELECT {select} FROM q_urls WHERE is_internal = 1"),
        cols.len(),
    )
    .into_iter()
    .map(|r| (r[0].clone(), r))
    .collect();

    let csv_rows = &csv[1..];
    assert_eq!(q.len(), csv_rows.len(), "gleiche Zeilenmenge");
    assert!(q.len() >= 15, "{} Zeilen", q.len());
    for row in csv_rows {
        let address = &row[at("Address")];
        let got = q
            .get(address)
            .unwrap_or_else(|| panic!("{address} fehlt in q_urls"));
        for (i, (csv_col, q_col)) in mapping.iter().enumerate() {
            let want = row[at(csv_col)].as_str();
            let have = got[i].as_str();
            match *q_col {
                // Sekunden mit drei Nachkommastellen
                "response_time" => {
                    let formatted = if have == "NULL" {
                        String::new()
                    } else {
                        format!("{:.3}", have.parse::<f64>().unwrap())
                    };
                    assert_eq!(formatted, want, "{address}: {q_col}");
                }
                // Zeitpunkte: ISO 8601 mit Offset; der Export trägt den lokalen, die Sicht den UTC-Offset
                "crawl_timestamp" => {
                    assert_eq!(instant_s(have), instant_s(want), "{address}: {q_col}")
                }
                // Der Export zeigt den Header roh, die Sicht den Zeitpunkt
                "last_modified" => {
                    assert_eq!(want.is_empty(), have == "NULL", "{address}: {q_col}")
                }
                _ => {
                    let have = if have == "NULL" { "" } else { have };
                    assert_eq!(have, want, "{address}: {q_col}");
                }
            }
        }
    }
}

// spec: OUT-234
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn q_links_equals_the_all_inlinks_export() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let path = crawl_with_exports(&env, &site).await;
    let csv = read_csv(&env.out().join("links_all_inlinks.csv"));
    let header = &csv[0];
    let at = |name: &str| header.iter().position(|h| h == name).unwrap();

    let mut expected: Vec<Vec<String>> = csv[1..]
        .iter()
        .map(|r| {
            vec![
                r[at("Type")].clone(),
                r[at("Source")].clone(),
                r[at("Destination")].clone(),
                r[at("Alt Text")].clone(),
                r[at("Anchor")].clone(),
                r[at("Status Code")].clone(),
                // Export: True/False; Sicht: 1/0
                (r[at("Follow")] == "True").to_string(),
                r[at("Target")].clone(),
                r[at("Rel")].clone(),
                r[at("Path Type")].clone(),
                r[at("Link Path")].clone(),
                r[at("Link Position")].clone(),
                r[at("Link Origin")].clone(),
            ]
        })
        .collect();
    expected.sort();

    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let mut actual: Vec<Vec<String>> = rows(
        &file,
        "SELECT type, source_address, target_address, alt_text, anchor, CAST(target_status_code AS TEXT), \
         CAST(follow AS TEXT), target, rel, path_type, link_path, link_position, link_origin FROM q_links",
        13,
    )
    .into_iter()
    .map(|mut r| {
        for cell in r.iter_mut() {
            if cell == "NULL" {
                cell.clear();
            }
        }
        r[6] = (r[6] == "1").to_string();
        r
    })
    .collect();
    actual.sort();
    assert!(actual.len() >= 30, "{} Links", actual.len());
    assert_eq!(
        actual, expected,
        "q_links = All Inlinks (Zeilenmenge und Werte)"
    );
}

// spec: OUT-233
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_manifest_user_agent_is_the_one_the_testsite_logged_for_the_start_url() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let path = crawl_with_exports(&env, &site).await;
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let manifest: BTreeMap<String, Option<String>> = file.manifest().unwrap().into_iter().collect();

    let start = site
        .requests()
        .into_iter()
        .find(|r| r.path() == "/" && r.method == "GET")
        .expect("Abruf der Start-URL im Request-Log");
    let logged = start.header_values("user-agent");
    assert_eq!(logged.len(), 1);
    assert_eq!(manifest["user_agent"].as_deref(), Some(logged[0]));
    assert_eq!(
        manifest["user_agent"].as_deref(),
        Some(os_fetch::user_agents::default_user_agent().as_str())
    );

    // Weitere Werte des Manifests eines CLI-Crawls.
    assert_eq!(manifest["writer_kind"].as_deref(), Some("cli"));
    assert_eq!(manifest["mode"].as_deref(), Some("spider"));
    assert_eq!(manifest["start_url"].as_deref(), Some("http://site.test/"));
    assert_eq!(manifest["state"].as_deref(), Some("Completed"));
    assert_eq!(manifest["completeness"].as_deref(), Some("complete"));
    assert_eq!(manifest["end_reason"].as_deref(), Some("queue_empty"));
    assert_eq!(manifest["analysis_state"].as_deref(), Some("done"));
    assert_eq!(
        manifest["openspider_version"].as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(manifest["ended_at"].is_some());
}

/// Textausgabe aller `q_`-Sichten; flüchtige Werte (Zeiten, Hash, Versionen, Konfigurationstext) sind maskiert.
fn dump_views(file: &CrawlFile) -> String {
    let conn = file.conn();
    let views: [(&str, &str, &[&str]); 6] = [
        ("q_manifest", "ORDER BY key", &[]),
        ("q_config", "", &[]),
        (
            "q_urls",
            "ORDER BY address",
            &[
                "response_time",
                "crawl_timestamp",
                "hash",
                "last_modified",
                "url_id",
            ],
        ),
        (
            "q_links",
            "ORDER BY source_address, target_address, link_path, type",
            &["source_url_id", "target_url_id"],
        ),
        (
            "q_sitemap_urls",
            "ORDER BY sitemap_url, address",
            &["url_id"],
        ),
        ("q_columns", "", &[]),
    ];
    let mut out = String::new();
    for (view, order, masked) in views {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({view})"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        out.push_str(&format!("## {view}\n{}\n", columns.join("|")));
        let select = columns
            .iter()
            .map(|c| format!("IFNULL(CAST(\"{c}\" AS TEXT), 'NULL')"))
            .collect::<Vec<_>>()
            .join(", ");
        for mut row in rows(
            file,
            &format!("SELECT {select} FROM {view} {order}"),
            columns.len(),
        ) {
            for (i, c) in columns.iter().enumerate() {
                let secret = match (view, c.as_str()) {
                    ("q_manifest", _) => false,
                    ("q_config", "osconfig") => true,
                    _ => masked.contains(&c.as_str()),
                };
                if secret {
                    row[i] = "<VAR>".into();
                }
            }
            if view == "q_manifest" {
                match row[0].as_str() {
                    "openspider_version" | "last_writer_version" => row[1] = "<VERSION>".into(),
                    "config_sha256" => row[1] = "<SHA256>".into(),
                    "user_agent" => row[1] = row[1].replace(env!("CARGO_PKG_VERSION"), "<VERSION>"),
                    _ => {}
                }
            }
            out.push_str(&row.join("|"));
            out.push('\n');
        }
    }
    normalize(&mask_loopback_base(&out), &[])
}

/// Die Seiten der Testsite tragen ihre Basis-URL (`http://127.0.0.1:<port>`) in Canonical-Elementen und Texten;
/// der Port unterscheidet die Läufe. Ersetzt jede solche Angabe durch `{BASE}`.
fn mask_loopback_base(text: &str) -> String {
    const PREFIX: &str = "http://127.0.0.1:";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(PREFIX) {
        out.push_str(&rest[..at]);
        out.push_str("{BASE}");
        let after = &rest[at + PREFIX.len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

// spec: OUT-234
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_golden_of_all_views_is_identical_for_a_new_crawl_and_a_file_migrated_from_schema_v2() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let path = crawl_with_exports(&env, &site).await;
    let fresh = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let fresh_dump = dump_views(&fresh);

    // Die v2-Datei ist ein echter M1-Crawl der Site; Öffnen zum Schreiben migriert sie (Sicherungskopie daneben).
    let work = tempfile::tempdir().unwrap();
    let v2 = work.path().join("out-basic-site.v2.ospider");
    std::fs::copy(golden_dir().join("out-basic-site.v2.ospider"), &v2).unwrap();
    {
        let conn_version: i64 = {
            let f = CrawlFile::open(&v2, OpenMode::ReadOnly);
            // Nur-Lesen weist eine ältere Datei ab; die Schemaversion der Datei steht im Fehler.
            match f {
                Err(os_store::StoreError::MigrationRequired { found, .. }) => i64::from(found),
                other => panic!("v2-Datei erwartet: {:?}", other.err()),
            }
        };
        assert_eq!(conn_version, 2);
    }
    let migrated = CrawlFile::open(&v2, OpenMode::ReadWrite).unwrap();
    let migrated_manifest: BTreeMap<String, Option<String>> =
        migrated.manifest().unwrap().into_iter().collect();
    assert_eq!(
        migrated_manifest["openspider_version"], None,
        "unbekannt = NULL"
    );
    assert_eq!(
        migrated_manifest["schema_version"].as_deref(),
        Some(os_store::SCHEMA_VERSION.to_string().as_str())
    );
    let migrated_dump = dump_views(&migrated);

    assert_eq!(
        fresh_dump, migrated_dump,
        "neue und migrierte Datei zeigen dieselben Sichten"
    );
    assert_golden(golden_dir().join("out-basic-site.q.txt"), &fresh_dump);
}

// --- Beispielleser in Python (Standardbibliothek, kein OpenSpider-Code) -----------------------------------------

/// Der Python-Interpreter; `None`, wenn keiner da ist und die CI ihn nicht verlangt.
fn python() -> Option<String> {
    for name in ["python3", "python"] {
        let ok = Command::new(name)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            return Some(name.to_string());
        }
    }
    assert!(
        std::env::var_os("OPENSPIDER_REQUIRE_PYTHON").is_none(),
        "Python 3 fehlt, die CI verlangt den Beispielleser (OPENSPIDER_REQUIRE_PYTHON)"
    );
    eprintln!("SKIP: Python 3 nicht gefunden: Beispielleser-Test übersprungen");
    None
}

fn smoke_script() -> PathBuf {
    repo().join("scripts/ci/read_api_smoke.py")
}

fn run_reader(python: &str, file: &Path, extra: &[&str]) -> (i32, String, String) {
    let out = Command::new(python)
        // `-I`: kein Suchpfad aus dem Arbeitsverzeichnis, keine Umgebungsvariablen von Python
        .arg("-I")
        .arg(smoke_script())
        .arg(file)
        .args(extra)
        .output()
        .expect("Python startbar");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// spec: OUT-234
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_reader_reads_a_finished_file_and_rejects_an_unknown_read_api_version() {
    let Some(python) = python() else { return };
    let site = site("out-basic-site").await;
    let env = Env::new();
    let path = crawl_with_exports(&env, &site).await;

    // Die abgeschlossene Datei hat keine WAL/SHM-Begleitdateien mehr. Für diesen
    // unveränderlichen Snapshot erlaubt immutable auch System-SQLite den Nur-Lese-Zugriff.
    // Normales mode=ro neben einem Schreiber prüft der nächste Test ausdrücklich.
    let (code, stdout, stderr) = run_reader(&python, &path, &["--immutable"]);
    assert_eq!(code, 0, "{stdout}\n{stderr}");
    assert!(stdout.starts_with("ok:"), "{stdout}");

    // schreibgeschütztes Verzeichnis: `immutable=1`
    let frozen = tempfile::tempdir().unwrap();
    let copy = frozen.path().join("frozen.ospider");
    CrawlFile::open(&path, OpenMode::ReadOnly)
        .unwrap()
        .save_as(&copy)
        .unwrap();
    let (code, stdout, stderr) = run_reader(&python, &copy, &["--immutable"]);
    assert_eq!(code, 0, "{stdout}\n{stderr}");

    // Eine Datei mit read_api_version = 99: der Leser fällt mit der klaren Meldung aus.
    let future = frozen.path().join("future.ospider");
    CrawlFile::open(&path, OpenMode::ReadOnly)
        .unwrap()
        .save_as(&future)
        .unwrap();
    {
        let f = CrawlFile::open(&future, OpenMode::ReadWrite).unwrap();
        f.conn()
            .execute_batch(
                "DROP VIEW q_manifest; \
                 CREATE VIEW q_manifest AS \
                 SELECT 'format' AS key, 'ospider' AS value \
                 UNION ALL SELECT 'read_api_version', '99';",
            )
            .unwrap();
    }
    let (code, _, stderr) = run_reader(&python, &future, &["--immutable"]);
    assert_eq!(code, 3, "{stderr}");
    assert!(
        stderr.contains("unbekannte Version der Leseschnittstelle"),
        "{stderr}"
    );

    // Keine OpenSpider-Datei: klarer Fehler statt Absturz.
    let foreign = frozen.path().join("fremd.ospider");
    std::fs::write(&foreign, b"kein sqlite").unwrap();
    let (code, _, _) = run_reader(&python, &foreign, &[]);
    assert_eq!(code, 1);
}

// spec: OUT-234
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_python_reader_reads_the_file_next_to_a_running_crawl_without_disturbing_it() {
    let Some(python) = python() else { return };
    let site = site("out-queue").await;
    let env = Env::new();
    // Der Crawl dauert einige Sekunden (jede Unterseite antwortet mit 400 ms Verzögerung).
    let mut crawl = env
        .command()
        .args([
            "--crawl",
            "http://site.test/",
            "--resolve",
            &resolve(&site),
            "--set",
            "threads=2",
            "--set",
            "robots.retries=0",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("openspider startbar");

    // Warten, bis die Datei mit den Sichten dasteht und der Crawl läuft.
    let started = Instant::now();
    let path = loop {
        if let Some(p) = env.crawl_files().into_iter().next() {
            let ready = CrawlFile::open(&p, OpenMode::ReadOnly)
                .ok()
                .and_then(|f| f.manifest().ok())
                .is_some_and(|m| m.iter().any(|(k, v)| k == "state" && v.is_some()));
            if ready {
                break p;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "Crawl-Datei fehlt"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    let reader_python = python.clone();
    let reader_path = path.clone();
    let reader = tokio::task::spawn_blocking(move || {
        run_reader(
            &reader_python,
            &reader_path,
            &["--repeat", "12", "--pause-ms", "100"],
        )
    });
    let (code, stdout, stderr) = reader.await.unwrap();
    assert_eq!(code, 0, "Leser neben dem Crawl: {stdout}\n{stderr}");

    // Der Crawl ist unbeeinträchtigt: er läuft zu Ende und endet mit Exit-Code 0.
    let status = tokio::task::spawn_blocking(move || crawl.wait().unwrap())
        .await
        .unwrap();
    assert!(status.success(), "{status:?}");
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let manifest: BTreeMap<String, Option<String>> = file.manifest().unwrap().into_iter().collect();
    assert_eq!(manifest["completeness"].as_deref(), Some("complete"));
    let crawled = rows(
        &file,
        "SELECT CAST(COUNT(*) AS TEXT) FROM q_urls WHERE crawled = 1",
        1,
    );
    assert_eq!(
        manifest["urls_crawled"].as_deref(),
        Some(crawled[0][0].as_str())
    );
    assert!(crawled[0][0].parse::<u32>().unwrap() >= 20);
}
