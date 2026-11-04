//! B3-DIFF (Nutzerentscheidung 04.11.2026) über das Binary: zwei Projektcrawls der Fixture `out-b3-diff`
//! (`v1` → `v2`), Unterfilter-Exporte im Vergleich (OUT-151, OUT-192, OUT-202, OUT-203) und der Report
//! „Crawl Changes“ (OUT-154, OUT-194) mit `--compare-with-previous` und `--crawl-comparison`.

mod common;

use common::*;
use os_model::CrawlConfig;
#[path = "../../os-analyze/tests/change_detection_fixture.rs"]
mod fixture;

const FAST: &str = "robots.retries=0";

fn golden() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/out-b3-diff/crawl_changes.csv")
}

fn text(path: &std::path::Path) -> String {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    raw.strip_prefix('\u{feff}')
        .unwrap_or(&raw)
        .replace("\r\n", "\n")
}

async fn crawl(env: &Env, version: &str, out: &std::path::Path, extra: &[&str]) -> Run {
    let site = site(&format!("out-b3-diff/{version}")).await;
    let mut args: Vec<String> = [
        "--crawl",
        "http://www.site.test/",
        "--resolve",
        &resolve(&site),
        "--set",
        FAST,
        "--project-name",
        "deploy",
        "--output-folder",
        &out.to_string_lossy(),
        "--overwrite",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(extra.iter().map(|s| s.to_string()));
    env.run_async(args).await
}

const EXPORTS: [&str; 5] = [
    "--compare-with-previous",
    "--export-tabs",
    "Internal:HTML:Missing,Response Codes:Client Error (4xx):Removed,internal : html : NEW",
    "--save-report",
    "Crawl Changes",
];

// spec: OUT-151
// spec: OUT-154
// spec: OUT-202
// spec: OUT-194
// spec: OUT-152
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_project_crawl_exports_missing_rows_and_the_crawl_changes_report() {
    let env = Env::new();
    // Erster Projektcrawl: kein Vorlauf → Unterfilter-Exporte werden mit Warnung übersprungen, der Report bleibt
    // leer (Voraussetzung unerfüllt); ohne --strict Exit-Code 0.
    let first_out = env.work.path().join("first");
    let first = crawl(&env, "v1", &first_out, &EXPORTS).await;
    assert_eq!(first.code, 0, "{}", first.dump());
    assert!(
        first.stderr.contains("kein Vergleich vorhanden"),
        "{}",
        first.dump()
    );
    assert!(!first_out.join("internal_html_missing.csv").exists());
    assert_eq!(
        read_csv(&first_out.join("crawl_changes.csv")).len(),
        1,
        "nur die Kopfzeile"
    );
    assert!(
        first.stderr.contains("requires a crawl comparison"),
        "{}",
        first.dump()
    );

    let out = env.work.path().join("second");
    let second = crawl(&env, "v2", &out, &EXPORTS).await;
    assert_eq!(second.code, 0, "{}", second.dump());
    let missing = read_csv(&out.join("internal_html_missing.csv"));
    assert_eq!(
        column(&missing, "Address"),
        ["http://www.site.test/gone.html"]
    );
    assert_eq!(column(&missing, "Comparison Status"), ["Missing"]);
    assert_eq!(
        column(&missing, "Title 1"),
        ["Weg"],
        "Werte aus dem Previous-Crawl"
    );
    let removed = read_csv(&out.join("response_codes_client_error_4xx_removed.csv"));
    assert_eq!(
        column(&removed, "Address"),
        ["http://www.site.test/fixed.html"]
    );
    assert_eq!(column(&removed, "Comparison Status"), ["Removed"]);
    let new = read_csv(&out.join("internal_html_new.csv"));
    assert_eq!(column(&new, "Address"), ["http://www.site.test/new.html"]);
    assert_eq!(text(&out.join("crawl_changes.csv")), text(&golden()));

    // Dieselben zwei Crawls über --crawl-comparison: identischer Report, Reihenfolge der IDs egal.
    let ids: Vec<String> = env
        .crawl_files()
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    for (x, y) in [(0, 1), (1, 0)] {
        let dir = env.work.path().join(format!("pair-{x}"));
        let run = env.run(&[
            "--crawl-comparison",
            &ids[x],
            &ids[y],
            "--save-report",
            "Crawl Changes",
            "--export-tabs",
            "Internal:HTML:Missing",
            "--output-folder",
            &dir.to_string_lossy(),
        ]);
        assert_eq!(run.code, 0, "{}", run.dump());
        assert_eq!(text(&dir.join("crawl_changes.csv")), text(&golden()));
        assert_eq!(
            column(&read_csv(&dir.join("internal_html_missing.csv")), "Address"),
            ["http://www.site.test/gone.html"]
        );
    }
}

// spec: OUT-151
// spec: OUT-203
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subfilters_without_a_comparison_or_on_change_detection_stop_before_the_crawl() {
    let site = site("out-b3-diff/v1").await;
    let env = Env::new();
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            "http://www.site.test/".into(),
            "--resolve".into(),
            resolve(&site),
            "--export-tabs".into(),
            "Internal:HTML:Missing".into(),
        ])
        .await;
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(run.stderr.contains("--crawl-comparison"), "{}", run.dump());
    assert_eq!(
        site.total_requests(),
        0,
        "keine Anfrage vor der Validierung"
    );

    let run = env.run(&[
        "--crawl",
        "http://www.site.test/",
        "--export-tabs",
        "Internal:HTML:Gone",
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(
        run.stderr.contains("unknown export 'Internal:HTML:Gone'"),
        "{}",
        run.dump()
    );

    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let old = fixture::fixture(&env.crawls_dir(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(&env.crawls_dir(), 2, 2, CrawlConfig::default());
    let ids = [
        old.meta().unwrap().crawl_id.to_string(),
        new.meta().unwrap().crawl_id.to_string(),
    ];
    let run = env.run(&[
        "--crawl-comparison",
        &ids[0],
        &ids[1],
        "--export-tabs",
        "Change Detection:Page Titles:Added",
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(run.stderr.contains("Change Detection"), "{}", run.dump());
}

// spec: OUT-154
// spec: OUT-194
#[test]
fn the_report_without_a_comparison_warns_and_strict_turns_it_into_exit_code_4() {
    let env = Env::new();
    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let file = fixture::fixture(&env.crawls_dir(), 1, 1, CrawlConfig::default());
    let path = file.path().to_string_lossy().into_owned();
    let out = env.out();
    for (strict, code) in [(false, 0), (true, 4)] {
        let mut args = vec![
            "--load-crawl",
            &path,
            "--save-report",
            "Crawl Changes",
            "--output-folder",
            out.to_str().unwrap(),
            "--overwrite",
        ];
        if strict {
            args.push("--strict");
        }
        let run = env.run(&args);
        assert_eq!(run.code, code, "{}", run.dump());
        assert!(
            run.stderr.contains("requires a crawl comparison"),
            "{}",
            run.dump()
        );
        assert_eq!(read_csv(&out.join("crawl_changes.csv")).len(), 1);
    }
}

// spec: OUT-151
// spec: OUT-152
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_consolidated_segment_workbook_warns_about_subfilters_without_a_comparison() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let config = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/out-m2-cli/segments.osconfig");
    let run = env
        .run_async(vec![
            "--crawl".to_string(),
            "http://site.test/".into(),
            "--resolve".into(),
            resolve(&site),
            "--set".into(),
            FAST.into(),
            "--config".into(),
            config.to_string_lossy().into_owned(),
            "--project-name".into(),
            "segments".into(),
            "--compare-with-previous".into(),
            "--segment".into(),
            "all".into(),
            "--single-workbook".into(),
            "--export-format".into(),
            "xlsx".into(),
            "--export-tabs".into(),
            "Internal:All,Internal:HTML:Missing".into(),
            "--output-folder".into(),
            env.out().to_string_lossy().into_owned(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    // Erster Projektcrawl: kein Vergleich – der Unterfilter wird mit genau einer Warnung übersprungen, nicht stumm.
    let warning =
        "Warnung: Export 'Internal:HTML:Missing' übersprungen: kein Vergleich vorhanden (OUT-151)";
    assert_eq!(run.stderr.matches(warning).count(), 1, "{}", run.dump());
    assert!(!file_names(&env.out()).is_empty());
}
