#[path = "http_support.rs"]
mod support;
use support::*;
// spec: CRAWL-122
// spec: OUT-269
#[tokio::test]
async fn system_proxy_environment_is_used_by_new_clients() {
    let proxy = MiniServer::start(|_| ok_text("system-proxy-fixture")).await;
    let address = proxy.url("/").to_string();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "system_proxy_child", "--nocapture"])
            .env("OPENSPIDER_E03_PROXY_CHILD", "1")
            .env("HTTP_PROXY", &address)
            .env("http_proxy", &address)
            .env_remove("ALL_PROXY")
            .env_remove("all_proxy")
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .status()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(status.success());
}
// spec: CRAWL-122
#[tokio::test]
async fn system_proxy_child() {
    if std::env::var_os("OPENSPIDER_E03_PROXY_CHILD").is_none() {
        return;
    }
    let client = os_fetch::net::client_builder_with_password(&Default::default(), None)
        .build()
        .unwrap();
    let r = client.get("http://fixture.invalid/").send().await.unwrap();
    assert_eq!(r.text().await.unwrap(), "system-proxy-fixture");
}
