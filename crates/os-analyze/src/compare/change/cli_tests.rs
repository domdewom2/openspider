//! Öffentliche CLI-/Scheduling-Abläufe; eingebunden von os-cli::exec.
use super::*;
use clap::Parser;
use os_analyze::compare::change::{self, auto};
use os_model::{CancelToken, CrawlConfig};
use os_store::paths::{app_paths_with_home, AppPaths};

#[path = "../../../tests/change_detection_fixture.rs"]
mod fixture;

async fn invoke(paths: &AppPaths, args: Vec<String>) -> (Outcome, Arc<Reporter>) {
    let cli = crate::Cli::try_parse_from(std::iter::once("openspider".into()).chain(args)).unwrap();
    let rep = Reporter::new(false);
    let plan = crate::plan::build(&cli.run, paths.clone(), &rep).unwrap();
    let result = execute(plan, rep.clone(), Signals::manual(CancelToken::new())).await;
    (result, rep)
}
fn args(path: &Path, flag: &str) -> Vec<String> {
    let mut args = vec![
        "--load-crawl".into(),
        path.to_string_lossy().into_owned(),
        "--project-name".into(),
        "project".into(),
        flag.into(),
    ];
    if flag == "--project-crawl-comparison" {
        args.push("true".into());
    }
    args
}
fn paths(home: &Path) -> AppPaths {
    let paths = app_paths_with_home(Some(home.as_os_str()));
    paths.create_all().unwrap();
    paths
}

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
#[tokio::test]
async fn public_cli_flags_have_identical_saved_results_and_first_crawl_succeeds() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let old = fixture::fixture(&paths.crawls, 1, 1, CrawlConfig::default());
    let first = invoke(&paths, args(old.path(), "--compare-with-previous"))
        .await
        .0;
    assert_eq!(first.exit, Exit::Ok, "{}", first.summary);
    assert_eq!(
        old.get_meta("compare.auto_state").unwrap().as_deref(),
        Some("no_previous")
    );
    let new = fixture::fixture(&paths.crawls, 2, 2, CrawlConfig::default());
    for flag in ["--compare-with-previous", "--project-crawl-comparison"] {
        let result = invoke(&paths, args(new.path(), flag)).await.0;
        assert_eq!(result.exit, Exit::Ok, "{}", result.summary);
    }
    let artifacts = std::fs::read_dir(paths.crawls.join("comparisons"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let (saved, cfg) = auto::load(&artifacts[0].path(), &CancelToken::new()).unwrap();
    let manual = fixture::comparison(
        CrawlFile::open(new.path(), OpenMode::ReadOnly).unwrap(),
        CrawlFile::open(old.path(), OpenMode::ReadOnly).unwrap(),
    );
    change::calculate(&manual, &cfg.change_detection, &CancelToken::new()).unwrap();
    assert_eq!(
        change::counts(&saved, &cfg.change_detection, false).unwrap(),
        change::counts(&manual, &cfg.change_detection, false).unwrap()
    );
}

// spec: OUT-152
#[tokio::test]
async fn comparison_write_and_corrupt_artifact_fail_even_without_strict_and_never_fall_back() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let _old = fixture::fixture(&paths.crawls, 1, 1, CrawlConfig::default());
    let new = fixture::fixture(&paths.crawls, 2, 2, CrawlConfig::default());
    let output = home.path().join("exports");
    let mut args = args(new.path(), "--compare-with-previous");
    args.extend([
        "--output-folder".into(),
        output.to_string_lossy().into_owned(),
        "--export-tabs".into(),
        "Internal:All".into(),
        "--overwrite".into(),
    ]);
    let comparisons = paths.crawls.join("comparisons");
    std::fs::write(&comparisons, "blocks directory creation").unwrap();
    let (result, rep) = invoke(&paths, args.clone()).await;
    assert_eq!(result.exit, Exit::ExportFailed);
    assert!(rep.exported_files().is_empty());
    std::fs::remove_file(&comparisons).unwrap();
    assert_eq!(invoke(&paths, args.clone()).await.0.exit, Exit::Ok);
    let artifact = std::fs::read_dir(&comparisons)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(artifact, "damaged comparison database").unwrap();
    let (result, rep) = invoke(&paths, args.clone()).await;
    assert_eq!(result.exit, Exit::ExportFailed);
    assert!(rep.exported_files().is_empty());
    // Auch ohne Export muss der angeforderte Vergleich den Exit-Code beeinflussen.
    args.truncate(5);
    assert_eq!(invoke(&paths, args).await.0.exit, Exit::ExportFailed);
}

// spec: OUT-152
// spec: TECH-301
#[tokio::test]
async fn cli_comparison_exports_intersect_saved_segment_for_normal_and_change_tabs() {
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let cfg = CrawlConfig { segments: serde_json::from_value(serde_json::json!({"list":[{"name":"Selected","color":"#123456","groups":[{"conditions":[{"field":"Address","op":"equals","value":"https://site.test/a"}]}]}]})).unwrap(), ..Default::default() };
    let _old = fixture::fixture(&paths.crawls, 1, 1, cfg.clone());
    let new = fixture::fixture(&paths.crawls, 2, 2, cfg);
    let output = home.path().join("exports");
    let mut args = args(new.path(), "--compare-with-previous");
    args.extend([
        "--output-folder".into(),
        output.to_string_lossy().into_owned(),
        "--export-tabs".into(),
        "Internal:All,Change Detection:All".into(),
        "--segment".into(),
        "Selected".into(),
    ]);
    let (result, rep) = invoke(&paths, args).await;
    assert_eq!(result.exit, Exit::Ok, "{}", result.summary);
    assert_eq!(rep.exported_files().len(), 2);
    for path in rep.exported_files() {
        let mut csv = csv::Reader::from_path(path).unwrap();
        let rows = csv.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(&rows[0][0], "https://site.test/a");
    }
}

// spec: OUT-152
// spec: TECH-305
// spec: OUT-145
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_crawl_content_uses_the_content_area_and_reclassifies_partial_changes() {
    let home = tempfile::tempdir().unwrap();
    let site_dir = tempfile::tempdir().unwrap();
    std::fs::write(site_dir.path().join("scenario.toml"), "").unwrap();
    std::fs::write(site_dir.path().join("index.html"), "<html><body><a href='/partial.html'>Partial</a><a href='/footer.html'>Footer</a></body></html>").unwrap();
    let words = |prefix: &str| {
        (0..30)
            .map(|n| format!("{prefix}{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let common = words("common");
    let different = words("different");
    let page = |main: &str, footer: &str| {
        format!("<html><head><title>Content</title></head><body><main>{main}</main><footer>{footer}</footer></body></html>")
    };
    std::fs::write(
        site_dir.path().join("partial.html"),
        page(&format!("{common} {common}"), "old footer"),
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("footer.html"),
        page(&common, "old footer"),
    )
    .unwrap();
    let site = os_testsite::TestSite::start_dir(site_dir.path(), 0)
        .await
        .unwrap();
    let paths = paths(home.path());
    let crawl = || {
        vec![
            "--crawl".into(),
            site.base_url(),
            "--headless".into(),
            "--project-name".into(),
            "project".into(),
            "--set".into(),
            "robots.retries=0".into(),
            "--set".into(),
            "threads=1".into(),
        ]
    };
    let first = invoke(&paths, crawl()).await.0;
    assert_eq!(first.exit, Exit::Ok, "{}", first.summary);
    std::fs::write(
        site_dir.path().join("partial.html"),
        page(&format!("{common} {different}"), "old footer"),
    )
    .unwrap();
    std::fs::write(
        site_dir.path().join("footer.html"),
        page(&common, "completely changed footer text"),
    )
    .unwrap();
    let second = invoke(&paths, crawl()).await.0;
    assert_eq!(second.exit, Exit::Ok, "{}", second.summary);
    let c = fixture::comparison(
        CrawlFile::open(second.crawl_path.as_ref().unwrap(), OpenMode::ReadOnly).unwrap(),
        CrawlFile::open(first.crawl_path.as_ref().unwrap(), OpenMode::ReadOnly).unwrap(),
    );
    let mut cfg = fixture::all();
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    let pct = |path: &str| {
        c.current().file.conn().query_row("SELECT pct FROM temp.c02_cells JOIN urls u ON u.id=url_id WHERE element='content' AND u.address LIKE ?1",[format!("%/{path}.html")],|r|r.get::<_,f64>(0)).unwrap()
    };
    assert_eq!(pct("footer"), 0.0);
    assert!(
        (35.0..=65.0).contains(&pct("partial")),
        "{}",
        pct("partial")
    );
    assert_eq!(change::ids(&c, "Content", true).unwrap().len(), 1);
    cfg.content_threshold_pct = 65;
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    assert!(change::ids(&c, "Content", true).unwrap().is_empty());
}

// spec: OUT-152
// spec: TECH-308
// spec: CRAWL-036
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_task_runs_persist_exactly_one_comparison_of_the_last_two_crawls() {
    use crate::run_task::{run_in, DefaultHooks, RunCtx};
    use os_sched::tasks::{Task, TaskStore};
    let home = tempfile::tempdir().unwrap();
    let paths = paths(home.path());
    let site = os_testsite::TestSite::start("out-basic-site")
        .await
        .unwrap();
    let task: Task = serde_json::from_value(serde_json::json!({
        "id":"c02-auto","name":"C02 Auto","project":"project",
        "schedule":{"kind":"daily","time":"03:00"},
        "crawl":{"source":{"kind":"spider","url":site.base_url()},"overrides":{"threads":"1","robots.retries":"0"}},
        "execution":{"auto_compare":true}
    })).unwrap();
    let task = TaskStore::in_settings(&paths.settings).save(task).unwrap();
    let ctx = RunCtx {
        paths: paths.clone(),
        clock: Arc::new(os_model::SystemClock),
        secrets: Arc::new(os_fetch::MemorySecretStore::new()),
        run_id: None,
        manual: false,
        log_path: None,
        export_dir: home.path().join("exports"),
        json_progress: false,
        heartbeat: std::time::Duration::from_secs(60),
        poll: std::time::Duration::from_millis(20),
        timeout_scale: 1.0,
    };
    let mut ids = Vec::new();
    for _ in 0..2 {
        let run = run_in(
            &ctx,
            &task.id,
            &DefaultHooks,
            Signals::manual(CancelToken::new()),
            Reporter::for_task(false),
        )
        .await;
        assert_eq!(run.exit, Exit::Ok, "{}", run.summary);
        ids.push(run.record.unwrap().crawl_id.unwrap());
    }
    let artifacts = std::fs::read_dir(paths.crawls.join("comparisons"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(artifacts.len(), 1);
    let (comparison, config) = auto::load(&artifacts[0].path(), &CancelToken::new()).unwrap();
    assert_eq!(comparison.previous().meta.crawl_id.to_string(), ids[0]);
    assert_eq!(comparison.current().meta.crawl_id.to_string(), ids[1]);
    assert_eq!(
        auto::saved_counts(&artifacts[0].path(), false).unwrap(),
        change::counts(&comparison, &config.change_detection, false).unwrap()
    );
}
