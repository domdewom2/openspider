//! Konfiguration über `--config`, `--set` und `--max-urls` (OUT-186, CRAWL-135).

mod common;

use std::path::{Path, PathBuf};

use common::{column, read_csv, site, Env};
use os_store::{CrawlFile, OpenMode};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn stored_config(env: &Env) -> os_model::CrawlConfig {
    let files = env.crawl_files();
    assert_eq!(files.len(), 1, "{files:?}");
    CrawlFile::open(&files[0], OpenMode::ReadOnly)
        .unwrap()
        .config()
        .unwrap()
}

// spec: OUT-186
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_config_with_an_exclude_rule_keeps_the_excluded_folder_out_of_the_crawl() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let crawl = |config: Option<PathBuf>| {
        let env = Env::new();
        let mut args = vec![
            "--crawl".to_string(),
            base.clone(),
            "--set".into(),
            "robots.retries=0".into(),
            "--export-tabs".into(),
            "Internal:All".into(),
            "--output-folder".into(),
            env.out().to_string_lossy().into_owned(),
        ];
        if let Some(c) = config {
            args.push("--config".into());
            args.push(c.to_string_lossy().into_owned());
        }
        async move {
            let run = env.run_async(args).await;
            assert_eq!(run.code, 0, "{}", run.dump());
            column(&read_csv(&env.out().join("internal_all.csv")), "Address")
        }
    };
    let without = crawl(None).await;
    assert!(
        without.iter().any(|a| a.contains("/privat/")),
        "Gegenprobe: {without:?}"
    );
    let with = crawl(Some(fixture("out-basic-site/exclude.osconfig"))).await;
    assert!(!with.iter().any(|a| a.contains("/privat/")), "{with:?}");
    assert!(with.len() < without.len());
}

// spec: OUT-186
#[test]
fn unreadable_or_invalid_config_files_and_bad_set_values_are_usage_errors() {
    let env = Env::new();
    let bad = env.work.path().join("kaputt.osconfig");
    std::fs::write(&bad, "[speed\nthreads = 5\n").unwrap();
    let run = env.run(&[
        "--crawl",
        "http://127.0.0.1:1/",
        "--config",
        &bad.to_string_lossy(),
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(
        run.stderr.contains("Zeile 1"),
        "Zeilenangabe fehlt: {}",
        run.dump()
    );

    let missing = env.work.path().join("fehlt.osconfig");
    let run = env.run(&[
        "--crawl",
        "http://127.0.0.1:1/",
        "--config",
        &missing.to_string_lossy(),
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());

    let run = env.run(&["--crawl", "http://127.0.0.1:1/", "--set", "threads=abc"]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(
        run.stderr
            .contains("ungültiger Wert für threads: Ganzzahl 1–200 erwartet"),
        "{}",
        run.dump()
    );

    // Vorschlag nur bei Levenshtein ≤ 3 (K-30), sonst Verweis auf die Liste der setzbaren Schlüssel.
    let run = env.run(&["--crawl", "http://127.0.0.1:1/", "--set", "threadz=1"]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(
        run.stderr.contains("meinten Sie \"threads\"?"),
        "{}",
        run.dump()
    );
    let run = env.run(&["--crawl", "http://127.0.0.1:1/", "--set", "unbekannt=1"]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(
        run.stderr.contains("unbekannter Schlüssel"),
        "{}",
        run.dump()
    );
    assert!(!run.stderr.contains("meinten Sie"), "{}", run.dump());
    assert!(run.stderr.contains("--help set"), "{}", run.dump());
    assert!(env.crawl_files().is_empty(), "kein Crawl bei Aufruffehlern");
}

// spec: OUT-186
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_config_from_a_newer_schema_warns_and_ignores_unknown_fields() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let file = env.work.path().join("neu.osconfig");
    std::fs::write(
        &file,
        "schema_version = 99\nzukunft = 1\n[speed]\nthreads = 2\nneues_feld = \"x\"\n",
    )
    .unwrap();
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            site.base_url(),
            "--config".into(),
            file.to_string_lossy().into_owned(),
            "--set".into(),
            "robots.retries=0".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(
        run.stderr.contains("unbekannter Schlüssel, wird ignoriert"),
        "{}",
        run.dump()
    );
    assert!(run.stderr.contains("schema_version 99"), "{}", run.dump());
    assert_eq!(
        stored_config(&env).speed.threads,
        2,
        "bekannte Felder gelten"
    );
}

// spec: OUT-186
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_overrides_the_config_file_and_max_urls_is_a_shortcut() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let file = env.work.path().join("a.osconfig");
    std::fs::write(&file, "[speed]\nthreads = 5\n").unwrap();
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            site.base_url(),
            "--config".into(),
            file.to_string_lossy().into_owned(),
            "--set".into(),
            "threads=10".into(),
            "--max-urls".into(),
            "7".into(),
            "--set".into(),
            "robots.retries=0".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let cfg = stored_config(&env);
    assert_eq!(cfg.speed.threads, 10, "--set gewinnt gegen die Datei (5)");
    assert_eq!(cfg.limits.max_urls, 7);
    // Das URL-Limit wirkt: höchstens 7 Seiten gecrawlt, Rest bleibt in der Warteschlange.
    assert!(run.stderr.contains("URL-Limit erreicht"), "{}", run.dump());
}

// spec: OUT-186, CRAWL-135
#[test]
fn a_crawl_with_a_malformed_resolve_rule_is_rejected_before_any_crawl() {
    let env = Env::new();
    let run = env.run(&[
        "--crawl",
        "http://site.test/",
        "--resolve",
        "ohne-gleichheitszeichen",
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(env.crawl_files().is_empty());
}
