//! tasks.history, tasks.openLog, tasks.export/import (OUT-169, OUT-170) und der Abgleich beim App-Start
//! (OUT-166, OUT-169). Kein Systemdienst, kein `openspider`-Prozess.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_app::commands::tasks::TasksState;
use os_app::dto::{CmdError, ErrorCode, OpState};
use os_app::runtime::hooks::DesktopActions;
use os_app::App;
use os_model::Timestamp;
use os_sched::history::{HistoryStore, RunRecord, RunResult};
use os_sched::planner::{self, Backend};
use serde_json::{json, Value};

fn def_json(out: &Path) -> Value {
    let text = include_str!("../../../tests/fixtures/out-m2-tasks/task-def-basic.json")
        .replace("{OUT}", &out.to_string_lossy().replace('\\', "\\\\"));
    serde_json::from_str(&text).expect("Fixture")
}

fn app_in(dir: &Path) -> App {
    let app = common::new_app(dir);
    app.ext::<TasksState>()
        .set_planner(planner::dry_run(Backend::Launchd, &dir.join("planner")));
    app
}

async fn call(app: &App, name: &str, args: Value) -> Result<Value, CmdError> {
    app.dispatch(common::ctx(), name, args).await
}

async fn ok(app: &App, name: &str, args: Value) -> Value {
    call(app, name, args)
        .await
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

async fn operation(app: &App, name: &str, args: Value) -> os_app::dto::OpStatus {
    let handle = ok(app, name, args).await;
    let id = handle["opId"].as_str().unwrap().to_string();
    tokio::time::timeout(Duration::from_secs(20), app.ops().wait(&id))
        .await
        .unwrap()
        .unwrap()
}

fn run(task: &str, id: &str, started: &str, result: RunResult) -> RunRecord {
    let at = Timestamp::from_db(started).unwrap();
    let mut r = RunRecord::start(id, task, "Nightly", at);
    r.finish(result, Some(0), at);
    r
}

// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_lists_runs_newest_first_in_pages_with_all_columns_and_flags_crashed_runs() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_in(dir.path());
    let id = ok(
        &app,
        "tasks.save",
        json!({ "task": def_json(&dir.path().join("x")) }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h = HistoryStore::in_settings(dir.path());
    for day in 1..=5 {
        let mut r = run(
            &id,
            &format!("run-{day}"),
            &format!("2025-09-0{day}T01:00:00.000Z"),
            RunResult::Completed,
        );
        r.crawl_id = Some(format!("crawl-{day}"));
        r.urls_crawled = Some(100 * day);
        r.output_folder = Some(format!("out/{day}"));
        r.exports = vec![format!("out/{day}/internal_all.csv")];
        h.write(&r).unwrap();
    }
    let mut warned = run(
        &id,
        "run-w",
        "2025-09-06T01:00:00.000Z",
        RunResult::CompletedWithWarnings,
    );
    warned.warnings = vec!["PSI unavailable".into(), "Drive quota exceeded".into()];
    h.write(&warned).unwrap();
    let mut failed = run(&id, "run-f", "2025-09-07T01:00:00.000Z", RunResult::Failed);
    failed.message = Some("Failed: config not found".into());
    failed.exit_code = Some(2);
    h.write(&failed).unwrap();
    let mut skipped = run(&id, "run-s", "2025-09-08T01:00:00.000Z", RunResult::Skipped);
    skipped.message = Some("Skipped: previous run still active".into());
    h.write(&skipped).unwrap();
    // Abgestürzt: laufend, aber ohne frischen Herzschlag.
    let mut dead = RunRecord::start(
        "run-dead",
        &id,
        "Nightly",
        Timestamp::from_db("2025-09-09T01:00:00.000Z").unwrap(),
    );
    dead.heartbeat_at = "2025-09-09T01:00:05.000Z".into();
    h.write(&dead).unwrap();

    let all = ok(
        &app,
        "tasks.history",
        json!({ "id": id, "offset": 0, "limit": 100 }),
    )
    .await;
    let ids: Vec<&str> = all
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["runId"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["run-dead", "run-s", "run-f", "run-w", "run-5", "run-4", "run-3", "run-2", "run-1"]
    );
    let by = |id: &str| {
        all.as_array()
            .unwrap()
            .iter()
            .find(|r| r["runId"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(by("run-dead")["state"], "failed");
    assert_eq!(by("run-dead")["error"], "unexpected termination");
    assert_eq!(by("run-s")["state"], "skipped");
    assert_eq!(by("run-s")["error"], "Skipped: previous run still active");
    assert_eq!(by("run-f")["state"], "failed");
    assert_eq!(by("run-w")["state"], "succeeded");
    assert_eq!(by("run-w")["exitCode"], 4);
    assert_eq!(
        by("run-w")["error"],
        "PSI unavailable; Drive quota exceeded"
    );
    let five = by("run-5");
    assert_eq!(five["crawlId"], "crawl-5");
    assert_eq!(five["urlsCrawled"], 500);
    assert_eq!(five["outputFolder"], "out/5");
    assert_eq!(five["exports"], json!(["out/5/internal_all.csv"]));
    assert_eq!(five["exitCode"], 0);

    // Seiten: offset/limit.
    let page = ok(
        &app,
        "tasks.history",
        json!({ "id": id, "offset": 2, "limit": 3 }),
    )
    .await;
    let ids: Vec<&str> = page
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["runId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["run-f", "run-w", "run-5"]);
    // Ohne Aufgabe: alle Aufgaben.
    let everything = ok(
        &app,
        "tasks.history",
        json!({ "id": null, "offset": 0, "limit": 2 }),
    )
    .await;
    assert_eq!(everything.as_array().unwrap().len(), 2);
}

#[derive(Default)]
struct Revealed(Mutex<Vec<PathBuf>>);

impl DesktopActions for Revealed {
    fn write_clipboard(&self, _: &str) -> Result<(), CmdError> {
        Ok(())
    }
    fn reveal_paths(&self, p: &[PathBuf]) -> Result<(), CmdError> {
        self.0.lock().unwrap().extend(p.iter().cloned());
        Ok(())
    }
}

// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_log_reveals_the_log_of_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_in(dir.path());
    let id = ok(
        &app,
        "tasks.save",
        json!({ "task": def_json(&dir.path().join("x")) }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let log = dir.path().join("logs").join("task-x.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, "line").unwrap();
    let mut r = run(
        &id,
        "run-log",
        "2025-09-01T01:00:00.000Z",
        RunResult::Completed,
    );
    r.log_path = Some(log.to_string_lossy().into_owned());
    HistoryStore::in_settings(dir.path()).write(&r).unwrap();
    let revealed = Arc::new(Revealed::default());
    app.set_desktop_actions(revealed.clone());
    ok(&app, "tasks.openLog", json!({ "id": "run-log" })).await;
    assert_eq!(revealed.0.lock().unwrap().as_slice(), [log]);
    let err = call(&app, "tasks.openLog", json!({ "id": "unknown-run" }))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

// spec: OUT-170
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_and_import_roundtrip_without_secrets_and_with_missing_dependencies_reported() {
    let a = tempfile::tempdir().unwrap();
    let app_a = app_in(a.path());
    let mut def = def_json(&a.path().join("out"));
    def["crawl"]["profile"] = json!("shop");
    // Konfigurationsprofil, das zur Aufgabe gehört.
    std::fs::create_dir_all(a.path().join("profiles")).unwrap();
    std::fs::write(
        a.path().join("profiles").join("shop.osconfig"),
        "[speed]\nthreads = 3\n",
    )
    .unwrap();
    let id = ok(&app_a, "tasks.save", json!({ "task": def })).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Nur Text: eine .ostask-Datei.
    let file = a.path().join("export").join("tasks.ostask");
    let status = operation(
        &app_a,
        "tasks.export",
        json!({ "ids": [id], "path": file.to_string_lossy() }),
    )
    .await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    assert_eq!(
        status.result.as_ref().unwrap()["path"],
        file.to_string_lossy().as_ref()
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        !text.contains("geheim-proxy-kennwort"),
        "Geheimnis im Export:\n{text}"
    );
    assert!(text.contains("speed.threads"));

    // Auf einem anderen Rechner: Aufgabe erscheint, Profil fehlt -> ungültig mit konkretem Grund.
    let b = tempfile::tempdir().unwrap();
    let app_b = app_in(b.path());
    let status = operation(
        &app_b,
        "tasks.import",
        json!({ "path": file.to_string_lossy() }),
    )
    .await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let imported = status.result.unwrap();
    assert_eq!(imported.as_array().unwrap().len(), 1);
    let imported_id = imported[0]["id"].as_str().unwrap().to_string();
    assert_eq!(imported[0]["name"], "Nightly");
    assert_eq!(
        imported[0]["registered"], false,
        "ungültige Aufgaben werden nicht eingetragen"
    );
    let task = ok(&app_b, "tasks.get", json!({ "id": imported_id })).await;
    let issues = ok(&app_b, "tasks.validate", json!({ "task": task })).await;
    assert_eq!(issues[0]["messageKey"], "tasks-issue-config-missing");
    assert_eq!(issues[0]["args"]["config"], "shop");
    assert_eq!(
        ok(&app_b, "tasks.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

// spec: OUT-170
// spec: OUT-166
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_zip_carries_the_config_profile_and_a_valid_import_is_registered() {
    let a = tempfile::tempdir().unwrap();
    let app_a = app_in(a.path());
    let mut def = def_json(&a.path().join("out"));
    def["crawl"]["profile"] = json!("shop");
    std::fs::create_dir_all(a.path().join("profiles")).unwrap();
    std::fs::write(
        a.path().join("profiles").join("shop.osconfig"),
        "[speed]\nthreads = 3\n",
    )
    .unwrap();
    let id = ok(&app_a, "tasks.save", json!({ "task": def })).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let file = a.path().join("tasks.zip");
    let status = operation(
        &app_a,
        "tasks.export",
        json!({ "ids": [id], "path": file.to_string_lossy() }),
    )
    .await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    assert!(std::fs::read(&file).unwrap().starts_with(b"PK"));

    let b = tempfile::tempdir().unwrap();
    // Die Ausgabeordner der Aufgabe existieren auch auf dem zweiten Rechner (Pfade sind absolut).
    let app_b = app_in(b.path());
    let status = operation(
        &app_b,
        "tasks.import",
        json!({ "path": file.to_string_lossy() }),
    )
    .await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let imported = status.result.unwrap();
    // Mitgepacktes Profil wurde übernommen, daher ist die Aufgabe gültig und eingetragen.
    assert!(b.path().join("profiles").join("shop.osconfig").is_file());
    assert_eq!(imported[0]["registered"], true, "{imported}");
    let imported_id = imported[0]["id"].as_str().unwrap();
    assert!(b
        .path()
        .join("planner")
        .join(format!("de.openspider.task.{imported_id}.plist"))
        .is_file());
    // Unlesbare Dateien sind ein Fehler der Operation, keine Aufgabe entsteht.
    let broken = b.path().join("broken.ostask");
    std::fs::write(&broken, "version = [").unwrap();
    let status = operation(
        &app_b,
        "tasks.import",
        json!({ "path": broken.to_string_lossy() }),
    )
    .await;
    assert_eq!(status.state, OpState::Failed);
    assert_eq!(
        ok(&app_b, "tasks.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

// spec: OUT-166
// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn at_app_start_registrations_pointing_to_an_old_binary_are_renewed_and_crashed_runs_are_failed(
) {
    let dir = tempfile::tempdir().unwrap();
    let app = app_in(dir.path());
    let id = ok(
        &app,
        "tasks.save",
        json!({ "task": def_json(&dir.path().join("x")) }),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Eintrag, der noch auf ein altes Programm zeigt.
    let planner = planner::dry_run(Backend::Launchd, &dir.path().join("planner"));
    let task = os_sched::tasks::TaskStore::in_settings(dir.path())
        .get(&id)
        .unwrap();
    planner
        .install(&os_sched::planner::PlannerSpec {
            task_id: id.clone(),
            task_name: task.name.clone(),
            schedule: task.schedule.clone(),
            binary: PathBuf::from("/old/OpenSpider.app/Contents/MacOS/openspider"),
            home: None,
            run_if_missed: true,
            on_battery: true,
            wake: false,
        })
        .unwrap();
    let mut dead = RunRecord::start(
        "run-dead",
        &id,
        "Nightly",
        Timestamp::from_db("2025-09-09T01:00:00.000Z").unwrap(),
    );
    dead.heartbeat_at = "2025-09-09T01:00:05.000Z".into();
    HistoryStore::in_settings(dir.path()).write(&dead).unwrap();

    os_app::commands::tasks::on_startup(&app);
    let current = planner::cli_binary();
    assert!(
        common::eventually(Duration::from_secs(10), || {
            planner.registered_binary(&id).as_deref() == Some(current.as_path())
        })
        .await,
        "Eintrag zeigt auf {:?}",
        planner.registered_binary(&id)
    );
    assert!(
        common::eventually(Duration::from_secs(10), || {
            HistoryStore::in_settings(dir.path())
                .get(&id, "run-dead")
                .is_some_and(|r| r.result == RunResult::Failed)
        })
        .await
    );
}
