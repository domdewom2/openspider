//! tasks.* (OUT-160 … OUT-171): Verwalten, Eintragen beim OS-Planer (Dry-Run), Ausführen über einen Fake-Starter,
//! Abbruch, Historie, Export/Import. Es wird weder ein Systemdienst registriert noch `openspider` gestartet.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_app::commands::tasks::{LaunchRequest, RunningChild, TaskLauncher, TasksState};
use os_app::dto::OpState;
use os_app::{App, ErrorCode};
use os_sched::history::{HistoryStore, RunRecord, RunResult};
use os_sched::planner::{self, Backend};
use os_sched::tasks::EuZone;
use serde_json::{json, Value};

fn def_json(out: &Path) -> Value {
    let text = include_str!("../../../tests/fixtures/out-m2-tasks/task-def-basic.json")
        .replace("{OUT}", &out.to_string_lossy().replace('\\', "\\\\"));
    serde_json::from_str(&text).expect("Fixture")
}

struct Setup {
    dir: tempfile::TempDir,
    app: App,
    planner_dir: PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let planner_dir = dir.path().join("planner");
    let state = app.ext::<TasksState>();
    state.set_planner(planner::dry_run(Backend::Launchd, &planner_dir));
    state.set_zone(Arc::new(EuZone::central_europe()));
    Setup {
        dir,
        app,
        planner_dir,
    }
}

async fn call(app: &App, name: &str, args: Value) -> Result<Value, os_app::dto::CmdError> {
    app.dispatch(common::ctx(), name, args).await
}

async fn ok(app: &App, name: &str, args: Value) -> Value {
    call(app, name, args)
        .await
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

async fn operation(app: &App, name: &str, args: Value) -> os_app::dto::OpStatus {
    let handle = ok(app, name, args).await;
    let id = handle["opId"].as_str().expect("OpHandle").to_string();
    tokio::time::timeout(Duration::from_secs(20), app.ops().wait(&id))
        .await
        .expect("Operation endet")
        .expect("Operation bekannt")
}

fn plist(setup: &Setup, id: &str) -> PathBuf {
    setup
        .planner_dir
        .join(format!("de.openspider.task.{id}.plist"))
}

// spec: OUT-160
// spec: OUT-161
// spec: OUT-168
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_task_is_validated_saved_listed_edited_and_deleted_without_touching_exports() {
    let s = setup();
    let out = s.dir.path().join("exports");
    let def = def_json(&out);

    // Gültig: keine Befunde.
    let issues = ok(&s.app, "tasks.validate", json!({ "task": def })).await;
    assert_eq!(issues, json!([]), "{issues}");

    let info = ok(&s.app, "tasks.save", json!({ "task": def })).await;
    let id = info["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("nightly-"), "{id}");
    assert_eq!(info["name"], "Nightly");
    assert_eq!(
        info["project"], "Nightly",
        "Projektname = Taskname, wenn leer"
    );
    assert_eq!(info["registered"], false);
    // ManualClock = 2025-10-09T08:53:20Z; täglich 03:00 Berlin (MESZ) = 01:00 UTC am nächsten Tag.
    assert_eq!(info["nextRun"], "2025-10-10T01:00:00.000Z");

    let list = ok(&s.app, "tasks.list", Value::Null).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], id);
    assert_eq!(list[0]["lastRun"], Value::Null);

    // `tasks.get` liefert das Dialogmodell zurück: Überschreibungen als Zahlen, Projekt gesetzt.
    let got = ok(&s.app, "tasks.get", json!({ "id": id })).await;
    assert_eq!(got["crawl"]["overrides"]["speed.threads"], 2);
    assert_eq!(got["exportOptions"]["skipEmpty"], true);
    assert_eq!(got["exports"][0]["kind"], "sitemap");

    // Ein zweiter Name ist nicht erlaubt (Groß-/Kleinschreibung egal), die Validierung nennt den Grund.
    let mut second = def.clone();
    second["name"] = json!("NIGHTLY");
    let issues = ok(&s.app, "tasks.validate", json!({ "task": second })).await;
    assert_eq!(issues[0]["messageKey"], "tasks-issue-name-taken");
    let err = call(&s.app, "tasks.save", json!({ "task": second }))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);

    // Bearbeiten über das Dialogmodell lässt alles andere, was der Dialog nicht zeigt, unverändert.
    let mut edited = got.clone();
    edited["schedule"] = json!({ "kind": "weekly", "days": [1, 3], "time": "06:00" });
    edited["execution"]["overlap"] = json!("skip");
    let saved = ok(&s.app, "tasks.save", json!({ "task": edited })).await;
    assert_eq!(saved["id"], id);
    assert_eq!(saved["schedule"]["kind"], "weekly");
    let again = ok(&s.app, "tasks.get", json!({ "id": id })).await;
    assert_eq!(again["execution"]["overlap"], "skip");
    assert_eq!(again["name"], "Nightly");
    assert_eq!(
        ok(&s.app, "tasks.list", Value::Null)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Löschen: Datei weg, Exporte bleiben.
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("internal_all.csv"), "x").unwrap();
    ok(&s.app, "tasks.delete", json!({ "id": id })).await;
    assert_eq!(ok(&s.app, "tasks.list", Value::Null).await, json!([]));
    assert!(out.join("internal_all.csv").is_file());
    let err = call(&s.app, "tasks.get", json!({ "id": id }))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

// spec: OUT-168
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_tasks_are_reported_with_reasons_and_not_saved() {
    let s = setup();
    let out = s.dir.path().join("exports");
    let mut def = def_json(&out);
    def["crawl"]["source"] = json!({ "kind": "spider", "url": "not a url" });
    def["crawl"]["profile"] = json!("missing");
    def["apis"] = json!(["gsc"]);
    def["notify"] = json!({ "onSuccess": true, "onFailure": false, "account": "nobody", "to": [] });
    let issues = ok(&s.app, "tasks.validate", json!({ "task": def })).await;
    let keys: Vec<&str> = issues
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["messageKey"].as_str().unwrap())
        .collect();
    for expected in [
        "tasks-issue-start-url-invalid",
        "tasks-issue-config-missing",
        "tasks-issue-api-credentials",
        "tasks-issue-mail-account-missing",
        "tasks-issue-mail-recipients-missing",
    ] {
        assert!(keys.contains(&expected), "{expected} in {keys:?}");
    }
    assert!(issues
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["warning"] == false));
    let err = call(&s.app, "tasks.save", json!({ "task": def }))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(ok(&s.app, "tasks.list", Value::Null).await, json!([]));
    // Zugangsdaten im Schlüsselbund machen die Integration gültig.
    s.app.config().secrets.set("api/gsc", "{}").unwrap();
    let issues = ok(&s.app, "tasks.validate", json!({ "task": def })).await;
    assert!(!issues
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["messageKey"] == "tasks-issue-api-credentials"));
    // Ein nicht darstellbares Format wird als Befund gemeldet.
    let mut json_format = def_json(&out);
    json_format["exportOptions"]["format"] = json!("json");
    let issues = ok(&s.app, "tasks.validate", json!({ "task": json_format })).await;
    assert_eq!(issues[0]["messageKey"], "tasks-issue-format-unsupported");
}

// spec: OUT-166
// spec: OUT-160
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn register_previews_without_writing_installs_updates_and_unregisters() {
    let s = setup();
    let def = def_json(&s.dir.path().join("exports"));
    let id = ok(&s.app, "tasks.save", json!({ "task": def })).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let preview = ok(
        &s.app,
        "tasks.register",
        json!({ "id": id, "dryRun": true }),
    )
    .await;
    assert_eq!(preview["backend"], "launchd");
    let content = preview["files"][0]["content"].as_str().unwrap();
    assert!(content.contains("--run-task"), "{content}");
    assert!(content.contains(&id));
    assert_eq!(preview["command"][0], "launchctl");
    assert!(!plist(&s, &id).exists(), "Vorschau schreibt nichts");
    assert_eq!(
        ok(&s.app, "tasks.list", Value::Null).await[0]["registered"],
        false
    );

    ok(
        &s.app,
        "tasks.register",
        json!({ "id": id, "dryRun": false }),
    )
    .await;
    assert!(plist(&s, &id).is_file());
    assert_eq!(
        ok(&s.app, "tasks.list", Value::Null).await[0]["registered"],
        true
    );

    // Bearbeiten einer eingetragenen Aufgabe zieht den Eintrag nach (neuer Zeitplan).
    let mut got = ok(&s.app, "tasks.get", json!({ "id": id })).await;
    got["schedule"] = json!({ "kind": "daily", "time": "04:15" });
    ok(&s.app, "tasks.save", json!({ "task": got })).await;
    let text = std::fs::read_to_string(plist(&s, &id)).unwrap();
    assert!(
        text.contains("<integer>4</integer>") && text.contains("<integer>15</integer>"),
        "{text}"
    );

    ok(&s.app, "tasks.unregister", json!({ "id": id })).await;
    assert!(!plist(&s, &id).exists());
    // Löschen trägt ebenfalls aus.
    ok(
        &s.app,
        "tasks.register",
        json!({ "id": id, "dryRun": false }),
    )
    .await;
    ok(&s.app, "tasks.delete", json!({ "id": id })).await;
    assert!(!plist(&s, &id).exists());
    let err = call(
        &s.app,
        "tasks.register",
        json!({ "id": id, "dryRun": true }),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

// spec: OUT-166
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_disabled_task_is_not_registered_on_save_and_all_entries_go_on_uninstall() {
    let s = setup();
    let def = def_json(&s.dir.path().join("exports"));
    let id = ok(&s.app, "tasks.save", json!({ "task": def })).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    ok(
        &s.app,
        "tasks.register",
        json!({ "id": id, "dryRun": false }),
    )
    .await;
    // Ausschalten entfernt den Eintrag beim Speichern.
    let mut got = ok(&s.app, "tasks.get", json!({ "id": id })).await;
    got["enabled"] = json!(false);
    let info = ok(&s.app, "tasks.save", json!({ "task": got })).await;
    assert_eq!(info["registered"], false);
    assert_eq!(
        info["nextRun"],
        Value::Null,
        "ausgeschaltet: kein nächster Lauf"
    );
    assert!(!plist(&s, &id).exists());

    // Deinstallation: alle Einträge weg.
    let planner = planner::dry_run(Backend::Launchd, &s.planner_dir);
    for n in ["a1", "b2", "c3"] {
        planner
            .install(&os_sched::planner::PlannerSpec {
                task_id: n.into(),
                task_name: n.into(),
                schedule: os_sched::tasks::Schedule::Daily {
                    time: "03:00".into(),
                    from: None,
                },
                binary: PathBuf::from("/x/openspider"),
                home: None,
                run_if_missed: true,
                on_battery: true,
                wake: false,
            })
            .unwrap();
    }
    assert_eq!(planner.registered_ids().len(), 3);
    assert_eq!(os_app::commands::tasks::unregister_all(planner.as_ref()), 3);
    assert!(planner.registered_ids().is_empty());
}

/// Fake für den Prozess von `openspider --run-task`: schreibt wie das Binary Beginn und Ende in die Historie.
struct FakeLauncher {
    requests: Mutex<Vec<LaunchRequest>>,
    /// Beendet sich selbst mit diesem Ergebnis; `None` = läuft, bis die Abbruchmarke erscheint.
    finish_with: Option<RunResult>,
}

struct FakeChild {
    done: Arc<std::sync::atomic::AtomicBool>,
}

impl RunningChild for FakeChild {
    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        Ok(self
            .done
            .load(std::sync::atomic::Ordering::SeqCst)
            .then_some(0))
    }
    fn kill(&mut self) -> std::io::Result<()> {
        self.done.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

impl TaskLauncher for FakeLauncher {
    fn launch(&self, request: LaunchRequest) -> std::io::Result<Box<dyn RunningChild>> {
        self.requests.lock().unwrap().push(request.clone());
        let history = HistoryStore::in_settings(&request.home);
        let now = common::created_at();
        let mut record = RunRecord::start(&request.run_id, &request.task_id, "Nightly", now);
        record.heartbeat_at = os_model::Timestamp::now_utc().to_db();
        history.write(&record).unwrap();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&done);
        let finish_with = self.finish_with;
        std::thread::spawn(move || {
            let mut record = record;
            let started = std::time::Instant::now();
            loop {
                if let Some(result) = finish_with {
                    std::thread::sleep(Duration::from_millis(100));
                    record.crawl_id = Some("crawl-1".into());
                    record.urls_crawled = Some(42);
                    record.warnings = vec!["PSI unavailable".into()];
                    record.finish(result, Some(0), now);
                    history.write(&record).unwrap();
                    break;
                }
                if history.cancel_requested(&record.task_id, &record.run_id) {
                    record.finish(RunResult::Aborted, Some(130), now);
                    history.write(&record).unwrap();
                    history.clear_cancel(&record.task_id, &record.run_id);
                    break;
                }
                if started.elapsed() > Duration::from_secs(30) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        Ok(Box::new(FakeChild { done }))
    }
}

async fn saved_id(s: &Setup) -> String {
    let def = def_json(&s.dir.path().join("exports"));
    ok(&s.app, "tasks.save", json!({ "task": def })).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

// spec: OUT-160
// spec: OUT-163
// spec: OUT-169
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_now_starts_the_scheduler_entry_point_as_its_own_process_and_reports_the_run() {
    let s = setup();
    let id = saved_id(&s).await;
    let launcher = Arc::new(FakeLauncher {
        requests: Mutex::new(Vec::new()),
        finish_with: Some(RunResult::CompletedWithWarnings),
    });
    s.app.ext::<TasksState>().set_launcher(launcher.clone());
    let status = operation(&s.app, "tasks.run", json!({ "id": id })).await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let row = status.result.unwrap();
    assert_eq!(row["taskId"], id);
    assert_eq!(row["state"], "succeeded");
    assert_eq!(row["exitCode"], 4, "Completed with warnings");
    assert_eq!(row["error"], "PSI unavailable");
    assert_eq!(row["urlsCrawled"], 42);
    let request = launcher.requests.lock().unwrap()[0].clone();
    assert_eq!(request.task_id, id);
    assert_eq!(request.home, s.dir.path());
    assert_eq!(row["runId"], request.run_id.as_str());

    // Der nächste geplante Lauf bleibt unverändert; die Historie führt den Lauf.
    let list = ok(&s.app, "tasks.list", Value::Null).await;
    assert_eq!(list[0]["nextRun"], "2025-10-10T01:00:00.000Z");
    assert_eq!(list[0]["lastRun"]["runId"], request.run_id.as_str());
    let history = ok(
        &s.app,
        "tasks.history",
        json!({ "id": id, "offset": 0, "limit": 100 }),
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 1);
}

// spec: OUT-171
// spec: CRAWL-136
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn op_cancel_aborts_a_running_run_through_the_marker_and_there_is_no_pause() {
    let s = setup();
    let id = saved_id(&s).await;
    s.app
        .ext::<TasksState>()
        .set_launcher(Arc::new(FakeLauncher {
            requests: Mutex::new(Vec::new()),
            finish_with: None,
        }));
    let handle = ok(&s.app, "tasks.run", json!({ "id": id })).await;
    assert_eq!(handle["cancellable"], true);
    let op_id = handle["opId"].as_str().unwrap().to_string();
    // Läuft: Historie zeigt `running` mit derselben Operation.
    assert!(
        common::eventually(Duration::from_secs(5), || {
            HistoryStore::in_settings(s.dir.path()).last(&id).is_some()
        })
        .await
    );
    let running = ok(&s.app, "tasks.list", Value::Null).await;
    assert_eq!(running[0]["lastRun"]["state"], "running");
    assert_eq!(running[0]["lastRun"]["operation"]["opId"], op_id.as_str());

    let asked = std::time::Instant::now();
    ok(&s.app, "op.cancel", json!({ "opId": op_id })).await;
    let status = tokio::time::timeout(Duration::from_secs(10), s.app.ops().wait(&op_id))
        .await
        .unwrap()
        .unwrap();
    assert!(asked.elapsed() < Duration::from_secs(10));
    assert_eq!(status.state, OpState::Cancelled);
    let last = HistoryStore::in_settings(s.dir.path()).last(&id).unwrap();
    assert_eq!(last.result, RunResult::Aborted);
    assert!(last.exports.is_empty());

    // Parität: Es gibt nur Abbrechen, kein Pausieren oder Fortsetzen.
    let names: Vec<String> = s
        .app
        .registry()
        .manifest()
        .iter()
        .map(|m| m.name.to_string())
        .filter(|n| n.starts_with("tasks."))
        .collect();
    assert!(
        names
            .iter()
            .all(|n| !n.contains("pause") && !n.contains("resume")),
        "{names:?}"
    );
    assert_eq!(names.len(), 12, "{names:?}");
}

// spec: OUT-171
// spec: OUT-163
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runs_started_by_the_scheduler_get_an_operation_and_quitting_the_app_leaves_them_running() {
    let s = setup();
    let id = saved_id(&s).await;
    let history = HistoryStore::in_settings(s.dir.path());
    let mut record = RunRecord::start("sched-run-1", &id, "Nightly", common::created_at());
    record.heartbeat_at = os_model::Timestamp::now_utc().to_db();
    history.write(&record).unwrap();
    let list = ok(&s.app, "tasks.list", Value::Null).await;
    let op_id = list[0]["lastRun"]["operation"]["opId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(list[0]["lastRun"]["operation"]["cancellable"], true);
    // Dieselbe Operation für denselben Lauf, auch über die Historie.
    let rows = ok(
        &s.app,
        "tasks.history",
        json!({ "id": id, "offset": 0, "limit": 10 }),
    )
    .await;
    assert_eq!(rows[0]["operation"]["opId"], op_id.as_str());

    // Beenden der App bricht den Lauf des Planers nicht ab.
    s.app.ops().cancel_all();
    let status = tokio::time::timeout(Duration::from_secs(5), s.app.ops().wait(&op_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, OpState::Cancelled);
    assert!(
        !history.cancel_requested(&id, "sched-run-1"),
        "keine Abbruchmarke beim Beenden"
    );
    assert_eq!(
        history.get(&id, "sched-run-1").unwrap().result,
        RunResult::Running
    );
}

// spec: OUT-171
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aborting_a_scheduler_run_writes_the_marker() {
    let s = setup();
    let id = saved_id(&s).await;
    let history = HistoryStore::in_settings(s.dir.path());
    let mut record = RunRecord::start("sched-run-2", &id, "Nightly", common::created_at());
    record.heartbeat_at = os_model::Timestamp::now_utc().to_db();
    history.write(&record).unwrap();
    let list = ok(&s.app, "tasks.list", Value::Null).await;
    let op_id = list[0]["lastRun"]["operation"]["opId"]
        .as_str()
        .unwrap()
        .to_string();
    ok(&s.app, "op.cancel", json!({ "opId": op_id })).await;
    assert!(
        common::eventually(Duration::from_secs(5), || history
            .cancel_requested(&id, "sched-run-2"))
        .await,
        "Abbruchmarke erwartet"
    );
    // Der Lauf beendet sich wie das Binary: Aborted, Marke weg.
    let mut done = history.get(&id, "sched-run-2").unwrap();
    done.finish(RunResult::Aborted, Some(130), common::created_at());
    history.write(&done).unwrap();
    let status = tokio::time::timeout(Duration::from_secs(5), s.app.ops().wait(&op_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, OpState::Cancelled);
}
