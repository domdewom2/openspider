//! `openspider --run-task` als eigener Prozess (OUT-209, OUT-163, OUT-165, OUT-167, OUT-168, OUT-169,
//! OUT-171, OUT-240) gegen `out-basic-site` und `out-queue`. Der OS-Planer wird nie angesprochen.

mod common;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use common::{file_names, golden_text, read_csv, site, Env};
use os_sched::history::{HistoryStore, RunRecord, RunResult};
use os_sched::tasks::{
    CrawlSpec, Execution, ExportFormat, ExportOptions, Notify, Output, Overlap, Schedule, Source,
    Task, TaskStore,
};
use os_store::{CrawlFile, OpenMode};

fn threads_one() -> std::collections::BTreeMap<String, String> {
    [("threads", "1"), ("robots.retries", "0")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Aufgabe mit Spider-Quelle, einem Thread und Exporten in `out`.
fn task(id: &str, name: &str, url: &str, out: &Path, items: &[(&str, &str)]) -> Task {
    Task {
        id: id.into(),
        name: name.into(),
        project: "Shop".into(),
        description: String::new(),
        enabled: true,
        schedule: Schedule::Daily {
            time: "03:00".into(),
            from: None,
        },
        crawl: CrawlSpec {
            source: Source::Spider { url: url.into() },
            config: None,
            auth_profile: None,
            overrides: threads_one(),
        },
        execution: Execution::default(),
        apis: Vec::new(),
        output: Output::default(),
        exports: Vec::new(),
        export_options: Some(ExportOptions {
            items: items
                .iter()
                .map(|(k, n)| (k.to_string(), n.to_string()))
                .collect(),
            format: ExportFormat::Csv,
            dir: out.to_string_lossy().into_owned(),
            single_workbook: false,
            skip_empty: false,
        }),
        preset: None,
        custom_summary: None,
        notify: Notify::default(),
    }
}

fn save(env: &Env, task: &Task) {
    TaskStore::in_settings(env.home.path())
        .save(task.clone())
        .expect("Aufgabe speicherbar");
}

fn history(env: &Env) -> HistoryStore {
    HistoryStore::in_settings(env.home.path())
}

/// Der einzige Unterordner (Zeitstempel-Ordner) von `<out>/<Projekt>/<Aufgabe>`.
fn run_folder(out: &Path, project: &str, name: &str) -> PathBuf {
    let base = out.join(project).join(name);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&base)
        .unwrap_or_else(|e| panic!("{}: {e}", base.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    dirs.remove(0)
}

const TABLES: [(&str, &str); 5] = [
    ("tab", "Internal:All"),
    ("tab", "Response Codes:Client Error (4xx)"),
    ("bulk", "Links:All Inlinks"),
    ("report", "Crawl Overview"),
    ("report", "Redirects:Redirect Chains"),
];
const FILES: [&str; 5] = [
    "internal_all.csv",
    "response_codes_client_error_4xx.csv",
    "links_all_inlinks.csv",
    "crawl_overview.csv",
    "redirect_chains.csv",
];

// spec: OUT-209
// spec: OUT-163
// spec: OUT-165
// spec: OUT-169
// spec: OUT-240
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_scheduled_run_exports_exactly_like_the_equivalent_manual_call_and_records_history() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let bases = site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let env = Env::new();
    let out = env.work.path().join("task-out");
    save(&env, &task("nightly", "Nightly", &base, &out, &TABLES));

    // Der Lauf des Planers.
    let scheduled = env
        .run_async(vec!["--run-task".to_string(), "nightly".into()])
        .await;
    assert_eq!(scheduled.code, 0, "{}", scheduled.dump());
    assert!(
        scheduled.last_line().ends_with("(Exit-Code 0)"),
        "{}",
        scheduled.dump()
    );
    assert!(
        scheduled.stdout.is_empty(),
        "stdout bleibt Nutzdaten vorbehalten"
    );
    let folder = run_folder(&out, "Shop", "Nightly");

    // Der gleichwertige Aufruf von Hand (eigene Daten, gleiche Quelle, gleiche Optionen).
    let manual_env = Env::new();
    let manual_out = manual_env.out();
    let mut args: Vec<String> = [
        "--crawl",
        &base,
        "--set",
        "threads=1",
        "--set",
        "robots.retries=0",
        "--export-tabs",
        "Internal:All,Response Codes:Client Error (4xx)",
        "--bulk-export",
        "Links:All Inlinks",
        "--save-report",
        "Crawl Overview,Redirects:Redirect Chains",
        "--output-folder",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(manual_out.to_string_lossy().into_owned());
    let manual = manual_env.run_async(args).await;
    assert_eq!(manual.code, 0, "{}", manual.dump());

    assert_eq!(file_names(&folder), file_names(&manual_out));
    for name in FILES {
        let a = golden_text(&read_csv(&folder.join(name)), &bases);
        let b = golden_text(&read_csv(&manual_out.join(name)), &bases);
        assert_eq!(a, b, "{name} weicht vom Aufruf von Hand ab");
    }

    // Historie: Completed, URLs, Crawl-ID, Dateien, Lauf-Log.
    let runs = history(&env).list(Some("nightly"), 0, 10);
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert!(common::is_completed(run), "{run:?}");
    assert_eq!(run.exit_code, Some(0));
    assert!(run.urls_crawled.unwrap_or(0) > 5);
    let crawls = env.crawl_files();
    assert_eq!(crawls.len(), 1);
    assert_eq!(
        crawls[0].file_stem().unwrap().to_string_lossy(),
        run.crawl_id.clone().unwrap()
    );
    assert_eq!(run.exports.len(), FILES.len());
    assert_eq!(
        run.output_folder.as_deref(),
        Some(folder.to_string_lossy().as_ref())
    );
    assert!(run.ended_at.is_some() && !run.manual);

    // Lauf-Log task-<id>-<zeitstempel>.log mit Zeitstempel samt Millisekunden und Offset, Start und Ende.
    let log = run.log_path.clone().expect("Log-Pfad in der Historie");
    assert!(log.contains("task-nightly-"), "{log}");
    let text = std::fs::read_to_string(&log).unwrap();
    let line = regex_like_timestamp(text.lines().next().unwrap());
    assert!(
        line,
        "Zeitstempel mit Millisekunden und Offset erwartet: {text}"
    );
    assert!(text.contains("task run started id=nightly"), "{text}");
    assert!(
        text.contains("task run finished result=Completed"),
        "{text}"
    );
    assert!(text.contains("mode=spider"), "{text}");
}

/// `2026-10-20T03:00:00.123+02:00 …` (OUT-240).
fn regex_like_timestamp(line: &str) -> bool {
    let b = line.as_bytes();
    line.len() > 29
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'.'
        && b[20..23].iter().all(u8::is_ascii_digit)
        && (b[23] == b'+' || b[23] == b'-')
        && b[26] == b':'
}

// spec: OUT-165
// spec: OUT-166
// spec: OUT-133
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn internal_all_redirect_chains_and_an_xml_sitemap_make_three_files() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("three");
    let mut t = task(
        "three",
        "Three",
        &site.base_url(),
        &out,
        &[
            ("tab", "Internal:All"),
            ("report", "Redirects:Redirect Chains"),
        ],
    );
    t.exports.push(os_sched::tasks::ExportItem {
        kind: "sitemap".into(),
        name: "pages".into(),
        format: ExportFormat::Csv,
        dir: None,
    });
    save(&env, &t);
    let run = env
        .run_async(vec!["--run-task".to_string(), "three".into()])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let folder = run_folder(&out, "Shop", "Three");
    let names = file_names(&folder);
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names.contains(&"internal_all.csv".to_string()));
    assert!(names.contains(&"redirect_chains.csv".to_string()));
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("sitemap") && n.ends_with(".xml")),
        "{names:?}"
    );
    let sitemap = std::fs::read_to_string(
        folder.join(names.iter().find(|n| n.starts_with("sitemap")).unwrap()),
    )
    .unwrap();
    assert!(sitemap.contains("<urlset"), "{sitemap}");
}

// spec: OUT-209
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_task_id_exits_with_2_and_logs_to_the_global_log() {
    let env = Env::new();
    let run = env.run(&["--run-task", "does-not-exist"]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(run.stderr.contains("nicht gefunden"), "{}", run.dump());
    assert!(run.last_line().ends_with("(Exit-Code 2)"), "{}", run.dump());
    let global =
        std::fs::read_to_string(env.home.path().join("logs").join("openspider.log")).unwrap();
    assert!(global.contains("unknown task 'does-not-exist'"), "{global}");
    // Auch ein unzulässiger Name erreicht nie das Dateisystem.
    let bad = env.run(&["--run-task", "../etc/passwd"]);
    assert_eq!(bad.code, 2, "{}", bad.dump());
    assert!(history(&env).list(None, 0, 10).is_empty());
}

// spec: OUT-168
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invalid_task_is_called_by_the_scheduler_but_ends_at_once_with_a_history_entry() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("invalid");
    let mut t = task("broken", "Broken", &site.base_url(), &out, &TABLES[..1]);
    t.crawl.config = Some("missing-profile".into());
    t.apis = vec!["gsc".into()];
    save(&env, &t);
    let run = env
        .run_async(vec!["--run-task".to_string(), "broken".into()])
        .await;
    assert_eq!(run.code, 2, "{}", run.dump());
    assert_eq!(
        site.total_requests(),
        0,
        "ein ungültiger Lauf crawlt nichts"
    );
    assert!(!out.exists());
    let runs = history(&env).list(Some("broken"), 0, 10);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].result, RunResult::Failed);
    let message = runs[0].message.clone().unwrap();
    assert!(message.contains("tasks-issue-config-missing"), "{message}");
    assert!(message.contains("tasks-issue-api-credentials"), "{message}");
    assert!(env.crawl_files().is_empty());
}

// spec: OUT-168
// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreadable_task_file_is_a_failed_run_in_the_history() {
    let env = Env::new();
    let store = TaskStore::in_settings(env.home.path());
    std::fs::create_dir_all(store.dir()).unwrap();
    std::fs::write(store.dir().join("garbled.ostask"), "version = [").unwrap();
    let run = env.run(&["--run-task", "garbled"]);
    assert_eq!(run.code, 2, "{}", run.dump());
    let runs = history(&env).list(Some("garbled"), 0, 10);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].result, RunResult::Failed);
    assert!(runs[0].message.as_deref().unwrap().contains("not readable"));
}

// spec: OUT-167
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skip_mode_records_a_skipped_run_while_the_previous_run_is_active() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("skip");
    let mut t = task("skipper", "Skipper", &site.base_url(), &out, &TABLES[..1]);
    t.execution.overlap = Overlap::Skip;
    save(&env, &t);
    // Ein Vorlauf mit frischem Herzschlag.
    let now = os_model::Timestamp::now_utc();
    let mut running = RunRecord::start("previous-run", "skipper", "Skipper", now);
    running.pid = 1;
    history(&env).write(&running).unwrap();

    let run = env
        .run_async(vec!["--run-task".to_string(), "skipper".into()])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(site.total_requests(), 0);
    let runs = history(&env).list(Some("skipper"), 0, 10);
    let skipped = runs.iter().find(|r| r.run_id != "previous-run").unwrap();
    assert_eq!(skipped.result, RunResult::Skipped);
    assert_eq!(
        skipped.message.as_deref(),
        Some("Skipped: previous run still active")
    );

    // Läuft der Vorlauf nicht mehr (kein frischer Herzschlag), startet der neue Lauf normal.
    let mut old = RunRecord::start("previous-run", "skipper", "Skipper", now);
    old.heartbeat_at = "2020-01-01T00:00:00.000Z".into();
    history(&env).write(&old).unwrap();
    let run = env
        .run_async(vec!["--run-task".to_string(), "skipper".into()])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(site.total_requests() > 0);
    let previous = history(&env).get("skipper", "previous-run").unwrap();
    assert_eq!(previous.result, RunResult::Failed, "abgestürzter Vorlauf");
    assert_eq!(previous.message.as_deref(), Some("unexpected termination"));
}

// spec: OUT-167
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_tasks_at_the_same_time_make_two_processes_and_two_crawl_files() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("parallel");
    save(
        &env,
        &task("alpha", "Alpha", &site.base_url(), &out, &TABLES[..1]),
    );
    save(
        &env,
        &task("beta", "Beta", &site.base_url(), &out, &TABLES[..1]),
    );
    let (a, b) = tokio::join!(
        env.run_async(vec!["--run-task".to_string(), "alpha".into()]),
        env.run_async(vec!["--run-task".to_string(), "beta".into()])
    );
    assert_eq!(a.code, 0, "{}", a.dump());
    assert_eq!(b.code, 0, "{}", b.dump());
    assert_eq!(env.crawl_files().len(), 2);
    assert!(common::is_completed(
        &history(&env).list(Some("alpha"), 0, 5)[0]
    ));
    assert!(common::is_completed(
        &history(&env).list(Some("beta"), 0, 5)[0]
    ));
    // Jede Aufgabe schreibt in ihren eigenen Ordner.
    assert!(run_folder(&out, "Shop", "Alpha")
        .join("internal_all.csv")
        .is_file());
    assert!(run_folder(&out, "Shop", "Beta")
        .join("internal_all.csv")
        .is_file());
}

async fn wait_for_requests(site: &os_testsite::TestSite, env: &Env, requests: u64) {
    let started = Instant::now();
    while (env.crawl_files().is_empty() || site.total_requests() < requests)
        && started.elapsed() < Duration::from_secs(120)
    {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        site.total_requests() >= requests,
        "der Crawl kam nicht in Gang"
    );
}

fn spawn_task(env: &Env, id: &str, run_id: &str) -> std::process::Child {
    let mut cmd = env.command();
    cmd.args(["--run-task", id])
        .env("OPENSPIDER_TASK_RUN_ID", run_id)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().unwrap()
}

// spec: OUT-171
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_cancel_marker_aborts_within_ten_seconds_saves_the_crawl_and_skips_the_exports() {
    let site = site("out-queue").await;
    let env = Env::new();
    let out = env.work.path().join("abort");
    save(
        &env,
        &task("long", "Long", &site.base_url(), &out, &TABLES[..1]),
    );
    let child = spawn_task(&env, "long", "run-cancel-1");
    wait_for_requests(&site, &env, 3).await;
    // Der Lauf ist als laufend sichtbar.
    let running = history(&env)
        .get("long", "run-cancel-1")
        .expect("Lauf in der Historie");
    assert_eq!(running.result, RunResult::Running);
    assert!(running.manual, "die App gibt die Laufkennung vor");

    let asked = Instant::now();
    history(&env)
        .request_cancel("long", "run-cancel-1")
        .unwrap();
    let output = tokio::task::spawn_blocking(move || child.wait_with_output().unwrap())
        .await
        .unwrap();
    assert!(
        asked.elapsed() < Duration::from_secs(10),
        "{:?}",
        asked.elapsed()
    );
    let run = common::Run::from_output(output);
    assert_eq!(run.code, 130, "{}", run.dump());

    let record = history(&env).get("long", "run-cancel-1").unwrap();
    assert_eq!(record.result, RunResult::Aborted);
    assert_eq!(record.exit_code, Some(130));
    assert!(record.exports.is_empty());
    // Der Zeitstempel-Ordner entsteht vor dem Crawl (OUT-013); nach einem Abbruch bleibt er leer.
    assert!(
        run_folder(&out, "Shop", "Long")
            .read_dir()
            .unwrap()
            .next()
            .is_none(),
        "nach einem Abbruch folgen keine Exporte"
    );
    // Der Crawl ist gespeichert und fortsetzbar.
    let crawls = env.crawl_files();
    assert_eq!(crawls.len(), 1);
    let file = CrawlFile::open(&crawls[0], OpenMode::ReadOnly).unwrap();
    assert_eq!(file.meta().unwrap().state.as_str(), "Paused");
    assert!(
        !history(&env).cancel_requested("long", "run-cancel-1"),
        "Marke aufgeräumt"
    );
}

// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_killed_run_is_reported_as_failed_unexpected_termination() {
    let site = site("out-queue").await;
    let env = Env::new();
    let out = env.work.path().join("killed");
    save(
        &env,
        &task("crash", "Crash", &site.base_url(), &out, &TABLES[..1]),
    );
    let mut child = spawn_task(&env, "crash", "run-killed-1");
    wait_for_requests(&site, &env, 3).await;
    child.kill().unwrap();
    let _ = tokio::task::spawn_blocking(move || child.wait())
        .await
        .unwrap();

    let h = history(&env);
    assert_eq!(
        h.get("crash", "run-killed-1").unwrap().result,
        RunResult::Running
    );
    // Zwei Minuten später gilt der Herzschlag als erloschen; der nächste Start gleicht ab.
    let later = os_model::Timestamp::now_utc().plus(Duration::from_secs(120));
    let fixed = h.reconcile(later);
    assert_eq!(fixed.len(), 1);
    let record = h.get("crash", "run-killed-1").unwrap();
    assert_eq!(record.result, RunResult::Failed);
    assert_eq!(record.message.as_deref(), Some("unexpected termination"));
}

// spec: OUT-169
// spec: OUT-162
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_mode_runs_appear_in_the_history_like_spider_runs() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("list");
    let list = env.work.path().join("urls.txt");
    std::fs::write(
        &list,
        format!(
            "{0}/about.html\n{0}/contact.html\n{0}/about.html\n",
            site.base_url().trim_end_matches('/')
        ),
    )
    .unwrap();
    let mut t = task("lister", "Lister", "", &out, &TABLES[..1]);
    t.crawl.source = Source::List {
        path: list.to_string_lossy().into_owned(),
    };
    save(&env, &t);
    let run = env
        .run_async(vec!["--run-task".to_string(), "lister".into()])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let record = history(&env).last("lister").unwrap();
    assert!(common::is_completed(&record), "{record:?}");
    assert_eq!(record.urls_crawled, Some(2), "Duplikate zählen einmal");
    let folder = run_folder(&out, "Shop", "Lister");
    assert!(folder.join("internal_all.csv").is_file());
}

// spec: OUT-168
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_disabled_task_is_skipped_by_the_scheduler_but_runs_when_started_by_hand() {
    let site = site("out-basic-site").await;
    let env = Env::new();
    let out = env.work.path().join("disabled");
    let mut t = task("off", "Off", &site.base_url(), &out, &TABLES[..1]);
    t.enabled = false;
    save(&env, &t);
    let run = env
        .run_async(vec!["--run-task".to_string(), "off".into()])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert_eq!(
        history(&env).last("off").unwrap().result,
        RunResult::Skipped
    );
    assert_eq!(site.total_requests(), 0);
    let mut cmd = env.command();
    cmd.args(["--run-task", "off"])
        .env("OPENSPIDER_TASK_RUN_ID", "manual-1");
    let out_run =
        tokio::task::spawn_blocking(move || common::Run::from_output(cmd.output().unwrap()))
            .await
            .unwrap();
    assert_eq!(out_run.code, 0, "{}", out_run.dump());
    assert!(common::is_completed(
        &history(&env).get("off", "manual-1").unwrap()
    ));
}

// spec: OUT-207
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_headless_rendering_persists_dom_and_screenshot() {
    let server = site("render-r01").await;
    let env = Env::new();
    let mut task = task(
        "render-headless",
        "Render",
        &format!("{}/csr.html", server.base_url()),
        &env.out(),
        &[("tab", "Internal:All")],
    );
    for (key, value) in [
        ("rendering.mode", "javascript"),
        ("rendering.ajax_timeout_s", "0.1"),
        ("rendering.screenshots.enabled", "true"),
        ("extraction.store_rendered_html", "true"),
        ("limits.max_depth", "0"),
    ] {
        task.crawl.overrides.insert(key.into(), value.into());
    }
    save(&env, &task);
    let run = env.run_async(vec!["--run-task", "render-headless"]).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let paths = env.crawl_files();
    assert_eq!(paths.len(), 1);
    let file = CrawlFile::open(&paths[0], OpenMode::ReadOnly).unwrap();
    let (id, title): (i64, String) = file
        .conn()
        .query_row(
            "SELECT id,title_1 FROM urls WHERE url LIKE '%/csr.html'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "rendered");
    let dom = os_store::queries::stored_rendered_html(&file, os_model::UrlId(id))
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&dom.bytes).contains("JS link"));
    let screenshots: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM screenshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(screenshots, 1);
}
