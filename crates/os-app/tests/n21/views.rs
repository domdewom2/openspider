#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use crate::{dto::OpState, App, AppConfig, CallCtx};
use os_model::{CrawlConfig, CrawlMode, ManualClock, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, OpenMode, StorageProfile};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn fixture() -> (tempfile::TempDir, App, Arc<ManualClock>, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n21.ospider");
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n21".into(),
            name: "n21".into(),
            config: CrawlConfig::default(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic/scenario.sql"
        ))
        .unwrap();
    file.set_meta("state", "Completed").unwrap();
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let app = App::new(
        AppConfig::new(dir.path())
            .with_clock(clock.clone())
            .with_ids(Arc::new(SeededIds::new(21))),
    )
    .unwrap();
    (dir, app, clock, path)
}

async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}
async fn open_crawl(app: &App, path: &std::path::Path) -> Value {
    call(app, "crawl.open", json!({"kind":"path", "path":path})).await
}
fn generation(app: &App, handle: &Value) -> u64 {
    let tab = crate::commands::view::get(
        app,
        &CallCtx::for_window("main"),
        handle["viewId"].as_str().unwrap(),
    )
    .unwrap()
    .tab
    .unwrap();
    let core = super::lock(&tab.state).core;
    core
}
async fn open_view(app: &App) -> Value {
    open_columns(app, json!(["Address"])).await
}
async fn open_columns(app: &App, columns: Value) -> Value {
    open_spec(
        app,
        json!({"kind":"tab","tab":"Internal","filter":"All","sort":[]}),
        columns,
    )
    .await
}
async fn open_spec(app: &App, spec: Value, columns: Value) -> Value {
    let h = call(app, "view.open", json!({"spec":spec,"columns":columns})).await;
    if h["state"] == "building" {
        let op = app
            .ops()
            .wait(h["op"]["opId"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(op.state, OpState::Done, "{op:?}");
        op.result.unwrap()
    } else {
        h
    }
}

// spec: UI-140
// spec: UI-142
// spec: ONPAGE-015
#[tokio::test]
async fn percent_denominator_is_bound_to_the_view_snapshot_and_rebuilt_on_change() {
    let (_dir, app, _clock, path) = fixture();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .execute_batch("UPDATE url_counters SET html_2xx_sources=2 WHERE url_id=1; UPDATE urls SET content_class='HTML',status_code=200 WHERE id IN (6,7);")
        .unwrap();
    drop(file);
    open_crawl(&app, &path).await;
    // Hidden internal HTML counts in the denominator; external HTML does not.
    for (expected, total) in [(50.0, 4), (66.67, 3)] {
        let h = open_columns(&app, json!(["Address", "% of Total"])).await;
        let rows = call(
            &app,
            "view.rows",
            json!({"viewId":h["viewId"],"version":h["version"],"offset":0,"limit":1,"columns":[]}),
        )
        .await;
        assert_eq!(rows["cells"][0][1], expected, "denominator {total}");
        call(&app, "view.close", json!({"viewId":h["viewId"]})).await;
        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        file.conn().execute_batch("UPDATE urls SET status_code=404,change_seq=43 WHERE id=8; UPDATE crawl_meta SET value='43' WHERE key='change_seq';").unwrap();
    }
}

// spec: UI-147 [P1]
// spec: UI-141
// spec: UI-142
#[tokio::test]
async fn closed_fixture_tab_is_reused_and_released_after_sixty_seconds() {
    let (_dir, app, clock, path) = fixture();
    let summary = open_crawl(&app, &path).await;
    assert_eq!(summary["urls"], 8); // includes external and hidden discovered URLs
    let first = open_view(&app).await;
    let core = generation(&app, &first);
    call(&app, "view.close", json!({"viewId":first["viewId"]})).await;
    assert!(app.dispatch(CallCtx::for_window("main"), "view.rows", json!({"viewId":first["viewId"],"version":first["version"],"offset":0,"limit":1,"columns":[]})).await.is_err());
    let second = open_view(&app).await;
    assert_ne!(second["viewId"], first["viewId"]);
    for field in ["total", "columns", "state", "version"] {
        assert_eq!(second[field], first[field]);
    }
    assert_eq!(
        generation(&app, &second),
        core,
        "reuse must not rebuild the core view"
    );
    call(&app, "view.close", json!({"viewId":second["viewId"]})).await;
    clock.advance(Duration::from_secs(60));
    super::release_idle(&app);
    let third = open_view(&app).await;
    assert_ne!(third["viewId"], first["viewId"]);
    assert_eq!(third["total"], first["total"]);
}

// spec: UI-142
#[tokio::test]
async fn dormant_fixture_never_reuses_rows_after_a_file_change_or_reset() {
    let (_dir, app, _clock, path) = fixture();
    open_crawl(&app, &path).await;
    let first = open_view(&app).await;
    call(&app, "view.close", json!({"viewId":first["viewId"]})).await;
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn().execute_batch("UPDATE urls SET in_tables=0,change_seq=43 WHERE id=1; UPDATE crawl_meta SET value='43' WHERE key='change_seq';").unwrap();
    drop(file);
    let second = open_view(&app).await;
    assert_eq!(second["version"], 43);
    assert_eq!(second["total"].as_u64().unwrap() + 1, first["total"]);
    call(&app, "view.close", json!({"viewId":second["viewId"]})).await;
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("reset_seq", "43").unwrap();
    drop(file);
    let third = open_view(&app).await;
    assert_eq!(third["total"], second["total"]);
    assert_ne!(
        third["viewId"], second["viewId"],
        "reset invalidates a cache even at the same change_seq"
    );
    call(&app, "view.close", json!({"viewId":third["viewId"]})).await;
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("reset_seq", "1").unwrap();
    file.set_meta("change_seq", "1").unwrap();
    drop(file);
    let reset = open_view(&app).await;
    assert_ne!(reset["viewId"], third["viewId"]);
    assert_eq!(reset["version"], 1);
}

// spec: UI-141
// spec: UI-142
#[tokio::test]
async fn filter_open_reuses_the_tab_session_but_keeps_closed_handles_revoked() {
    let (_dir, app, _clock, path) = fixture();
    open_crawl(&app, &path).await;
    let all = open_view(&app).await;
    let old = crate::commands::view::get(
        &app,
        &CallCtx::for_window("main"),
        all["viewId"].as_str().unwrap(),
    )
    .unwrap()
    .tab
    .unwrap();
    call(&app, "view.close", json!({"viewId":all["viewId"]})).await;
    let html = open_spec(
        &app,
        json!({"kind":"tab","tab":"Internal","filter":"HTML","sort":[]}),
        json!(["Address"]),
    )
    .await;
    let current = crate::commands::view::get(
        &app,
        &CallCtx::for_window("main"),
        html["viewId"].as_str().unwrap(),
    )
    .unwrap()
    .tab
    .unwrap();
    assert!(Arc::ptr_eq(&old, &current));
    assert_ne!(all["viewId"], html["viewId"]);
    assert_eq!(all["total"], 6);
    assert_eq!(html["total"], 5);
    assert!(crate::commands::view::get(
        &app,
        &CallCtx::for_window("main"),
        all["viewId"].as_str().unwrap()
    )
    .is_err());
    let rows = call(&app, "view.rows", json!({"viewId":html["viewId"],"version":html["version"],"offset":0,"limit":200,"columns":[]})).await;
    assert_eq!(rows["rowKeys"], json!([1, 2, 3, 4, 8]));
}

// spec: UI-147 [P1]
#[tokio::test]
async fn grouped_catalog_width_keeps_population_and_directive_scope() {
    let (_dir, app, _clock, path) = fixture();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    // An external URL with a wider list must not widen Internal's catalogue.
    file.conn().execute_batch("INSERT INTO directives(url_id,source,position,raw,tokens,applies,in_head) VALUES (6,'meta_robots',0,'a','a',1,1),(6,'meta_robots',1,'b','b',1,1),(6,'meta_robots',2,'c','c',1,1),(1,'meta_robots',2,'ignored','ignored',0,1);").unwrap();
    drop(file);
    open_crawl(&app, &path).await;
    let columns = call(&app, "catalog.columns", json!({"tab":"Internal"})).await;
    let keys: Vec<_> = columns
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&"Meta Robots 2"));
    assert!(!keys.contains(&"Meta Robots 3"));
}
