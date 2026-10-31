#[path = "session_common.rs"]
mod common;
use common::*;
use os_model::{CrawlState, EndReason};
use serde_json::json;

// spec: UI-003
// spec: CRAWL-126 [M2]
#[tokio::test]
async fn stopping_a_restored_paused_crawl_is_terminal_and_runs_automatic_analysis() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let path = dir.path().join("paused.ospider");
    let mut config = cfg();
    config.analysis.auto_at_end = true;
    let file = create(&path, config);
    file.set_meta("state", "Paused").unwrap();
    drop(file);
    open(&app, &path).await;
    call(&app, "crawl.stop", json!(null)).await;
    let s = app.sessions().current(&window()).unwrap();
    assert_eq!(s.state, CrawlState::Completed);
    app.read_pool()
        .run(s.key.clone(), |file, _| {
            let meta = file.meta()?;
            assert_eq!(meta.state, CrawlState::Completed);
            assert_eq!(meta.end_reason, Some(EndReason::Stopped));
            Ok(())
        })
        .await
        .unwrap();
    assert!(app
        .dispatch(ctx(), "crawl.resume", json!(null))
        .await
        .is_err());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let status = call(&app, "analysis.status", json!(null)).await;
        if !status["steps"].as_array().unwrap().is_empty() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "automatic analysis never started"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

// spec: UI-003
// spec: CRAWL-126 [M2]
#[tokio::test]
async fn stopping_a_running_crawl_joins_workers_and_rejects_resume() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let fetch = os_engine::testing::SimFetcher::new(
        os_engine::testing::SimSite::new()
            .delay(std::time::Duration::from_millis(50))
            .page("https://fixture.test/", "<a href='/a'>A</a>")
            .page("https://fixture.test/a", "A"),
    );
    os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request("https://fixture.test/".into()),
        cfg(),
        fetch.clone(),
    )
    .await
    .unwrap();
    call(&app, "crawl.stop", json!(null)).await;
    assert_eq!(fetch.inflight(), 0);
    assert_eq!(
        app.sessions().current(&window()).unwrap().state,
        CrawlState::Completed
    );
    assert!(app
        .dispatch(ctx(), "crawl.resume", json!(null))
        .await
        .is_err());
}
