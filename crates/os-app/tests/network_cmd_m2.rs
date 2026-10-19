#[path = "runtime_common.rs"]
mod common;
use common::*;
use os_app::{App, AppConfig};
use os_fetch::{MemorySecretStore, SecretStore};
use serde_json::json;
use std::sync::Arc;
// spec: OUT-269
// spec: CRAWL-122
#[tokio::test]
async fn proxy_password_is_written_only_to_secret_store_and_can_be_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets.clone())).unwrap();
    app.dispatch(
        ctx(),
        "settings.setProxyPassword",
        json!({"password":"fixture-password"}),
    )
    .await
    .unwrap();
    assert_eq!(
        secrets.get(os_fetch::net::PROXY_PASSWORD).as_deref(),
        Some("fixture-password")
    );
    let settings = app
        .dispatch(ctx(), "settings.get", json!(null))
        .await
        .unwrap();
    assert!(!settings.to_string().contains("fixture-password"));
    app.dispatch(ctx(), "settings.setProxyPassword", json!({"password":null}))
        .await
        .unwrap();
    assert_eq!(secrets.get(os_fetch::net::PROXY_PASSWORD), None);
}
// spec: OUT-270
#[tokio::test]
async fn invalid_certificate_is_rejected_without_modifying_settings() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let bad = dir.path().join("bad.pem");
    std::fs::write(&bad, "not a certificate").unwrap();
    let before = app.settings_snapshot();
    let r = app
        .dispatch(
            ctx(),
            "settings.set",
            json!({"patch":{"network":{"trustedCertificates":[bad.to_string_lossy()]}}}),
        )
        .await;
    assert!(r.is_err());
    assert_eq!(app.settings_snapshot(), before);
    assert!(!dir.path().join("settings.toml").exists());
    assert!(app
        .dispatch(
            ctx(),
            "settings.probeCertificates",
            json!({"url":"http://localhost/"})
        )
        .await
        .is_err());
}

// spec: CRAWL-122
// spec: OUT-269
#[tokio::test]
async fn crawl_client_freezes_app_network_settings_and_preflights_proxy() {
    use os_fetch::Fetch;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut head = vec![];
                loop {
                    let Ok(b) = stream.read_u8().await else {
                        return;
                    };
                    head.push(b);
                    if head.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nproxy",
                    )
                    .await
                    .unwrap();
            });
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.dispatch(ctx(),"settings.set",json!({"patch":{"network":{"proxy":{"mode":"manual","kind":"http","host":"127.0.0.1","port":addr.port()}}}})).await.unwrap();
    let f = os_app::commands::settings_network::crawl_fetcher(&app, &Default::default())
        .await
        .unwrap();
    app.dispatch(
        ctx(),
        "settings.set",
        json!({"patch":{"network":{"proxy":{"mode":"none"}}}}),
    )
    .await
    .unwrap();
    let request = os_fetch::FetchRequest {
        url: "http://fixture.invalid/".parse().unwrap(),
        purpose: os_fetch::FetchPurpose::Page,
        cookies: None,
        max_body_bytes: 1024,
        attempt: 0,
        body_limit: Default::default(),
    };
    let os_fetch::FetchOutcome::Response(r) = f.fetch(request.clone()).await.outcome else {
        panic!("snapshot must retain proxy")
    };
    assert_eq!(r.body, "proxy");
    let direct = os_app::commands::settings_network::crawl_fetcher(&app, &Default::default())
        .await
        .unwrap();
    assert!(matches!(
        direct.fetch(request).await.outcome,
        os_fetch::FetchOutcome::NoResponse {
            reason: os_model::NoResponseReason::DnsLookupFailed,
            ..
        }
    ));
    server.abort();
    server.await.unwrap_err();
    app.dispatch(
        ctx(),
        "settings.set",
        json!({"patch":{"network":{"proxy":{"mode":"manual"}}}}),
    )
    .await
    .unwrap();
    assert!(
        os_app::commands::settings_network::crawl_fetcher(&app, &Default::default())
            .await
            .is_err()
    );
}

// spec: CRAWL-023
// spec: OUT-270
#[tokio::test]
async fn certificate_command_returns_the_chain_and_current_trust_decision() {
    let site = os_testsite::TestSite::start("crawl-m2-http").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let url = format!("https://127.0.0.1:{}/", site.port());
    let before = app
        .dispatch(ctx(), "settings.probeCertificates", json!({"url":url}))
        .await
        .unwrap();
    assert_eq!(before["trusted"], false);
    assert!(!before["certificates"].as_array().unwrap().is_empty());
    let ca = dir.path().join("ca.der");
    std::fs::write(&ca, site.ca_cert_der()).unwrap();
    app.dispatch(
        ctx(),
        "settings.set",
        json!({"patch":{"network":{"trustedCertificates":[ca.to_string_lossy()]}}}),
    )
    .await
    .unwrap();
    let after = app
        .dispatch(ctx(), "settings.probeCertificates", json!({"url":url}))
        .await
        .unwrap();
    assert_eq!(after["trusted"], true);
}

// spec: CRAWL-146
#[tokio::test(start_paused = true)]
async fn host_throttling_is_delivered_as_a_notice_in_the_next_tick() {
    use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimPage, SimSite};
    let site = SimSite::new().with(
        "https://site.test/",
        SimPage {
            synthetic: None,
            untruncated_bytes: None,
            set_cookies: vec![],
            status: 429,
            headers: vec![],
            body: String::new(),
            delay: None,
        },
    );
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let file = dir.path().join("crawl.ospider");
    let mut cfg = os_model::CrawlConfig::default();
    cfg.http.retries_network = 0;
    let handle = os_engine::start_crawl(
        SimFetcher::new(site),
        os_engine::CrawlRequest {
            config: cfg.clone(),
            source: os_engine::CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: file.clone(),
            project: String::new(),
            name: String::new(),
        },
        sim_deps(&MemorySink::new()),
    )
    .await
    .unwrap();
    let mut progress = handle.watch();
    while progress.borrow().state != os_model::CrawlState::Completed {
        progress.changed().await.unwrap();
    }
    let engine = Arc::new(os_app::runtime::sessions::EngineControl {
        handle: std::sync::Mutex::new(Some(handle)),
    });
    app.sessions().set_current(
        ctx().window,
        os_app::runtime::sessions::SessionInfo {
            key: os_app::pool::CrawlKey("fixture".into()),
            path: file,
            config: Arc::new(cfg),
            state: os_model::CrawlState::Completed,
            writable: true,
            engine: Some(engine.clone()),
        },
    );
    let tick = app.assemble_tick().unwrap();
    assert!(tick.notices.iter().any(|n| n.code == "network-throttled"));
    assert!(app.assemble_tick().is_none_or(|t| t.notices.is_empty()));
    let handle = engine.handle.lock().unwrap().take().unwrap();
    handle.wait().await.unwrap();
}

// spec: CRAWL-023
#[tokio::test]
async fn ignoring_certificate_errors_produces_a_visible_warning() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let mut cfg = os_model::CrawlConfig::default();
    cfg.http.ignore_certificate_errors = true;
    let _client = os_app::commands::settings_network::crawl_fetcher(&app, &cfg)
        .await
        .unwrap();
    assert!(app
        .assemble_tick()
        .unwrap()
        .notices
        .iter()
        .any(|n| n.code == "tls-verification-disabled"));
}
