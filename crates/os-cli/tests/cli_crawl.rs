//! Crawl, Exporte, Speichern und Ausgabeordner (OUT-182, OUT-189 bis OUT-195, CRAWL-135) gegen `out-basic-site`.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::{column, file_names, golden_text, read_csv, read_xlsx, resolve, site, Env};
use os_engine::testing::crawl_fixture;
use os_model::CrawlConfig;
use os_store::{CrawlFile, OpenMode};
use os_testsite::golden::assert_golden;

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/out-basic-site")
        .join(name)
}

/// Kurzer Crawl ab der Basis-URL der Testsite. Der Abruf von `localhost:8900` (externer Link der Fixture)
/// soll nicht auf Wiederholungen der robots.txt warten; ein Thread macht die Abfolge reproduzierbar.
fn crawl_args(base: &str, extra: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = [
        "--crawl",
        base,
        "--set",
        "threads=1",
        "--set",
        "robots.retries=0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(extra.iter().map(|s| s.to_string()));
    v
}

const EXPORT_ARGS: [&str; 8] = [
    "--export-tabs",
    "Internal:All,Response Codes:Client Error (4xx)",
    "--bulk-export",
    "Links:All Inlinks",
    "--save-report",
    "Crawl Overview,Redirects:Redirect Chains",
    "--output-folder",
    "OUT",
];

fn export_args(out: &Path) -> Vec<String> {
    EXPORT_ARGS
        .iter()
        .map(|a| {
            if *a == "OUT" {
                out.to_string_lossy().into_owned()
            } else {
                (*a).to_string()
            }
        })
        .collect()
}

const GOLDEN_FILES: [&str; 5] = [
    "internal_all",
    "response_codes_client_error_4xx",
    "links_all_inlinks",
    "crawl_overview",
    "redirect_chains",
];

// spec: OUT-192, OUT-193, OUT-194, OUT-195, CRAWL-135
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn golden_end_to_end_in_csv_and_xlsx() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let bases = site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let env = Env::new();

    // 1. Crawl mit allen Exporten als CSV.
    let mut args = crawl_args(&base, &[]);
    args.extend(export_args(&env.out()));
    let run = env.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(run.last_line().ends_with("(Exit-Code 0)"), "{}", run.dump());
    for name in GOLDEN_FILES {
        let rows = read_csv(&env.out().join(format!("{name}.csv")));
        assert_golden(golden(&format!("{name}.csv")), &golden_text(&rows, &bases));
    }
    // spec-abgeleitete Kontrollen (unabhängig vom Golden)
    let internal = read_csv(&env.out().join("internal_all.csv"));
    assert!(column(&internal, "Address").contains(&format!("{base}/about.html")));
    let client_errors = read_csv(&env.out().join("response_codes_client_error_4xx.csv"));
    assert_eq!(
        column(&client_errors, "Address"),
        [format!("{base}/missing.html")]
    );
    assert_eq!(column(&client_errors, "Status Code"), ["404"]);

    // 2. Dieselben Exporte als XLSX aus dem gespeicherten Crawl, ohne Netzwerkzugriff.
    let id = &env.crawl_files()[0];
    let id = id.file_stem().unwrap().to_string_lossy().into_owned();
    let before = site.total_requests();
    let xlsx_out = env.work.path().join("xlsx");
    let mut args = vec![
        "--load-crawl".to_string(),
        id,
        "--export-format".into(),
        "xlsx".into(),
    ];
    args.extend(export_args(&xlsx_out));
    let run = env.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(
        site.total_requests(),
        before,
        "--load-crawl braucht kein Netz"
    );
    for name in GOLDEN_FILES {
        let rows = read_xlsx(&xlsx_out.join(format!("{name}.xlsx")));
        assert_golden(
            golden(&format!("{name}.xlsx.txt")),
            &golden_text(&rows, &bases),
        );
    }
}

// spec: OUT-182, CRAWL-135
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_cli_crawl_has_the_same_urls_statuses_and_depths_as_an_engine_crawl() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let mut args = crawl_args(
        "http://site.test/",
        &["--resolve", &resolve(&site), "--save-crawl"],
    );
    args.extend([
        "--output-folder".to_string(),
        env.out().to_string_lossy().into_owned(),
    ]);
    let run = env.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());

    let query = |file: &CrawlFile, prefix: &str| -> BTreeSet<(String, i64, i64)> {
        let mut stmt = file
            .conn()
            .prepare(
                "SELECT address, COALESCE(status_code, -1), COALESCE(crawl_depth, -1) FROM urls \
                 WHERE is_internal = 1",
            )
            .unwrap();
        stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .map(|r| {
            let (a, s, d) = r.unwrap();
            (a.strip_prefix(prefix).unwrap_or(&a).to_string(), s, d)
        })
        .collect()
    };
    let cli_file = CrawlFile::open(&env.out().join("crawl.ospider"), OpenMode::ReadOnly).unwrap();
    let cli = query(&cli_file, "http://site.test");

    let mut cfg = CrawlConfig::default();
    cfg.robots.retries = 0;
    let engine_crawl = crawl_fixture("out-basic-site", cfg).await;
    let prefix = engine_crawl.site.base_url();
    let engine = query(&engine_crawl.file, prefix.trim_end_matches('/'));
    assert!(cli.len() > 10, "{cli:?}");
    assert_eq!(cli, engine);
}

// spec: OUT-189
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn save_crawl_writes_a_loadable_copy_and_no_store_discards_the_crawl() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let env = Env::new();
    let out = env.out();

    // --save-crawl: crawl.ospider im Ausgabeordner, per Pfad wieder ladbar mit identischen Exporten.
    let mut args = crawl_args(&base, &["--save-crawl", "--export-tabs", "Internal:All"]);
    args.extend([
        "--output-folder".to_string(),
        out.to_string_lossy().into_owned(),
    ]);
    let run = env.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(file_names(&out), ["crawl.ospider", "internal_all.csv"]);
    assert_eq!(
        env.crawl_files().len(),
        1,
        "der Crawl liegt auch in der Crawl-Verwaltung"
    );

    let again = env.work.path().join("again");
    let run = env
        .run_async(vec![
            "--load-crawl".to_string(),
            out.join("crawl.ospider").to_string_lossy().into_owned(),
            "--export-tabs".into(),
            "Internal:All".into(),
            "--output-folder".into(),
            again.to_string_lossy().into_owned(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let first = read_csv(&out.join("internal_all.csv"));
    let second = read_csv(&again.join("internal_all.csv"));
    let norm = |rows: &[Vec<String>]| golden_text(rows, &[]);
    assert_eq!(norm(&first), norm(&second));

    // --no-store: Die Crawl-Datei verschwindet nach dem Ende.
    let env2 = Env::new();
    let mut args = crawl_args(&base, &["--no-store", "--export-tabs", "Internal:All"]);
    args.extend([
        "--output-folder".to_string(),
        env2.out().to_string_lossy().into_owned(),
    ]);
    let run = env2.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(file_names(&env2.out()), ["internal_all.csv"]);
    assert!(env2.crawl_files().is_empty(), "{:?}", env2.crawl_files());
    let list = env2.run(&["--list-crawls", "--json"]);
    assert_eq!(list.stdout.trim(), "[]");
}

// spec: OUT-190
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_folder_rules_overwrite_timestamped_and_unicode_paths() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let env = Env::new();
    let run_with = |out: PathBuf, extra: &[&str]| {
        let mut args = crawl_args(&base, &["--export-tabs", "Internal:All"]);
        args.extend(extra.iter().map(|s| s.to_string()));
        args.extend([
            "--output-folder".to_string(),
            out.to_string_lossy().into_owned(),
        ]);
        args
    };

    // Pfad mit Leerzeichen und Nicht-ASCII-Zeichen wird angelegt.
    let unicode = env.work.path().join("Jürgen").join("SEO Reports");
    let run = env.run_async(run_with(unicode.clone(), &[])).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(unicode.join("internal_all.csv").is_file());

    // Ohne --overwrite wird ein Ordner mit Dateien abgelehnt (vor dem Crawl).
    let before = site.total_requests();
    let run = env.run_async(run_with(unicode.clone(), &[])).await;
    assert_eq!(run.code, 2, "{}", run.dump());
    assert_eq!(site.total_requests(), before, "Ablehnung vor dem Crawl");

    // Mit --overwrite ersetzt.
    std::fs::write(unicode.join("internal_all.csv"), "alt").unwrap();
    let run = env
        .run_async(run_with(unicode.clone(), &["--overwrite"]))
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_ne!(
        std::fs::read_to_string(unicode.join("internal_all.csv")).unwrap(),
        "alt"
    );

    // --timestamped-output legt <out>/<YYYY-MM-DD_HH-MM-SS>/ an; mit --overwrite hat er Vorrang (Warnung).
    let stamped = env.work.path().join("stamped");
    let run = env
        .run_async(run_with(
            stamped.clone(),
            &["--timestamped-output", "--overwrite"],
        ))
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(
        run.stderr.contains("--timestamped-output hat Vorrang"),
        "{}",
        run.dump()
    );
    let subs = file_names(&stamped);
    assert_eq!(subs.len(), 1, "{subs:?}");
    let name = &subs[0];
    let shape: Vec<bool> = name.chars().map(|c| c.is_ascii_digit()).collect();
    assert_eq!(name.len(), 19, "{name}");
    assert!(
        shape[..4].iter().all(|b| *b) && name.as_bytes()[4] == b'-' && name.as_bytes()[10] == b'_',
        "{name}"
    );
    assert!(stamped.join(name).join("internal_all.csv").is_file());
}

// spec: OUT-190
#[test]
fn an_output_path_ending_in_a_backslash_before_the_quote_is_repaired() {
    // Die Shell liefert `--output-folder "C:\out\" --export-tabs …` als EIN Argument; die CLI korrigiert es.
    let env = Env::new();
    let out = env.work.path().join("ziel");
    let swallowed = format!("{}\" --list-crawls", out.to_string_lossy());
    let run = env.run(&["--output-folder", &swallowed]);
    // Ohne Quelle endet der Aufruf mit Exit-Code 2 – aber nach der Korrektur wurde `--list-crawls` erkannt.
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(run.stderr.contains("Backslash"), "{}", run.dump());
}

// spec: OUT-191, OUT-185
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_and_task_names_show_up_in_list_crawls() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let env = Env::new();
    for extra in [
        &["--project-name", "Kunde A", "--task-name", "Relaunch"][..],
        &["--project-name", "Kunde A"][..],
        &["--project-name", "Kunde B", "--task-name", "Start"][..],
        &[][..],
    ] {
        let run = env.run_async(crawl_args(&base, extra)).await;
        assert_eq!(run.code, 0, "{}", run.dump());
    }
    let all = env.run(&["--list-crawls", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&all.stdout).unwrap();
    let rows = v.as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let by = |project: &str| -> Vec<String> {
        let mut names: Vec<String> = rows
            .iter()
            .filter(|r| r["project"] == project)
            .map(|r| r["name"].as_str().unwrap().to_string())
            .collect();
        names.sort();
        names
    };
    assert_eq!(by("Kunde A"), ["127.0.0.1", "Relaunch"]);
    assert_eq!(by("Kunde B"), ["Start"]);
    // Ohne Angabe: Projekt „Default“, Name = Host der Start-URL.
    assert_eq!(by("Default"), ["127.0.0.1"]);

    // Filter, Tabelle und stabile IDs
    let filtered = env.run(&["--list-crawls", "--project-name", "Kunde A"]);
    assert_eq!(filtered.stdout.lines().count(), 3, "{}", filtered.stdout);
    assert!(filtered.stdout.lines().next().unwrap().starts_with("ID"));
    for r in rows {
        assert_eq!(r["status"], "Completed");
        assert_eq!(r["mode"], "spider");
        assert!(filtered.stdout.contains(r["id"].as_str().unwrap()) || r["project"] != "Kunde A");
    }
    let ids: BTreeSet<String> = rows
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect();
    let files: BTreeSet<String> = env
        .crawl_files()
        .iter()
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        ids, files,
        "die ID ist der Dateiname in der Crawl-Verwaltung"
    );
}

// spec: OUT-192
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_names_are_case_insensitive_tolerate_spaces_and_escape_commas() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let mut args = crawl_args(
        &site.base_url(),
        &[
            "--export-tabs",
            "  internal : all ,RESPONSE CODES:client error (4xx)",
        ],
    );
    args.extend([
        "--output-folder".to_string(),
        env.out().to_string_lossy().into_owned(),
    ]);
    let run = env.run_async(args).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(
        file_names(&env.out()),
        ["internal_all.csv", "response_codes_client_error_4xx.csv"]
    );
}

// spec: OUT-194
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_report_with_an_unmet_prerequisite_warns_and_exit_4_only_with_strict() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let run_once = |strict: bool| {
        let env = Env::new();
        let mut extra = vec![
            "--set",
            "analysis.sitemaps=false",
            "--save-report",
            "Crawl Overview",
            "--output-folder",
        ];
        let out = env.out().to_string_lossy().into_owned();
        extra.push(&out);
        if strict {
            extra.push("--strict");
        }
        let args = crawl_args(&base, &extra);
        async move { (env.run_async(args).await, env) }
    };
    let (lenient, env) = run_once(false).await;
    assert_eq!(lenient.code, 0, "{}", lenient.dump());
    assert!(
        lenient.stderr.contains("requires the crawl analysis"),
        "{}",
        lenient.dump()
    );
    assert!(
        env.out().join("crawl_overview.csv").is_file(),
        "die Datei entsteht trotzdem"
    );
    let (strict, _env) = run_once(true).await;
    assert_eq!(strict.code, 4, "{}", strict.dump());
}

// spec: CRAWL-135
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_progress_writes_json_lines_to_stderr_and_nothing_to_stdout() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let run = env
        .run_async(crawl_args(&site.base_url(), &["--json-progress"]))
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(run.stdout.is_empty());
    let phases: Vec<String> = run
        .stderr
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["phase"].as_str().map(str::to_string))
        .collect();
    for phase in ["validate", "crawl", "done"] {
        assert!(
            phases.iter().any(|p| p == phase),
            "{phase} fehlt in {phases:?}"
        );
    }
    assert_eq!(phases.last().map(String::as_str), Some("done"));
}

fn analysis_state(path: &Path) -> os_analyze::analysis::persist::RunState {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    os_analyze::analysis::persist::read_run_state(&file).unwrap()
}

// spec: CRAWL-135
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_crawl_analysis_runs_at_the_end_of_the_crawl_and_before_dependent_exports() {
    use os_analyze::analysis::persist::RunState;
    let site = site("out-basic-site").await;
    let base = site.base_url();

    // Hook am Crawl-Ende (CRAWL-141): Die Analyse ist fertig, noch bevor ein Export läuft.
    let env = Env::new();
    let run = env.run_async(crawl_args(&base, &[])).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let path = env.crawl_files().remove(0);
    assert_eq!(analysis_state(&path), RunState::Completed);

    // Ohne automatische Analyse am Ende: nicht gelaufen; ein analyseabhängiger Export holt sie mit Hinweis nach.
    let env = Env::new();
    let run = env
        .run_async(crawl_args(&base, &["--set", "analysis.auto_at_end=false"]))
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let path = env.crawl_files().remove(0);
    assert_ne!(analysis_state(&path), RunState::Completed);
    let id = path.file_stem().unwrap().to_string_lossy().into_owned();
    let run = env
        .run_async(vec![
            "--load-crawl".to_string(),
            id,
            "--export-tabs".into(),
            "Canonicals:Missing".into(),
            "--output-folder".into(),
            env.out().to_string_lossy().into_owned(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(
        run.stderr.contains("Crawl-Analyse ist nicht aktuell"),
        "{}",
        run.dump()
    );
    assert_eq!(analysis_state(&path), RunState::Completed);
    assert!(env.out().join("canonicals_missing.csv").is_file());
}

// spec: OUT-182
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_url_without_a_scheme_gets_http_and_a_hint_and_an_invalid_url_stops_before_the_crawl() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            "site.test".into(),
            "--resolve".into(),
            resolve(&site),
            "--set".into(),
            "robots.retries=0".into(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(
        run.stderr.contains("'http://' wird ergänzt"),
        "{}",
        run.dump()
    );
    assert!(site.count("site.test", "/") >= 1);
    let list = env.run(&["--list-crawls", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&list.stdout).unwrap();
    assert_eq!(v[0]["start_url"], "http://site.test/");
    assert_eq!(v[0]["name"], "site.test");

    let before = site.total_requests();
    let run = env
        .run_async(vec!["--crawl".to_string(), "ftp://site.test/".into()])
        .await;
    assert_eq!(run.code, 2, "{}", run.dump());
    let run = env
        .run_async(vec!["--crawl".to_string(), "http://".into()])
        .await;
    assert_eq!(run.code, 2, "{}", run.dump());
    assert_eq!(site.total_requests(), before);
}
