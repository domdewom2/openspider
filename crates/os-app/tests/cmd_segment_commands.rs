#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{
    dto::OpState, pool::CrawlKey, runtime::sessions::SessionInfo, App, CallCtx, ErrorCode, WindowId,
};
use os_model::{CrawlConfig, CrawlState};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
fn setup(app: &App, dir: &std::path::Path) -> CrawlFile {
    let path = common::make_crawl(dir, "segments.ospider");
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    let key = CrawlKey("segment-test".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    file
}
fn definition(id: &str, name: &str, value: &str) -> Value {
    json!({"id":id,"name":name,"color":"#336699","order":0,"rules":{"kind":"cond","column":"Address","op":"endsWith","value":value}})
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
async fn operation(app: &App, name: &str, args: Value) -> Value {
    let handle = call(app, name, args).await;
    let id = handle["opId"].as_str().unwrap();
    let done = tokio::time::timeout(Duration::from_secs(10), app.ops().wait(id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.state, OpState::Done, "{:?}", done.error);
    done.result.unwrap()
}
// spec: CRAWL-134
// spec: UI-068 [P1]
#[tokio::test]
async fn command_fixture_roundtrip_preview_save_validate_reassign_delete() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = setup(&app, dir.path());
    let b = definition("b", "B", "/b");
    assert_eq!(call(&app, "segment.list", Value::Null).await, json!([]));
    let preview = operation(&app, "segment.preview", json!({"segments":[b.clone()]})).await;
    assert_eq!(
        preview,
        json!({"counts":[{"id":"b","urls":1}],"unassigned":2})
    );
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM url_segments", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    call(&app, "segment.save", json!({"segments":[b.clone()]})).await;
    assert_eq!(
        call(&app, "segment.list", Value::Null).await,
        json!([b.clone()])
    );
    let mut invalid = b.clone();
    invalid["name"] = json!("");
    assert!(!call(&app, "segment.validate", json!({"segment":invalid}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "segment.save",
            json!({"segments":[b.clone(),definition("duplicate","b","/a")]})
        )
        .await
        .is_err());
    assert_eq!(
        operation(&app, "segment.reassign", Value::Null).await,
        json!({"count":3})
    );
    let json = serde_json::to_string(&call(&app, "segment.list", Value::Null).await).unwrap();
    call(
        &app,
        "segment.save",
        json!({"segments":serde_json::from_str::<Value>(&json).unwrap()}),
    )
    .await;
    call(&app, "segment.delete", json!({"id":"b"})).await;
    assert_eq!(call(&app, "segment.list", Value::Null).await, json!([]));
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM url_segments", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}
// spec: UI-067 [P1]
// spec: UI-020 [P1]
#[tokio::test]
async fn sidebar_and_automatic_new_url_assignment_consume_saved_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = setup(&app, dir.path());
    call(
        &app,
        "segment.save",
        json!({"segments":[definition("b","B","/b")]}),
    )
    .await;
    let rows = call(&app, "sidebar.segments", json!({"view":"indexability"})).await;
    assert_eq!(rows[0]["name"], "B");
    assert_eq!(rows[0]["urls"], 1);
    assert!(rows[1]["name"].is_null());
    assert_eq!(rows[1]["urls"], 2);
    file.conn().execute_batch("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES(4,'https://site.test/new/b','https://site.test/new/b','https://site.test',1,'done','link',0,28); UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'").unwrap();
    assert!(
        common::eventually(Duration::from_secs(1), || file
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM url_segments WHERE url_id=4)",
                [],
                |r| r.get::<_, bool>(0)
            )
            .unwrap())
        .await
    );
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        4
    );
}
// spec: CRAWL-134
#[tokio::test]
async fn readonly_sessions_reject_changes_and_crawl_reopen_restores_rules() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = setup(&app, dir.path());
    let b = definition("b", "B", "/b");
    call(&app, "segment.save", json!({"segments":[b.clone()]})).await;
    let mut session = app.sessions().current(&WindowId("main".into())).unwrap();
    session.config = Arc::new(CrawlConfig::default());
    session.writable = false;
    app.sessions().set_current(WindowId("main".into()), session);
    assert_eq!(
        call(&app, "segment.list", Value::Null).await,
        json!([b.clone()])
    );
    let error = app
        .dispatch(
            CallCtx::for_window("main"),
            "segment.save",
            json!({"segments":[]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(call(&app, "segment.list", Value::Null).await, json!([b]));
    drop(file);
}
