//! Vollständiger Integrationstest gegen den F05-View-Erweiterungspunkt (Fachlogik separat grün).
// Nur der lokale Fixture-Aufbau greift direkt auf den Store zu.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
use os_app::pool::CrawlKey;
use os_app::runtime::sessions::SessionInfo;
use os_app::{App, AppConfig, CallCtx, WindowId};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, CrawlState, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use serde_json::json;
use std::sync::Arc;

// spec: UI-011 [P1]
#[tokio::test]
async fn parsed_rules_reach_the_app_view_and_invalid_regex_preserves_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fx-ui-basic.ospider");
    let cfg = CrawlConfig::default();
    let now = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: SeededIds::new(42).new_crawl_id(now),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "search".into(),
            name: "fx-ui-basic".into(),
            config: cfg.clone(),
            created_at: now,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn().execute_batch("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,word_count,content_class) VALUES (1,'https://site.test/Blog/a','https://site.test/Blog/a','https://site.test',1,'Done','Link',0,24,200,40,'HTML'),(2,'https://site.test/shop','https://site.test/shop','https://site.test',1,'Done','Link',0,22,200,100,'HTML')").unwrap();
    drop(file);
    let app = App::new(AppConfig::new(dir.path().join("app"))).unwrap();
    let key = CrawlKey("fixture".into());
    app.read_pool().register(key.clone(), path.clone());
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
    let tree = app
        .dispatch(
            CallCtx::for_window("main"),
            "search.parse",
            json!({"text":"Address ~ blog AND \"Word Count\" < 50"}),
        )
        .await
        .unwrap();
    let spec = json!({"kind":"tab","tab":"Internal","filter":"All","search":{"kind":"rules","tree":tree,"caseSensitive":false},"sort":[],"segment":null,"compare":null});
    let handle = app
        .dispatch(
            CallCtx::for_window("main"),
            "view.open",
            json!({"spec":spec,"columns":["Address"]}),
        )
        .await
        .unwrap();
    let rows_args = json!({"viewId":handle["viewId"],"version":handle["version"],"offset":0,"limit":50,"columns":["Address"]});
    let before = app
        .dispatch(CallCtx::for_window("main"), "view.rows", rows_args.clone())
        .await
        .unwrap();
    assert_eq!(before["cells"], json!([["https://site.test/Blog/a"]]));
    let mut invalid = spec;
    invalid["search"]["tree"] = json!({"kind":"cond","column":"Address","op":"regex","value":"["});
    assert!(app
        .dispatch(
            CallCtx::for_window("main"),
            "view.update",
            json!({"viewId":handle["viewId"],"spec":invalid})
        )
        .await
        .is_err());
    let after = app
        .dispatch(CallCtx::for_window("main"), "view.rows", rows_args)
        .await
        .unwrap();
    assert_eq!(after, before);
}
