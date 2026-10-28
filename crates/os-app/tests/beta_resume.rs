#[path = "session_common.rs"]
mod common;
use common::*;
use serde_json::json;

// spec: CRAWL-126 [M2]
// spec: UI-124
#[tokio::test]
async fn a_new_paused_crawl_is_listed_and_remembered_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let first = app(dir.path());
    let fetch = os_engine::testing::SimFetcher::new(
        os_engine::testing::SimSite::new()
            .delay(std::time::Duration::from_millis(50))
            .page("https://fixture.test/", "<a href='/a'>A</a>")
            .page("https://fixture.test/a", "A"),
    );
    let reference = os_app::session::control::start_with_fetcher(
        &first,
        ctx(),
        request("https://fixture.test/".into()),
        cfg(),
        fetch,
    )
    .await
    .unwrap();
    call(&first, "crawl.pause", json!(null)).await;
    first.read_pool().unregister(&key(&first));
    first.write_queue().unregister(&key(&first));
    drop(first);
    let restarted = app(dir.path());
    let listings = call(&restarted, "crawls.list", json!({"show":"all"})).await;
    let items = listings.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["crawl"]["phase"], "paused");
    assert_eq!(items[0]["canResume"], true);
    let ui = call(&restarted, "uistate.load", json!(null)).await;
    assert_eq!(ui["state"]["recentCrawls"][0]["path"], reference.file);
}
