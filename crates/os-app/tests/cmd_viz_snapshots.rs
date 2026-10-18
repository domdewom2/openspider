#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
#[path = "../../os-analyze/tests/graph_fixture.rs"]
mod fx;

use os_app::{
    dto::*,
    events::CollectingSink,
    pool::CrawlKey,
    runtime::{hooks::WindowHost, sessions::SessionInfo},
    App, CallCtx, WindowId,
};
use os_model::{CrawlConfig, CrawlState};
use os_store::CrawlFile;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
struct Host {
    opened: Mutex<Vec<String>>,
    focused: AtomicUsize,
}
impl WindowHost for Host {
    fn open_viz(&self, id: &str) -> Result<(), CmdError> {
        self.opened.lock().unwrap().push(id.into());
        Ok(())
    }
    fn focus_main(&self) {
        self.focused.fetch_add(1, Ordering::SeqCst);
    }
    fn exit(&self) {}
}
fn app(file: &CrawlFile, home: &std::path::Path) -> (App, Arc<Host>) {
    let app = common::new_app(home);
    let key = CrawlKey("fixture".into());
    app.register_crawl(key.clone(), file.path());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path: file.path().to_owned(),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );
    let host = Arc::new(Host::default());
    app.set_window_host(host.clone());
    (app, host)
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
async fn open(app: &App) -> Value {
    call(app, "viz.open", json!({"kind":"crawlTree"})).await
}
async fn graph(app: &App, id: &Value) -> Value {
    call(
        app,
        "viz.graph",
        json!({"snapshotId":id,"kind":"crawlTree","budget":10000,"metric":"word_count"}),
    )
    .await
}

// spec: UI-090
#[tokio::test]
async fn four_windows_are_independent_snapshots_refresh_and_close_release_data() {
    let (dir, file) = fx::basic();
    let (app, host) = app(&file, &dir.path().join("app"));
    let mut windows = Vec::new();
    for _ in 0..4 {
        windows.push(open(&app).await);
    }
    assert_eq!(host.opened.lock().unwrap().len(), 4);
    let before = graph(&app, &windows[0]["snapshotId"]).await;
    assert_eq!(before["nodes"][0]["metric"], json!(50.0));
    file.conn()
        .execute("UPDATE urls SET word_count=1234 WHERE id=1", [])
        .unwrap();
    assert_eq!(graph(&app, &windows[0]["snapshotId"]).await, before);
    let refreshed = call(
        &app,
        "viz.open",
        json!({"kind":"crawlTree","refreshSnapshotId":windows[0]["snapshotId"]}),
    )
    .await;
    assert_eq!(refreshed, windows[0]);
    assert_eq!(host.opened.lock().unwrap().len(), 4);
    assert_eq!(
        graph(&app, &windows[0]["snapshotId"]).await["nodes"][0]["metric"],
        json!(1234.0)
    );
    assert_eq!(graph(&app, &windows[1]["snapshotId"]).await, before);
    let id = windows[0]["snapshotId"].clone();
    app.window_closed(&WindowId(windows[0]["window"].as_str().unwrap().into()));
    let error = app
        .dispatch(
            CallCtx::for_window("main"),
            "viz.graph",
            json!({"snapshotId":id,"kind":"crawlTree","budget":10000,"metric":"word_count"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Stale);
    for w in &windows[1..] {
        app.window_closed(&WindowId(w["window"].as_str().unwrap().into()));
    }
    let weak = app.downgrade();
    drop(app);
    assert!(
        common::eventually(std::time::Duration::from_secs(1), || weak
            .upgrade()
            .is_none())
        .await
    );
}

// spec: UI-090
#[tokio::test]
async fn data_changed_notice_arrives_within_two_seconds_and_refresh_rearms_it() {
    let (dir, file) = fx::basic();
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let window = WindowId(opened["window"].as_str().unwrap().into());
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(window.clone(), sink.clone());
    for seq in [10, 11] {
        file.set_meta("change_seq", &seq.to_string()).unwrap();
        assert!(common::eventually(std::time::Duration::from_secs(2), || !sink.is_empty()).await);
        let events = sink.take();
        assert!(events.iter().any(|e| matches!(e,UiEvent::Notice {notice} if notice.code=="viz-data-changed" && notice.args["snapshotId"]==opened["snapshotId"])));
        call(
            &app,
            "viz.open",
            json!({"kind":"crawlTree","refreshSnapshotId":opened["snapshotId"]}),
        )
        .await;
    }
    app.window_closed(&window);
}

// spec: UI-090
#[tokio::test]
async fn focus_main_uses_snapshot_crawl_and_rejects_switched_session() {
    let (dir, file) = fx::basic();
    let (app, host) = app(&file, &dir.path().join("app"));
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), sink.clone());
    let opened = open(&app).await;
    let ctx = CallCtx::for_window(opened["window"].as_str().unwrap());
    app.dispatch(ctx.clone(), "viz.focusInMain", json!({"urlId":2}))
        .await
        .unwrap();
    assert_eq!(host.focused.load(Ordering::SeqCst), 1);
    assert!(sink.take().iter().any(|e| matches!(e,UiEvent::FocusUrl {crawl_id,url_id:2,is_internal:true} if crawl_id=="fixture")));
    let mut s = app.sessions().current(&WindowId("main".into())).unwrap();
    s.key = CrawlKey("other".into());
    app.sessions().set_current(WindowId("main".into()), s);
    let error = app
        .dispatch(ctx, "viz.focusInMain", json!({"urlId":2}))
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Stale);
    assert_eq!(host.focused.load(Ordering::SeqCst), 1);
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-091
// spec: UI-092
// spec: UI-099
#[tokio::test]
async fn graph_layouts_share_edges_and_words_use_frozen_anchor_counts() {
    let (dir, file) = fx::basic();
    fx::link(&file, 1, 2, "Product", "", true, "Hyperlink");
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let tree = graph(&app, &opened["snapshotId"]).await;
    let force = call(&app,"viz.graph",json!({"snapshotId":opened["snapshotId"],"kind":"crawlGraph","budget":10000,"metric":"word_count"})).await;
    assert_eq!(tree, force);
    let args = json!({"snapshotId":opened["snapshotId"],"kind":"anchors","urlId":2});
    assert_eq!(
        call(&app, "viz.words", args.clone()).await,
        json!([{"word":"Product","count":1}])
    );
    fx::link(&file, 1, 2, "Product", "", true, "Hyperlink");
    assert_eq!(
        call(&app, "viz.words", args).await,
        json!([{"word":"Product","count":1}])
    );
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-090
#[tokio::test]
async fn csv_is_an_operation_with_displayed_nodes_coordinates_and_rfc_escaping() {
    let (dir, file) = fx::basic();
    file.conn()
        .execute(
            r#"UPDATE urls SET address='https://site.test/a,"quoted"' WHERE id=2"#,
            [],
        )
        .unwrap();
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let mut data = graph(&app, &opened["snapshotId"]).await;
    data["nodes"][0]["x"] = json!(12.5);
    data["nodes"][0]["y"] = json!(-7);
    let args = json!({"snapshotId":opened["snapshotId"],"dir":dir.path().join("csv"),"nodes":data["nodes"],"edges":data["edges"]});
    let op = call(&app, "viz.exportCsv", args.clone()).await;
    let status = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let nodes = std::fs::read_to_string(dir.path().join("csv/nodes.csv")).unwrap();
    let edges = std::fs::read_to_string(dir.path().join("csv/edges.csv")).unwrap();
    assert_eq!(
        nodes.lines().count(),
        data["nodes"].as_array().unwrap().len() + 1
    );
    assert_eq!(
        edges.lines().count(),
        data["edges"].as_array().unwrap().len() + 1
    );
    assert!(nodes.starts_with("Id,Address,Parent Id,Depth,Status Code,Indexability,Segment,Size Metric,Color Value,x,y\r\n"));
    assert!(nodes.contains(",12.5,-7\r\n"));
    assert!(nodes.contains("\"https://site.test/a,\"\"quoted\"\"\""));
    assert!(edges.contains("1,2,tree\r\n"));
    assert_eq!(
        status.result.unwrap()["bytes"].as_u64().unwrap(),
        (nodes.len() + edges.len()) as u64
    );
    let mut invalid = args;
    invalid["nodes"][0]["id"] = json!(999);
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "viz.exportCsv", invalid)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-090
#[tokio::test]
async fn missing_crawl_and_unsupported_p2_kinds_return_explicit_errors() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("main"),
            "viz.open",
            json!({"kind":"crawlTree"})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NoCrawl
    );
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("main"),
            "viz.open",
            json!({"kind":"bodyWords"})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::Unsupported
    );
}

// spec: UI-096
#[tokio::test]
async fn consecutive_explores_keep_previously_opened_children_and_collapse_resets() {
    let (dir, file) = fx::basic();
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let base = json!({"snapshotId":opened["snapshotId"],"kind":"crawlTree","budget":2,"metric":"crawl_depth"});
    call(&app, "viz.graph", base.clone()).await;
    let mut request = base.clone();
    request["exploreNode"] = json!(1);
    let first = call(&app, "viz.graph", request.clone()).await;
    assert_eq!(first["nodes"].as_array().unwrap().len(), 3);
    request["exploreNode"] = json!(2);
    let second = call(&app, "viz.graph", request).await;
    assert_eq!(
        second["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(
        call(&app, "viz.graph", base).await["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-090
#[tokio::test]
async fn failed_native_open_drops_snapshot_and_invalid_root_never_opens_a_window() {
    struct FailingHost(Mutex<Option<String>>);
    impl WindowHost for FailingHost {
        fn open_viz(&self, id: &str) -> Result<(), CmdError> {
            *self.0.lock().unwrap() = Some(id.into());
            Err(CmdError::new(ErrorCode::Io, "fixture-window-failure"))
        }
        fn focus_main(&self) {}
        fn exit(&self) {}
    }
    let (dir, file) = fx::basic();
    let (app, _) = app(&file, &dir.path().join("app"));
    let host = Arc::new(FailingHost(Mutex::new(None)));
    app.set_window_host(host.clone());
    let invalid = app
        .dispatch(
            CallCtx::for_window("main"),
            "viz.open",
            json!({"kind":"crawlTree","rootUrlId":999}),
        )
        .await
        .unwrap_err();
    assert_eq!(invalid.code, ErrorCode::InvalidInput);
    assert!(host.0.lock().unwrap().is_none());
    let error = app
        .dispatch(
            CallCtx::for_window("main"),
            "viz.open",
            json!({"kind":"crawlTree"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.message_key, "fixture-window-failure");
    let id = host.0.lock().unwrap().clone().unwrap();
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("main"),
            "viz.words",
            json!({"snapshotId":id,"kind":"anchors","urlId":2})
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::Stale
    );
}

// spec: UI-090
#[tokio::test]
async fn export_io_failure_is_reported_on_the_operation() {
    let (dir, file) = fx::basic();
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let data = graph(&app, &opened["snapshotId"]).await;
    let target = dir.path().join("file-not-directory");
    std::fs::write(&target, "fixture").unwrap();
    let op=call(&app,"viz.exportCsv",json!({"snapshotId":opened["snapshotId"],"dir":target,"nodes":data["nodes"],"edges":data["edges"]})).await;
    let status = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert_eq!(status.state, OpState::Failed);
    assert_eq!(status.error.unwrap().code, ErrorCode::Io);
    assert_eq!(std::fs::read_to_string(target).unwrap(), "fixture");
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-090
// spec: UI-096
#[tokio::test]
async fn refresh_keeps_the_current_focused_root() {
    let (dir, file) = fx::basic();
    let (app, host) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    call(&app,"viz.graph",json!({"snapshotId":opened["snapshotId"],"kind":"crawlTree","root":2,"budget":10000,"metric":"crawl_depth"})).await;
    call(
        &app,
        "viz.open",
        json!({"kind":"crawlTree","refreshSnapshotId":opened["snapshotId"]}),
    )
    .await;
    let refreshed = graph(&app, &opened["snapshotId"]).await;
    assert_eq!(refreshed["nodes"][0]["id"], 2);
    assert_eq!(host.opened.lock().unwrap().len(), 1);
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-090
#[tokio::test]
async fn a_closed_event_channel_can_resubscribe_without_losing_the_changed_hint() {
    struct ClosedSink(Arc<AtomicUsize>);
    impl os_app::events::EventSink for ClosedSink {
        fn send(&self, _event: UiEvent) -> Result<(), os_app::events::SinkClosed> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(os_app::events::SinkClosed)
        }
    }
    let (dir, file) = fx::basic();
    let (app, _) = app(&file, &dir.path().join("app"));
    let opened = open(&app).await;
    let window = WindowId(opened["window"].as_str().unwrap().into());
    let attempts = Arc::new(AtomicUsize::new(0));
    app.subscribe(window.clone(), Arc::new(ClosedSink(attempts.clone())));
    file.set_meta("change_seq", "99").unwrap();
    assert!(
        common::eventually(std::time::Duration::from_secs(2), || attempts
            .load(Ordering::SeqCst)
            > 0)
        .await
    );
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(window.clone(), sink.clone());
    assert!(common::eventually(std::time::Duration::from_secs(2), || !sink.is_empty()).await);
    assert!(sink
        .take()
        .iter()
        .any(|e| matches!(e,UiEvent::Notice {notice} if notice.code=="viz-data-changed")));
    app.window_closed(&window);
}
