//! Vollständiger Integrationstest gegen den Crawl-Start von K04 (K2-26).
#[path = "runtime_common.rs"]
mod common;
use common::*;
use serde_json::json;
// spec: CRAWL-122
#[tokio::test]
#[ignore = "M2: wartet auf M2-K04"]
async fn crawl_start_rejects_unreachable_proxy_before_creating_a_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    app.dispatch(ctx(),"settings.set",json!({"patch":{"network":{"proxy":{"mode":"manual","kind":"http","host":"127.0.0.1","port":port}}}})).await.unwrap();
    let error=app.dispatch(ctx(),"crawl.start",json!({"source":{"kind":"spider","url":"http://site.test/"},"scope":"domain","profile":null})).await.unwrap_err();
    assert_ne!(error.code, os_app::ErrorCode::Unsupported);
    assert!(error.detail.unwrap_or_default().contains("Proxy"));
    assert!(app.sessions().current(&ctx().window).is_none());
}
