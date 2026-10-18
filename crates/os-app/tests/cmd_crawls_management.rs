//! M2-O02: echte Dispatcher-Aufrufe gegen generierte Crawl-Dateien und Desktop-Aktionsadapter.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::dto::{CmdError, OpState};
use os_app::{
    pool::CrawlKey,
    runtime::{hooks::DesktopActions, sessions::SessionInfo},
    App, CallCtx, ErrorCode, WindowId,
};
use os_model::{CrawlConfig, CrawlId, CrawlState, IdSource, SeededIds};
use os_store::{registry, CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

fn crawl(app: &App, seed: u64, state: CrawlState) -> (String, PathBuf) {
    let id = SeededIds::new(seed)
        .new_crawl_id(common::created_at())
        .to_string();
    let dir = &app.config().paths.crawls;
    std::fs::create_dir_all(dir).unwrap();
    let path = common::make_crawl(dir, &format!("{id}.ospider"));
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("crawl_id", &id).unwrap();
    file.set_meta("state", state.as_str()).unwrap();
    file.set_meta("heartbeat_at", &common::created_at().to_db())
        .unwrap();
    (id, path)
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
async fn operation(app: &App, name: &str, args: Value) -> os_app::dto::OpStatus {
    let handle = call(app, name, args).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        app.ops().wait(handle["opId"].as_str().unwrap()),
    )
    .await
    .unwrap()
    .unwrap()
}
async fn success(app: &App, name: &str, args: Value) -> Value {
    let status = operation(app, name, args).await;
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    status.result.unwrap()
}

#[derive(Default)]
struct Desktop {
    text: Mutex<String>,
    paths: Mutex<Vec<PathBuf>>,
}
impl DesktopActions for Desktop {
    fn write_clipboard(&self, text: &str) -> Result<(), CmdError> {
        *self.text.lock().unwrap() = text.into();
        Ok(())
    }
    fn reveal_paths(&self, paths: &[PathBuf]) -> Result<(), CmdError> {
        *self.paths.lock().unwrap() = paths.to_vec();
        Ok(())
    }
}

// spec: CRAWL-143
#[tokio::test]
async fn copied_ids_keep_selection_order_and_reveal_uses_actual_paths() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (a, pa) = crawl(&app, 101, CrawlState::Paused);
    let (b, pb) = crawl(&app, 102, CrawlState::Completed);
    let desktop = Arc::new(Desktop::default());
    app.set_desktop_actions(desktop.clone());
    call(&app, "crawls.copyIds", json!({"ids":[b,a]})).await;
    assert_eq!(*desktop.text.lock().unwrap(), format!("{b}\n{a}"));
    call(&app, "crawls.copyIds", json!({"ids":[a]})).await;
    assert_eq!(*desktop.text.lock().unwrap(), a);
    call(&app, "crawls.reveal", json!({"ids":[b,a]})).await;
    assert_eq!(*desktop.paths.lock().unwrap(), [pb, pa]);
}

// spec: OUT-228
// spec: UI-124
#[tokio::test]
async fn list_search_sort_projects_and_locks_are_combined() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (a, _) = crawl(&app, 103, CrawlState::Paused);
    let (b, _) = crawl(&app, 104, CrawlState::Completed);
    let project = call(&app, "projects.create", json!({"name":"Kunde Ä"})).await;
    success(
        &app,
        "crawls.move",
        json!({"ids":[a],"project":project["id"]}),
    )
    .await;
    call(
        &app,
        "crawls.rename",
        json!({"crawlId":a,"name":"Eigener Name"}),
    )
    .await;
    call(
        &app,
        "projects.lock",
        json!({"id":project["id"],"locked":true}),
    )
    .await;
    let list = call(
        &app,
        "crawls.list",
        json!({"show":"locked","search":"KUNDE ä","sort":{"column":"name","dir":"asc"}}),
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["crawl"]["crawlId"], a);
    assert_eq!(list[0]["canResume"], true);
    assert_eq!(list[0]["crawl"]["name"], "Eigener Name");
    assert_eq!(list[0]["project"], "Kunde Ä");
    let rename = app
        .dispatch(
            common::ctx(),
            "crawls.rename",
            json!({"crawlId":a,"name":"Verboten"}),
        )
        .await;
    assert!(rename.is_err());
    let delete = operation(&app, "crawls.delete", json!({"ids":[b,a]})).await;
    assert_eq!(delete.error.unwrap().code, ErrorCode::Permission);
    assert_eq!(
        call(&app, "crawls.list", json!({"show":"crawls"}))
            .await
            .as_array()
            .unwrap()
            .len(),
        2
    );
    call(
        &app,
        "projects.lock",
        json!({"id":project["id"],"locked":false}),
    )
    .await;
    call(
        &app,
        "projects.rename",
        json!({"id":project["id"],"name":"Umbenannt"}),
    )
    .await;
    assert!(call(&app, "projects.list", Value::Null)
        .await
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == project["id"] && p["name"] == "Umbenannt"));
}

// spec: CRAWL-128
#[tokio::test]
async fn list_evaluates_heartbeat_and_prevents_running_file_actions() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (id, path) = crawl(&app, 105, CrawlState::Running);
    let list = call(&app, "crawls.list", json!({"show":"running"})).await;
    assert_eq!(list[0]["crawl"]["phase"], "running");
    assert_eq!(list[0]["canResume"], false);
    for (name, args) in [
        ("crawls.delete", json!({"ids":[id]})),
        ("crawls.move", json!({"ids":[id],"project":null})),
        (
            "crawls.export",
            json!({"ids":[id],"dest":dir.path().join("export.ospider")}),
        ),
    ] {
        assert_eq!(
            operation(&app, name, args).await.error.unwrap().code,
            ErrorCode::Busy
        );
    }
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("heartbeat_at", "2000-01-01T00:00:00.000Z")
        .unwrap();
    drop(file);
    let list = call(&app, "crawls.list", json!({"show":"incomplete"})).await;
    assert_eq!(list[0]["canResume"], true);
    assert_eq!(list[0]["crawl"]["phase"], "incomplete");
}

// spec: CRAWL-124 [M2]
#[tokio::test]
async fn storage_setting_does_not_move_files_and_explicit_move_updates_open_session() {
    let dir = tempfile::tempdir().unwrap();
    let new_dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (id, path) = crawl(&app, 106, CrawlState::Paused);
    let key = CrawlKey(id.clone());
    app.register_crawl(key.clone(), path.clone());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key: key.clone(),
            path: path.clone(),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );
    app.read_pool()
        .run(key.clone(), |f, _| Ok(f.meta()?.crawl_id))
        .await
        .unwrap();
    app.write_queue()
        .submit_file(&key, |f| {
            f.set_meta("name", "Writer-Verbindung").unwrap();
            Ok(())
        })
        .await
        .unwrap();
    call(
        &app,
        "settings.set",
        json!({"patch":{"storage":{"crawlsDir":new_dir.path()}}}),
    )
    .await;
    assert!(path.exists());
    assert_eq!(
        call(&app, "crawls.list", json!({"show":"all"}))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    success(
        &app,
        "crawls.move",
        json!({"ids":[id],"destinationDir":new_dir.path()}),
    )
    .await;
    let target = registry::path_for(new_dir.path(), &id.parse::<CrawlId>().unwrap());
    assert!(!path.exists());
    assert!(target.exists());
    assert_eq!(
        app.sessions()
            .current(&WindowId("main".into()))
            .unwrap()
            .path,
        target
    );
    assert_eq!(app.read_pool().path_of(&key), Some(target));
    assert_eq!(
        app.read_pool()
            .run(key, |f, _| Ok(f.meta()?.name))
            .await
            .unwrap(),
        "Writer-Verbindung"
    );
}

// spec: CRAWL-127 [M2]
#[tokio::test]
async fn export_import_duplicate_have_results_and_independent_ids() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (id, _) = crawl(&app, 107, CrawlState::Paused);
    let target = external.path().join("portable.ospider");
    assert_eq!(
        success(&app, "crawls.export", json!({"ids":[id],"dest":target})).await["count"],
        1
    );
    let fake = external.path().join("fake.ospider");
    std::fs::write(&fake, "not sqlite").unwrap();
    let imported = success(&app, "crawls.import", json!({"paths":[target,fake]})).await;
    assert_eq!(imported["rejected"][0]["reasonKey"], "err-not-a-crawl-file");
    assert_ne!(imported["crawlIds"][0], id);
    success(&app, "crawls.duplicate", json!({"ids":[id]})).await;
    let list = call(&app, "crawls.list", json!({"show":"all"})).await;
    assert_eq!(list.as_array().unwrap().len(), 3);
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|l| l["canResume"] == true));
}

// spec: OUT-229
#[tokio::test]
async fn settings_change_and_manual_retention_protect_open_and_locked_crawls() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let (old, old_path) = crawl(&app, 108, CrawlState::Completed);
    let (open, path) = crawl(&app, 109, CrawlState::Completed);
    for p in [&old_path, &path] {
        let file = CrawlFile::open(p, OpenMode::ReadWrite).unwrap();
        file.set_meta("ended_at", "2000-01-01T00:00:00.000Z")
            .unwrap();
    }
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key: CrawlKey(open),
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    call(
        &app,
        "settings.set",
        json!({"patch":{"retention":{"policy":"one_week"}}}),
    )
    .await;
    os_app::commands::crawls::run_retention(&app).await.unwrap();
    assert!(!old_path.exists(), "{old}");
    assert_eq!(
        call(&app, "crawls.list", json!({"show":"all"}))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

// spec: CRAWL-124 [M2]
#[tokio::test]
async fn sync_and_network_paths_return_warning_levels() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let sync = call(
        &app,
        "crawls.checkLocation",
        json!({"path":Path::new("Dropbox").join("crawl")}),
    )
    .await;
    assert!(sync
        .as_array()
        .unwrap()
        .contains(&json!("warn-location-sync")));
    let network = call(
        &app,
        "crawls.checkLocation",
        json!({"path":r"\\server\crawls"}),
    )
    .await;
    assert!(network
        .as_array()
        .unwrap()
        .contains(&json!("warn-location-network")));
}
