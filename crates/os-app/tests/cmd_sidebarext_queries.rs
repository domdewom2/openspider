#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{pool::CrawlKey, runtime::sessions::SessionInfo, App, CallCtx, ErrorCode, WindowId};
use os_model::{AnalysisStep, CrawlConfig, CrawlState, StepState};
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::sync::Arc;

fn session(app: &App, dir: &std::path::Path) -> CrawlFile {
    let path = common::make_crawl(dir, "sidebar.ospider");
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    for (id, url, depth, class, internal, status, time) in [
        (
            1,
            "https://site.test/",
            0,
            "HTML",
            true,
            Some(200),
            Some(200),
        ),
        (
            2,
            "https://site.test/a/b/page?x=1",
            3,
            "HTML",
            true,
            Some(200),
            Some(1000),
        ),
        (
            3,
            "https://site.test/a/b/picture.png",
            10,
            "Images",
            true,
            Some(200),
            Some(11000),
        ),
        (
            4,
            "https://outside.test/",
            1,
            "HTML",
            false,
            Some(200),
            Some(800),
        ),
        (
            5,
            "https://site.test/error",
            4,
            "HTML",
            true,
            Some(404),
            Some(2500),
        ),
        (6, "https://site.test/timeout", 2, "HTML", true, None, None),
    ] {
        file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,crawl_depth,
            folder_depth,url_length,content_class,indexability,status_code,response_time_ms,change_seq)
            VALUES(?1,?2,?2,'https://site.test',?3,'done','hyperlink',?4,0,30,?5,'Indexable',?6,?7,1)",
            rusqlite::params![id,url,internal,depth,class,status,time]).unwrap();
    }
    file.set_meta("change_seq", "1").unwrap();
    let key = CrawlKey("k03".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: false,
            engine: None,
        },
    );
    file
}
async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}

// spec: UI-066
#[tokio::test]
async fn structure_views_paging_and_live_changes_use_current_session() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = session(&app, dir.path());
    let index = call(
        &app,
        "sidebar.siteStructure",
        json!({"view":"indexability","offset":0,"limit":10}),
    )
    .await;
    assert_eq!(index["paths"][0]["count"], 5);
    assert_eq!(index["paths"][0]["byClass"]["Indexable"], 4);
    assert_eq!(
        index["paths"][0]["children"][0]["path"],
        "https://site.test/a/"
    );
    let total = index["depths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["count"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(total, 4);
    for view in ["responseCodes", "crawlDepth"] {
        let out = call(
            &app,
            "sidebar.siteStructure",
            json!({"view":view,"offset":0,"limit":10}),
        )
        .await;
        assert_eq!(
            out["depths"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["count"].as_u64().unwrap())
                .sum::<u64>(),
            total
        );
        if view == "responseCodes" {
            assert_eq!(out["depths"][3]["byClass"]["4xx"], 1);
        }
    }
    let page = call(
        &app,
        "sidebar.siteStructure",
        json!({"view":"indexability","path":"https://site.test/a/","offset":0,"limit":1}),
    )
    .await;
    assert_eq!(page["paths"][0]["path"], "https://site.test/a/b/");
    assert_eq!(page["paths"][0]["count"], 2);
    assert!(call(
        &app,
        "sidebar.siteStructure",
        json!({"view":"indexability","path":"https://site.test/a/","offset":1,"limit":1})
    )
    .await["paths"]
        .as_array()
        .unwrap()
        .is_empty());
    file.conn()
        .execute(
            "UPDATE urls SET indexability='Non-Indexable',change_seq=2 WHERE id=2",
            [],
        )
        .unwrap();
    file.set_meta("change_seq", "2").unwrap();
    let new = call(
        &app,
        "sidebar.siteStructure",
        json!({"view":"indexability","offset":0,"limit":10}),
    )
    .await;
    assert_eq!(new["paths"][0]["byClass"]["Non-Indexable"], 1);
    let other = app
        .dispatch(
            CallCtx::for_window("other"),
            "sidebar.responseTimes",
            json!({"scope":"all"}),
        )
        .await
        .unwrap_err();
    assert_eq!(other.code, ErrorCode::NoCrawl);
}

// spec: UI-069
#[tokio::test]
async fn response_scopes_and_unbounded_bucket_follow_wire_contract() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let _file = session(&app, dir.path());
    let all = call(&app, "sidebar.responseTimes", json!({"scope":"all"})).await;
    assert_eq!(all["buckets"].as_array().unwrap().len(), 11);
    assert_eq!(all["buckets"][0]["count"], 2);
    assert_eq!(all["buckets"][1]["count"], 1);
    assert_eq!(
        all["buckets"][10],
        json!({"fromMs":10000,"toMs":null,"count":1})
    );
    assert_eq!(all["noResponse"], 1);
    let html = call(
        &app,
        "sidebar.responseTimes",
        json!({"scope":"internalHtml"}),
    )
    .await;
    assert_eq!(html["buckets"][0]["count"], 1);
    assert_eq!(html["buckets"][10]["count"], 0);
    assert_eq!(html["noResponse"], 1);
}

// spec: UI-082 [M2]
#[tokio::test]
async fn config_bus_invalidates_only_affected_steps_and_recrawl_maps_to_dto() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = session(&app, dir.path());
    let old = Arc::new(CrawlConfig::default());
    os_analyze::analysis::persist::write_step(
        &file,
        AnalysisStep::Content,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            AnalysisStep::Content,
            &old,
        )),
    )
    .unwrap();
    let mut new = (*old).clone();
    new.content.area.include_elements = vec!["main".into()];
    let new = Arc::new(new);
    file.set_config(&new).unwrap();
    app.sessions()
        .update_config(&CrawlKey("k03".into()), new.clone());
    app.crawl_events().emit(
        &app,
        os_app::runtime::lifecycle_bus::CrawlEvent::ConfigChanged {
            key: CrawlKey("k03".into()),
            old,
            new: new.clone(),
        },
    );
    assert!(
        common::eventually(std::time::Duration::from_secs(5), || {
            os_analyze::analysis::persist::read_stale(&file)
                .unwrap()
                .contains(AnalysisStep::Content)
        })
        .await
    );
    let source = os_analyze::aggregates::status::analysis_status(&file, &new);
    let dtos = os_app::commands::sidebar_ext::step_statuses(&source).unwrap();
    let content = dtos.iter().find(|s| s.step == "content").unwrap();
    assert!(content.stale);
    assert_eq!(content.state, os_app::dto::StepStateDto::NeedsRecrawl);
}

// spec: UI-066
#[tokio::test]
async fn invalid_arguments_and_cancel_are_typed_errors() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let _file = session(&app, dir.path());
    for limit in [0, 501] {
        let e = app
            .dispatch(
                common::ctx(),
                "sidebar.siteStructure",
                json!({"view":"indexability","offset":0,"limit":limit}),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidInput);
    }
    let ctx = common::ctx();
    ctx.cancel.cancel();
    assert_eq!(
        app.dispatch(ctx, "sidebar.responseTimes", json!({"scope":"all"}))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Cancelled
    );
}

// spec: UI-145
#[tokio::test]
async fn cache_connection_survives_pool_contention_and_exclusive_access_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = session(&app, dir.path());
    let first = call(&app, "sidebar.responseTimes", json!({"scope":"all"})).await;
    let key = CrawlKey("k03".into());
    // Eine normale Pool-Verbindung kann ihren eigenen Temp-Cache nicht sehen.
    let regular = app
        .read_pool()
        .run(key.clone(), |file, _| {
            Ok(file.conn().query_row(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='k03_meta'",
                [],
                |r| r.get::<_, u32>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(regular, 0);
    // Ein Cache-Hit benötigt auch nach anderen Pool-Aufrufen keine URL-Tabelle.
    file.conn()
        .execute("ALTER TABLE urls RENAME TO fixture_urls", [])
        .unwrap();
    assert_eq!(
        call(&app, "sidebar.responseTimes", json!({"scope":"all"})).await,
        first
    );
    file.conn()
        .execute("ALTER TABLE fixture_urls RENAME TO urls", [])
        .unwrap();
    let guard = app.read_pool().exclusive(&key).await;
    drop(guard);
    let cached = app
        .read_pool()
        .run_cached(key, "sidebar", Default::default(), |file, _| {
            Ok(file.conn().query_row(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='k03_meta'",
                [],
                |r| r.get::<_, u32>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(cached, 0);
    assert_eq!(
        call(&app, "sidebar.responseTimes", json!({"scope":"all"})).await,
        first
    );
}

// spec: UI-145
#[tokio::test]
async fn open_event_precomputes_sidebar_on_its_cache_connection() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let file = session(&app, dir.path());
    app.crawl_events().emit(
        &app,
        os_app::runtime::lifecycle_bus::CrawlEvent::Opened(CrawlKey("k03".into())),
    );
    // submit/await muss dem gestarteten Hörer Gelegenheit geben, sich einzureihen.
    for _ in 0..100 {
        let built = app
            .read_pool()
            .run_cached(
                CrawlKey("k03".into()),
                "sidebar",
                Default::default(),
                |file, _| {
                    Ok(file.conn().query_row(
                        "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='k03_meta'",
                        [],
                        |r| r.get::<_, u32>(0),
                    )?)
                },
            )
            .await
            .unwrap();
        if built > 0 {
            file.conn()
                .execute("ALTER TABLE urls RENAME TO fixture_urls", [])
                .unwrap();
            assert_eq!(
                call(&app, "sidebar.responseTimes", json!({"scope":"all"})).await["noResponse"],
                1
            );
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("Opened hat keine Sidebar-Aggregate vorbereitet");
}
