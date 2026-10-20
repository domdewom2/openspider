#![cfg(feature = "stub-views")]
use os_app::dto::ErrorCode;
use os_app::runtime::hooks::DesktopActions;
use os_app::{App, AppConfig, CallCtx, CmdError};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn spec(desc: bool) -> Value {
    json!({"kind":"tab","tab":"Internal","filter":"All","sort": if desc {json!([{"column":"Row","dir":"desc"}])} else {json!([])}})
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
fn app() -> (tempfile::TempDir, App) {
    let home = tempfile::tempdir().unwrap();
    let mut config = AppConfig::new(home.path());
    config.stub_views = true;
    let app = App::new(config).unwrap();
    (home, app)
}
#[derive(Default)]
struct Clipboard(Mutex<String>);
impl DesktopActions for Clipboard {
    fn write_clipboard(&self, text: &str) -> Result<(), CmdError> {
        *self.0.lock().unwrap() = text.to_owned();
        Ok(())
    }
    fn reveal_paths(&self, _: &[std::path::PathBuf]) -> Result<(), CmdError> {
        Ok(())
    }
}
// spec: UI-007
#[tokio::test]
async fn typed_basic_fixture_and_projected_columns() {
    let (_home, app) = app();
    let handle = call(&app, "view.open", json!({"spec":spec(false),"columns":[]})).await;
    assert_eq!(handle["total"], 5_000_000);
    assert_eq!(handle["columns"].as_array().unwrap().len(), 65);
    let fixture: Value =
        serde_json::from_str(include_str!("../src/stub/fixtures/fx-ui-basic.json")).unwrap();
    let rows = call(&app, "view.rows", json!({"viewId":handle["viewId"],"version":1,"offset":0,"limit":3,"columns":fixture["columns"]})).await;
    for key in ["offset", "rowKeys", "columns", "cells"] {
        assert_eq!(rows[key], fixture[key]);
    }
    let projected = call(&app, "view.rows", json!({"viewId":handle["viewId"],"version":1,"offset":0,"limit":1,"columns":["Address","Row"]})).await;
    assert_eq!(
        projected["cells"][0],
        json!(["https://fixture.invalid/page/0000001", 1])
    );
}
// spec: UI-140
#[tokio::test]
async fn large_fixture_last_row_versions_locate_and_window_isolation() {
    let (_home, app) = app();
    let h = call(&app, "view.open", json!({"spec":spec(false),"columns":[]})).await;
    let args =
        json!({"viewId":h["viewId"],"version":1,"offset":4_999_999,"limit":200,"columns":["Row"]});
    let rows = call(&app, "view.rows", args.clone()).await;
    assert_eq!(rows["cells"], json!([[5_000_000]]));
    assert_eq!(
        app.dispatch(CallCtx::for_window("other"), "view.rows", args.clone())
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    let h = call(
        &app,
        "view.update",
        json!({"viewId":h["viewId"],"spec":spec(true)}),
    )
    .await;
    assert_eq!(h["version"], 2);
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "view.rows", args)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Stale
    );
    assert_eq!(
        call(&app, "view.locate", json!({"viewId":h["viewId"],"urlId":1})).await,
        4_999_999
    );
    assert_eq!(
        call(&app, "view.locate", json!({"viewId":h["viewId"],"urlId":0})).await,
        Value::Null
    );
    let oversized =
        json!({"viewId":h["viewId"],"version":2,"offset":0,"limit":501,"columns":["Row"]});
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "view.rows", oversized)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    call(&app, "view.close", json!({"viewId":h["viewId"]})).await;
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("main"),
            "view.locate",
            json!({"viewId":h["viewId"],"urlId":1})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
}
// spec: UI-012
#[tokio::test]
async fn copy_uses_fixture_order_sanitizes_tsv_and_enforces_cell_limit() {
    let (_home, app) = app();
    let clipboard = Arc::new(Clipboard::default());
    app.set_desktop_actions(clipboard.clone());
    let h = call(&app, "view.open", json!({"spec":spec(false),"columns":[]})).await;
    let result = call(&app,"view.copy",json!({"viewId":h["viewId"],"selection":{"kind":"ids","ids":[3,1,2,1]},"columns":["Row","Title 1"],"withHeader":true})).await;
    assert_eq!(result["rows"], 3);
    let expected =
        "Row\tTitle 1\n1\tPage 1 fixture text\n2\tPage 2 fixture text\n3\tPage 3 fixture text\n";
    assert_eq!(
        *clipboard.0.lock().unwrap(),
        if cfg!(windows) {
            expected.replace('\n', "\r\n")
        } else {
            expected.into()
        }
    );
    let args = json!({"viewId":h["viewId"],"selection":{"kind":"all","viewId":h["viewId"],"version":1,"except":[]},"columns":["Address"],"withHeader":true});
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "view.copy", args)
            .await
            .unwrap_err()
            .message_key,
        "err-copy-export-required"
    );
}
// spec: UI-140
#[tokio::test]
async fn feature_does_not_replace_handlers_without_opt_in() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("main"),
            "view.open",
            json!({"spec":spec(false),"columns":[]})
        )
        .await
        .unwrap_err()
        .code,
        // view.open ist inzwischen ein echter Handler (ohne Crawl: NoCrawl); der Stub ersetzt ihn ohne Opt-in nicht.
        ErrorCode::NoCrawl
    );
}

// spec: UI-140
#[tokio::test]
async fn window_close_releases_its_fixture_views() {
    let (_home, app) = app();
    let h = call(&app, "view.open", json!({"spec":spec(false),"columns":[]})).await;
    app.window_closed(&os_app::WindowId("main".into()));
    let error = app
        .dispatch(
            CallCtx::for_window("main"),
            "view.locate",
            json!({"viewId":h["viewId"],"urlId":1}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NotFound);
}
