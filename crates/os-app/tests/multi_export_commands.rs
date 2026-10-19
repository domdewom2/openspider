// Direct database setup is confined to this fixture helper; commands use the read pool.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use os_app::{
    dto::{ExportBatchResult, ExportPreset, OpHandle, OpState},
    pool::CrawlKey,
    runtime::sessions::SessionInfo,
    App, AppConfig, CallCtx, WindowId,
};
use os_model::{
    time::{IdSource, SeededIds},
    CrawlConfig, CrawlMode, CrawlState, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use serde_json::json;
use std::{path::Path, sync::Arc};

fn app(home: &Path) -> App {
    App::new(AppConfig::new(home)).unwrap()
}
fn attach(app: &App, home: &Path) {
    let at = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let cfg = CrawlConfig::default();
    let path = home.join("fixture.ospider");
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: SeededIds::new(77).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "".into(),
            name: "Audit".into(),
            config: cfg.clone(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    let key = CrawlKey("fixture".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(cfg),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
}
async fn call(app: &App, command: &str, args: serde_json::Value) -> serde_json::Value {
    app.dispatch(CallCtx::for_window("main"), command, args)
        .await
        .unwrap()
}

// spec: OUT-075
#[tokio::test]
async fn presets_survive_restart_are_portable_and_can_be_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let first = app(dir.path());
    let preset = json!({"name":"Basic", "items":[{"kind":"tab","name":"Internal:All"}], "singleWorkbook":true,"skipEmpty":true});
    call(
        &first,
        "export.presetSave",
        json!({"name":"Basic", "preset":preset}),
    )
    .await;
    let text = std::fs::read_to_string(dir.path().join("export-presets/Basic.ospreset")).unwrap();
    let core = os_export::types::ExportPreset::parse(&text).unwrap();
    assert_eq!(core.name, "Basic");
    drop(first);
    let second = app(dir.path());
    let presets: Vec<ExportPreset> =
        serde_json::from_value(call(&second, "export.presetList", serde_json::Value::Null).await)
            .unwrap();
    assert_eq!(presets.len(), 1);
    assert!(presets[0].single_workbook);
    assert_eq!(presets[0].items[0].name, "Internal:All");
    call(&second, "export.presetDelete", json!({"name":"Basic"})).await;
    assert_eq!(
        call(&second, "export.presetList", serde_json::Value::Null).await,
        json!([])
    );
}

// spec: OUT-075
// spec: OUT-010
#[tokio::test]
async fn multi_returns_an_operation_with_files_progress_and_unknown_name_warning() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    attach(&app, dir.path());
    let handle: OpHandle = serde_json::from_value(call(&app, "export.multi", json!({
        "items":[{"kind":"tab","name":"Internal:All"},{"kind":"bulk","name":"Links:All Inlinks"},{"kind":"tab","name":"Future:All"}],
        "format":"csv", "dir":dir.path().join("out"), "singleWorkbook":false, "skipEmpty":false
    })).await).unwrap();
    assert!(handle.cancellable);
    let state = app.ops().wait(&handle.op_id).await.unwrap();
    assert_eq!(state.state, OpState::Done, "{:?}", state.error);
    assert_eq!(state.done, 9);
    assert_eq!(state.total, Some(9));
    let result: ExportBatchResult = serde_json::from_value(state.result.unwrap()).unwrap();
    assert_eq!(result.files.len(), 2);
    assert_eq!(result.skipped, ["Future:All"]);
    assert_eq!(result.warnings.len(), 1);
    assert!(result
        .files
        .iter()
        .all(|f| Path::new(&f.path).is_file() && f.bytes > 0));
}

// spec: OUT-075
// spec: OUT-007
// spec: OUT-012
#[tokio::test]
async fn preset_by_name_and_by_path_drives_workbook_and_empty_suppression() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    attach(&app, dir.path());
    let preset = json!({"name":"Basic", "items":[{"kind":"tab","name":"Internal:All"},{"kind":"tab","name":"Response Codes:Client Error (4xx)"}], "singleWorkbook":true,"skipEmpty":true});
    call(
        &app,
        "export.presetSave",
        json!({"name":"Basic", "preset":preset}),
    )
    .await;
    for preset in [
        "Basic".into(),
        dir.path()
            .join("export-presets/Basic.ospreset")
            .to_string_lossy()
            .to_string(),
    ] {
        let handle: OpHandle = serde_json::from_value(call(&app, "export.multi", json!({
            "items":[], "format":"xlsx", "dir":dir.path().join("out"), "singleWorkbook":false, "skipEmpty":false, "preset":preset
        })).await).unwrap();
        let status = app.ops().wait(&handle.op_id).await.unwrap();
        assert_eq!(status.state, OpState::Done, "{:?}", status.error);
        let result: ExportBatchResult = serde_json::from_value(status.result.unwrap()).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].path.ends_with("audit_export.xlsx"));
        assert_eq!(result.skipped, ["Response Codes:Client Error (4XX)"]);
    }
}

// spec: OUT-075
#[tokio::test]
async fn preset_names_cannot_escape_the_settings_directory() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for name in ["../escape", "/tmp/escape", "", "a\\b", "."] {
        let error = app
            .dispatch(
                CallCtx::for_window("main"),
                "export.presetDelete",
                json!({"name":name}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, os_app::dto::ErrorCode::InvalidInput);
    }
}

// spec: OUT-075
#[tokio::test]
async fn malformed_presets_are_invalid_input_and_missing_files_are_io_errors() {
    use os_app::dto::ErrorCode;
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    attach(&app, dir.path());
    let malformed = dir.path().join("malformed.ospreset");
    std::fs::write(&malformed, "items = [").unwrap();
    for (path, code, field) in [
        (malformed, ErrorCode::InvalidInput, Some("preset")),
        (dir.path().join("missing.ospreset"), ErrorCode::Io, None),
    ] {
        let output = dir.path().join("out");
        let handle: OpHandle = serde_json::from_value(
            call(
                &app,
                "export.multi",
                json!({
                    "items":[], "format":"csv", "dir":output,
                    "singleWorkbook":false, "skipEmpty":false, "preset":path
                }),
            )
            .await,
        )
        .unwrap();
        let status = app.ops().wait(&handle.op_id).await.unwrap();
        assert_ne!(status.state, OpState::Done);
        let error = status.error.unwrap();
        assert_eq!(error.code, code);
        assert_eq!(error.field.as_deref(), field);
        assert!(error.detail.is_some());
        assert!(status.result.is_none());
        assert!(!output.exists());
    }
}
