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
    // A frontend layout save starts before the core remembers the new crawl.
    let mut stale_ui = call(&first, "uistate.load", json!(null)).await;
    stale_ui["state"]["layout"] = json!({"split":0.4});
    // B3-FLAKES (CRAWL-126): Die Pause muss vor dem natürlichen Ende ankommen. Die frühere Fixture (2 Seiten à
    // 50 ms) war unter Windows schon fertig („completed“), bevor `crawl.pause` die Engine erreichte. Jetzt eine Kette:
    // Jede Seite verlinkt nur die nächste, also kann der Crawl unabhängig von der Thread-Zahl frühestens nach
    // CHAIN × 50 ms = 20 s enden – weit über der Pausenzusage (< 3 s). Antworten bis zur Pause zurückzuhalten geht
    // nicht: Die Pause wartet laufende Abrufe ab.
    const CHAIN: usize = 400;
    let mut site = os_engine::testing::SimSite::new().delay(std::time::Duration::from_millis(50));
    for n in 0..CHAIN {
        let url = if n == 0 {
            "https://fixture.test/".to_owned()
        } else {
            format!("https://fixture.test/p/{n}")
        };
        site = site.page(&url, &format!("<a href='/p/{}'>weiter</a>", n + 1));
    }
    let fetch = os_engine::testing::SimFetcher::new(site);
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
    let error = first
        .dispatch(ctx(), "uistate.save", stale_ui)
        .await
        .unwrap_err();
    assert_eq!(error.code, os_app::ErrorCode::Stale);
    let mut fresh = call(&first, "uistate.load", json!(null)).await;
    assert_eq!(fresh["state"]["recentCrawls"][0]["path"], reference.file);
    fresh["state"]["layout"] = json!({"split":0.4});
    call(&first, "uistate.save", fresh).await;
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
    assert_eq!(ui["state"]["layout"]["split"], 0.4);
}
