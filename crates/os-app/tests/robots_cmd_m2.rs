use os_app::{pool::CrawlKey, runtime::sessions::SessionInfo, App, AppConfig, CallCtx, WindowId};
use os_model::{CrawlConfig, CrawlState};
use serde_json::json;
use std::sync::Arc;

// spec: CRAWL-102
#[tokio::test]
async fn tester_uses_current_config_ua_custom_file_and_decisive_line() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    let key = CrawlKey("fixture".into());
    let mut cfg = CrawlConfig::default();
    cfg.robots.custom_files.insert(
        "site.test".into(),
        "User-agent: *\nDisallow: /blog/\nUser-agent: Special\nAllow: /blog/\n".into(),
    );
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key: key.clone(),
            path: dir.path().join("unused.ospider"),
            config: Arc::new(cfg.clone()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "config.testRobots",
            json!({"url":"https://site.test/blog/one"}),
        )
        .await
        .unwrap();
    assert_eq!(result["allowed"], false);
    assert_eq!(result["matchedLine"], 2);
    cfg.user_agent.robots_token = Some("Special".into());
    app.sessions().update_config(&key, Arc::new(cfg));
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "config.testRobots",
            json!({"url":"https://site.test/blog/one"}),
        )
        .await
        .unwrap();
    assert_eq!(result["allowed"], true);
    assert_eq!(result["matchedLine"], 4);
    assert_eq!(result["userAgent"], "Special");
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "config.testRobots",
            json!({"url":"https://site.test/blog/one","content":"User-agent: *\nDisallow: /"}),
        )
        .await
        .unwrap();
    assert_eq!(result["allowed"], false);
}

// spec: CRAWL-102
#[tokio::test]
async fn load_live_file_follows_redirect_and_does_not_return_custom_content() {
    let fixture = tempfile::tempdir().unwrap();
    std::fs::write(fixture.path().join("scenario.toml"),"[[route]]\npath='/robots.txt'\nstatus=302\nlocation='/live'\n[[route]]\npath='/live'\nbody='User-agent: *'\n").unwrap();
    let site = os_testsite::TestSite::start_dir(fixture.path(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(dir.path())).unwrap();
    let result = app
        .dispatch(
            CallCtx::for_window("main"),
            "config.loadRobots",
            json!({"host":site.url("/")}),
        )
        .await
        .unwrap();
    assert_eq!(result, "User-agent: *");
    assert_eq!(site.count("127.0.0.1", "/live"), 1);
}
