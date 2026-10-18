use os_app::diag;
use os_app::{App, AppConfig, CallCtx, ErrorCode};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    time::{Duration, SystemTime},
};

#[derive(Default)]
struct Desktop(std::sync::Mutex<Vec<std::path::PathBuf>>);
impl os_app::runtime::hooks::DesktopActions for Desktop {
    fn write_clipboard(&self, _: &str) -> Result<(), os_app::CmdError> {
        Ok(())
    }
    fn reveal_paths(&self, paths: &[std::path::PathBuf]) -> Result<(), os_app::CmdError> {
        *self.0.lock().unwrap() = paths.to_vec();
        Ok(())
    }
}

// spec: OUT-244
// spec: OUT-243
#[test]
fn zip_only_contains_recent_logs_and_redacted_metadata_and_cancels_atomically() {
    let home = tempfile::tempdir().unwrap();
    let logs = home.path().join("logs");
    fs::create_dir(&logs).unwrap();
    fs::write(logs.join("openspider.log"),"Authorization: Bearer AUTH-FIXTURE\n{\"Cookie\":\"COOKIE-FIXTURE\",\"url\":\"https://u:PASS-FIXTURE@fixture.test/?token=TOKEN-FIXTURE\"}\n").unwrap();
    fs::write(logs.join("cli-old.log"), "OLD").unwrap();
    fs::File::options()
        .write(true)
        .open(logs.join("cli-old.log"))
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(8 * 86400))
        .unwrap();
    fs::write(logs.join("crawl.ospider"), "CRAWL-FIXTURE").unwrap();
    let entries = diag::bundle_entries(
        &logs,
        json!({"os":"fixture"}),
        json!({"password":"CONFIG-FIXTURE"}),
        json!([{"name":"task","passphrase":"TASK-FIXTURE"}]),
        SystemTime::now(),
    )
    .unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.0.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "system.txt",
            "config-diff.json",
            "tasks.json",
            "logs/openspider.log"
        ]
    );
    let dest = home.path().join("bundle.zip");
    assert_eq!(
        diag::write_bundle(&dest, &entries, || true)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    assert!(!dest.exists());
    diag::write_bundle(&dest, &entries, || false).unwrap();
    let mut zip = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
    for i in 0..zip.len() {
        let mut raw = String::new();
        zip.by_index(i).unwrap().read_to_string(&mut raw).unwrap();
        assert!(!raw.contains("FIXTURE"), "{raw}");
    }
    assert!(
        diag::write_bundle(&dest, &entries, || false).is_err(),
        "existing output is protected"
    );
    assert!(!fs::read_dir(home.path()).unwrap().any(|e| e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|e| e == "tmp")));
}
// spec: OUT-244
// spec: OUT-227 [M2]
#[tokio::test]
async fn preview_matches_snapshot_and_bundle_is_an_operation_with_effective_debug_paths() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    fs::create_dir_all(home.path().join("logs")).unwrap();
    fs::write(home.path().join("logs/openspider.log"), "fixture log").unwrap();
    os_app::commands::diag::set_task_provider(
        &app,
        std::sync::Arc::new(|| json!([{"name":"fixture-task","password":"TASK-FIXTURE"}])),
    );
    app.config()
        .secrets
        .set("api/gsc", "INTEGRATION-FIXTURE")
        .unwrap();
    let ctx = || CallCtx::for_window("main");
    let desktop = std::sync::Arc::new(Desktop::default());
    app.set_desktop_actions(desktop.clone());
    app.dispatch(ctx(), "diag.openLogs", Value::Null)
        .await
        .unwrap();
    assert_eq!(*desktop.0.lock().unwrap(), vec![home.path().join("logs")]);
    app.dispatch(
        ctx(),
        "diag.logFrontend",
        json!({
            "level":"info", "message":"password=FRONTEND-FIXTURE",
            "context":{"Authorization":"Bearer AUTH-FIXTURE","Cookie":"COOKIE-FIXTURE"}
        }),
    )
    .await
    .unwrap();
    let frontend_log = fs::read_to_string(home.path().join("logs/openspider.log")).unwrap();
    assert!(frontend_log.contains("frontend"));
    assert!(!frontend_log.contains("FIXTURE"));
    let debug = app
        .dispatch(ctx(), "diag.debugInfo", Value::Null)
        .await
        .unwrap();
    assert!(!debug.to_string().contains("INTEGRATION-FIXTURE"));
    let connected = debug["extra"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "integrations")
        .unwrap();
    assert_eq!(connected["value"]["gsc"], true);
    assert_eq!(
        debug["app"]["paths"]["logs"],
        home.path().join("logs").to_string_lossy().as_ref()
    );
    assert!(debug["extra"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "cpuCores"));
    let preview = app
        .dispatch(ctx(), "diag.previewBundle", Value::Null)
        .await
        .unwrap();
    fs::write(
        home.path().join("logs/cli-new.log"),
        "created after preview",
    )
    .unwrap();
    let dest = home.path().join("out.zip");
    let handle = app
        .dispatch(ctx(), "diag.bundle", json!({"dest":dest}))
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        app.ops().wait(handle["opId"].as_str().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert!(dest.is_file());
    let mut zip = zip::ZipArchive::new(fs::File::open(dest).unwrap()).unwrap();
    let actual: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect();
    assert_eq!(serde_json::to_value(actual).unwrap(), preview["files"]);
    let mut tasks = String::new();
    zip.by_name("tasks.json")
        .unwrap()
        .read_to_string(&mut tasks)
        .unwrap();
    assert!(tasks.contains("fixture-task"));
    assert!(!tasks.contains("TASK-FIXTURE"));
    let err = app
        .dispatch(
            ctx(),
            "diag.logFrontend",
            json!({"level":"invalid","message":"hello"}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
}
