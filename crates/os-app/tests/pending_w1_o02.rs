//! M2-O02/K2-26: Ende-zu-Ende-Anbindung an die noch stubbehaftete Sitzung aus M2-K04.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{dto::OpState, CallCtx};
use os_store::{registry, CrawlFile, OpenMode};
use serde_json::json;

// spec: CRAWL-143
#[tokio::test]
#[ignore = "M2: wartet auf M2-K04"]
async fn duplicate_opens_by_copied_id_and_resumes_without_changing_original() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    std::fs::create_dir_all(&app.config().paths.crawls).unwrap();
    let id = common::crawl_id(1);
    let path = common::make_crawl(&app.config().paths.crawls, &registry::file_name(&id));
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.set_meta("state", "Paused").unwrap();
    drop(file);
    let original = std::fs::read(&path).unwrap();
    let op = app
        .dispatch(
            common::ctx(),
            "crawls.duplicate",
            json!({"ids":[id.to_string()]}),
        )
        .await
        .unwrap();
    let status = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
    assert_eq!(status.state, OpState::Done, "{:?}", status.error);
    let list = app
        .dispatch(common::ctx(), "crawls.list", json!({"show":"all"}))
        .await
        .unwrap();
    let duplicate = list
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["crawl"]["crawlId"] != id.to_string())
        .unwrap();
    let copied_id = duplicate["crawl"]["crawlId"].as_str().unwrap();
    assert_eq!(duplicate["canResume"], true);
    let opened = app
        .dispatch(
            common::ctx(),
            "crawl.open",
            json!({"kind":"id","crawlId":copied_id}),
        )
        .await
        .unwrap();
    assert_eq!(opened["crawlId"], copied_id);
    app.dispatch(
        CallCtx::for_window("main"),
        "crawl.resume",
        serde_json::Value::Null,
    )
    .await
    .unwrap();
    assert!(
        common::eventually(std::time::Duration::from_secs(10), || {
            app.sessions()
                .current(&os_app::WindowId("main".into()))
                .is_some_and(|s| s.state == os_model::CrawlState::Completed)
        })
        .await
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
