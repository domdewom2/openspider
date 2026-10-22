#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{
    dto::OpState, pool::CrawlKey, runtime::sessions::SessionInfo, App, CallCtx, WindowId,
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
// spec: UI-066
// spec: UI-020 [P1]
#[tokio::test]
async fn site_structure_is_restricted_to_a_segment_or_to_the_unassigned_urls() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = setup(&app, dir.path());
    file.conn().execute_batch("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES(4,'https://site.test/shop/deep/b','https://site.test/shop/deep/b','https://site.test',1,'done','link',2,29),(5,'https://site.test/shop/other','https://site.test/shop/other','https://site.test',1,'done','link',1,27); UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'").unwrap();
    call(
        &app,
        "segment.save",
        json!({"segments":[definition("b","B","/b")]}),
    )
    .await;
    let args = |segment: Value, path: Option<&str>| json!({"view":"indexability","segment":segment,"path":path,"offset":0,"limit":50});
    let root = |tree: &Value| {
        (
            tree["paths"][0]["count"].clone(),
            tree["paths"][0]["children"].clone(),
        )
    };
    let all = call(
        &app,
        "sidebar.siteStructure",
        json!({"view":"indexability","offset":0,"limit":50}),
    )
    .await;
    assert_eq!(root(&all).0, json!(5));
    let b = call(
        &app,
        "sidebar.siteStructure",
        args(json!({"kind":"segment","name":"B"}), None),
    )
    .await;
    let (count, children) = root(&b);
    assert_eq!(count, json!(2));
    let shop = &children[0];
    assert_eq!(
        (shop["name"].clone(), shop["count"].clone()),
        (json!("shop"), json!(1))
    );
    assert_eq!(shop["children"][0]["path"], "https://site.test/shop/deep/");
    let none = call(
        &app,
        "sidebar.siteStructure",
        args(json!({"kind":"none"}), None),
    )
    .await;
    assert_eq!(root(&none).0, json!(3));
    let lazy = call(
        &app,
        "sidebar.siteStructure",
        args(json!({"kind":"none"}), Some("https://site.test/")),
    )
    .await;
    assert_eq!(lazy["paths"][0]["name"], "shop");
    assert_eq!(lazy["paths"][0]["count"], 1);
    let unknown = call(
        &app,
        "sidebar.siteStructure",
        args(json!({"kind":"segment","name":"missing"}), None),
    )
    .await;
    assert_eq!(unknown["paths"], json!([]));
}

// spec: UI-067 [P1]
#[tokio::test]
async fn segment_overview_classes_use_the_ui_keys() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let _file = setup(&app, dir.path());
    call(
        &app,
        "segment.save",
        json!({"segments":[definition("b","B","/b")]}),
    )
    .await;
    let rows = call(&app, "sidebar.segments", json!({"view":"indexability"})).await;
    let classes = rows[0]["byClass"].as_object().unwrap();
    for key in [
        "Issues",
        "Warnings",
        "Opportunities",
        "High",
        "Medium",
        "Low",
        "Indexable",
        "Non-Indexable",
    ] {
        assert!(classes.contains_key(key), "{key} missing in {classes:?}");
    }
}

// spec: CRAWL-134
#[tokio::test]
async fn segment_validate_reports_the_condition_path_of_the_editor_tree() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let _file = setup(&app, dir.path());
    let mut segment = definition("b", "B", "/b");
    segment["rules"] = json!({"kind":"group","op":"or","children":[
        {"kind":"group","op":"and","children":[
            {"kind":"cond","column":"Address","op":"endsWith","value":"/ok"},
            {"kind":"cond","column":"Address","op":"regex","value":"["}]}]});
    let issues = call(&app, "segment.validate", json!({"segment":segment})).await;
    assert_eq!(issues[0]["path"], "segments[0].groups[0].conditions[1]");
    assert_eq!(issues.as_array().unwrap().len(), 1);
}

// spec: CRAWL-134
#[tokio::test]
async fn segment_reassign_is_idempotent_and_moves_the_versions_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(os_model::ManualClock::new(common::created_at()));
    let app = App::new(os_app::AppConfig::new(dir.path()).with_clock(clock.clone())).unwrap();
    let file = setup(&app, dir.path());
    call(
        &app,
        "segment.save",
        json!({"segments":[definition("b","B","/b")]}),
    )
    .await;
    let seq = || {
        file.conn()
            .query_row(
                "SELECT value FROM crawl_meta WHERE key='change_seq'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
    };
    let first = operation(&app, "segment.reassign", Value::Null).await;
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT url_id FROM url_segments WHERE segment_id=0",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let refresh_versions = || async {
        use os_app::events::TickSource;
        clock.advance(Duration::from_millis(100));
        let session = app.sessions().current(&WindowId("main".into())).unwrap();
        os_app::session::feed::refresh(&app, &session)
            .await
            .unwrap();
        let mut parts = os_app::events::TickParts::default();
        app.ext::<os_app::session::SessionService>()
            .collect(&mut parts);
        (
            parts.counts_version,
            parts.issues_version,
            parts.sidebar_version,
        )
    };
    let baseline_versions = refresh_versions().await;
    assert!(baseline_versions.0.is_some());
    let after_first = seq();
    assert_eq!(
        file.get_meta("segments.assignment_seq").unwrap(),
        Some(after_first.clone())
    );
    let second = operation(&app, "segment.reassign", Value::Null).await;
    assert_eq!(first, json!({"count":3}));
    assert_eq!(second, first);
    assert_eq!(refresh_versions().await, (None, None, None));
    assert_eq!(
        seq(),
        after_first,
        "a repeated reassign must not move the change sequence"
    );
    file.conn().execute_batch("UPDATE urls SET url='https://site.test/b2', address='https://site.test/b2' WHERE id=1; UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'").unwrap();
    let after_mutation = seq().parse::<u64>().unwrap();
    operation(&app, "segment.reassign", Value::Null).await;
    assert_eq!(seq().parse::<u64>().unwrap(), after_mutation + 1);
    assert_eq!(
        file.get_meta("segments.assignment_seq").unwrap(),
        Some(seq())
    );
    assert_eq!(file.get_meta("reset_seq").unwrap(), Some(seq()));
    let members: Vec<i64> = file
        .conn()
        .prepare("SELECT url_id FROM url_segments WHERE segment_id=0 ORDER BY url_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(members.is_empty());
    let versions = refresh_versions().await;
    assert_eq!(
        versions,
        (
            baseline_versions.0.map(|v| v + 1),
            baseline_versions.1.map(|v| v + 1),
            baseline_versions.2.map(|v| v + 1)
        )
    );
    assert_eq!(refresh_versions().await, (None, None, None));
}
