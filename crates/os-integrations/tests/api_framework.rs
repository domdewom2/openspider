use os_fetch::{MemorySecretStore, SecretStore};
use os_integrations::{
    credentials::{Credential, Credentials},
    http::{self, Transport},
    matching, oauth,
    queue::Queue,
    quota, ApiCtx, ApiError, ApiSource, ApiState, ApiStatus, UrlRef,
};
use os_model::{
    config::{ApiLimitsConfig, UrlMatchConfig},
    ApiKind, Clock, CrawlConfig, ManualClock, Timestamp,
};
use os_testsite::mocks::{common::FixtureApi, oauth::OAuthMock, MockServer};
use std::{sync::Arc, time::Duration};
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/integ-api/framework.json"
    ))
    .unwrap()
}
fn credential() -> Credential {
    Credential {
        account_id: "a".into(),
        label: "Fixture".into(),
        api_key: Some(fixture()["secret"].as_str().unwrap().into()),
        access_token: None,
        refresh_token: None,
        expires_at: None,
        client_secret: None,
        scopes: vec![],
    }
}
fn clock() -> Arc<ManualClock> {
    Arc::new(ManualClock::new(
        Timestamp::from_db("2026-03-08T09:59:59Z").unwrap(),
    ))
}
fn context() -> ApiCtx {
    ApiCtx {
        cfg: CrawlConfig::default(),
        secrets: Arc::new(MemorySecretStore::new()),
        network: Default::default(),
        allow_api_override: false,
        quota_ledger: None,
    }
}
fn endpoints(server: &MockServer) -> oauth::Endpoints {
    let base = server.base_url("oauth");
    oauth::Endpoints {
        authorize: format!("{base}/authorize"),
        token: format!("{base}/token"),
        revoke: format!("{base}/revoke"),
    }
}
// spec: INTEG-002
#[test]
fn credentials_one_json_entry_accounts_memory_and_restart() {
    let store = Arc::new(MemorySecretStore::new());
    let credentials = Credentials::new(store.clone());
    let c = credential();
    assert!(credentials.put(ApiKind::Ga4, c.clone(), true).unwrap());
    let mut other = c.clone();
    other.account_id = "b".into();
    credentials.put(ApiKind::Ga4, other, true).unwrap();
    let raw = store.get("api/ga4").unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["accounts"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        Credentials::new(store.clone())
            .list(ApiKind::Ga4)
            .unwrap()
            .len(),
        2
    );
    credentials.put(ApiKind::Ga4, c.clone(), false).unwrap();
    assert_eq!(
        Credentials::new(store.clone())
            .list(ApiKind::Ga4)
            .unwrap()
            .len(),
        1
    );
    assert!(!format!("{c:?}").contains(fixture()["secret"].as_str().unwrap()));
    credentials.remove(ApiKind::Ga4, "b").unwrap();
    assert!(store.get("api/ga4").is_none());
}
// spec: INTEG-002
#[test]
fn encrypted_export_authenticated_and_random_nonce() {
    let key = [7; 32];
    let secret = fixture()["secret"].as_str().unwrap().as_bytes().to_vec();
    let a = os_integrations::secret_export::encrypt(&secret, &key).unwrap();
    let b = os_integrations::secret_export::encrypt(&secret, &key).unwrap();
    assert_ne!(a, b);
    assert!(!a.windows(secret.len()).any(|w| w == secret));
    assert_eq!(
        os_integrations::secret_export::decrypt(&a, &key).unwrap(),
        secret
    );
    assert!(os_integrations::secret_export::decrypt(&a, &[8; 32]).is_err());
    let mut changed = a;
    changed[20] ^= 1;
    assert!(os_integrations::secret_export::decrypt(&changed, &key).is_err());
}
// spec: INTEG-003
#[tokio::test]
async fn pkce_loopback_exchange_scopes_refresh_and_revoke() {
    let mock = Arc::new(OAuthMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let endpoints = endpoints(&server);
    let clock = clock();
    let auth = oauth::Authorization::begin("custom-client", ApiKind::Ga4, &endpoints)
        .await
        .unwrap();
    let params = auth
        .url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        params["scope"],
        "https://www.googleapis.com/auth/analytics.readonly"
    );
    assert_eq!(params["client_id"], "custom-client");
    assert_eq!(params["code_challenge_method"], "S256");
    assert!(params["redirect_uri"].starts_with("http://127.0.0.1:"));
    let client = http::client(&Default::default()).unwrap();
    let browser = client.get(auth.url.clone()).send().await.unwrap();
    let redirect = browser.headers()["location"].to_str().unwrap().to_owned();
    let finish = auth.finish(&client, "custom-client", None, &endpoints, clock.clone());
    let callback = async {
        let url = url::Url::parse(&redirect).unwrap();
        let addr = ("127.0.0.1", url.port().unwrap());
        drop(tokio::net::TcpStream::connect(addr).await.unwrap());
        let favicon = client
            .get(format!(
                "http://127.0.0.1:{}/favicon.ico",
                url.port().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(favicon.status(), 404);
        client.get(&redirect).send().await
    };
    let (c, response) = tokio::join!(finish, callback);
    assert!(response.unwrap().status().is_success());
    let mut c = c.unwrap();
    assert!(client.get(&redirect).send().await.is_err());
    assert!(c.refresh_token.is_some());
    assert_eq!(c.scopes, vec![oauth::scope(ApiKind::Ga4).unwrap()]);
    clock.advance(Duration::from_secs(3600));
    oauth::refresh(&mut c, &client, "custom-client", &endpoints, clock.as_ref())
        .await
        .unwrap();
    oauth::revoke(&c, &client, &endpoints).await.unwrap();
    assert_eq!(
        mock.log
            .calls()
            .iter()
            .filter(|c| c.path == "token")
            .count(),
        2
    );
    server.stop().await;
}
// spec: INTEG-003
#[tokio::test]
async fn wrong_state_aborts_without_token_request() {
    let mock = Arc::new(OAuthMock::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let endpoints = endpoints(&server);
    let auth = oauth::Authorization::begin("client", ApiKind::Gsc, &endpoints)
        .await
        .unwrap();
    assert!(auth
        .url
        .query_pairs()
        .any(|(k, v)| k == "scope" && v == "https://www.googleapis.com/auth/webmasters.readonly"));
    let redirect = auth
        .url
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .unwrap()
        .1
        .to_string();
    let client = http::client(&Default::default()).unwrap();
    let (r, _) = tokio::join!(
        auth.finish(&client, "client", None, &endpoints, clock()),
        client
            .get(format!("{redirect}?state=wrong&code=stolen"))
            .send()
    );
    assert!(matches!(r, Err(ApiError::Unauthorized(_))));
    assert!(mock.log.calls().is_empty());
    server.stop().await;
}
// spec: INTEG-004
#[tokio::test]
async fn refresh_before_expiry_and_invalid_grant_requires_reconnection() {
    let server = MockServer::start(vec![Arc::new(OAuthMock::default())])
        .await
        .unwrap();
    let client = http::client(&Default::default()).unwrap();
    let mut c = credential();
    c.refresh_token = Some("fixture-refresh".into());
    c.expires_at = Some(clock().now());
    oauth::refresh(
        &mut c,
        &client,
        "custom",
        &endpoints(&server),
        clock().as_ref(),
    )
    .await
    .unwrap();
    assert_eq!(c.access_token.as_deref(), Some("fixture-access"));
    c.refresh_token = Some("invalid".into());
    c.expires_at = None;
    assert!(matches!(
        oauth::refresh(
            &mut c,
            &client,
            "custom",
            &endpoints(&server),
            clock().as_ref()
        )
        .await,
        Err(ApiError::Unauthorized(_))
    ));
    server.stop().await;
}
// spec: INTEG-005
#[tokio::test]
async fn transient_statuses_retry_but_auth_errors_do_not() {
    let mock = Arc::new(FixtureApi::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let transport = Transport::new(&Default::default(), ApiLimitsConfig::default()).unwrap();
    transport
        .execute(
            transport
                .client
                .get(server.base_url("common"))
                .header("X-Scenario", "429x2"),
            clock().now(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(mock.log.calls().len(), 3);
    let before = mock.log.calls().len();
    assert!(transport
        .execute(
            transport
                .client
                .get(server.base_url("common"))
                .header("X-Scenario", "error"),
            clock().now(),
            1
        )
        .await
        .is_err());
    assert_eq!(mock.log.calls().len(), before + 1);
    server.stop().await;
}
// spec: INTEG-005
#[test]
fn retry_after_dates_and_seeded_backoff() {
    let now = Timestamp::from_db("2026-10-18T12:00:00Z").unwrap();
    assert_eq!(
        http::retry_after("Sun, 18 Oct 2026 12:00:07 GMT", now),
        Some(Duration::from_secs(7))
    );
    assert_eq!(http::retry_after("7", now), Some(Duration::from_secs(7)));
    for n in 0..5 {
        let delay = http::backoff(n, 42).as_secs_f64();
        assert!(delay >= 0.8 * 2_f64.powi(n as i32) && delay <= 1.2 * 2_f64.powi(n as i32));
    }
}
// spec: INTEG-006
#[test]
fn persistent_property_quota_and_pacific_dst_reset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quotas.json");
    let ledger = quota::QuotaLedger::new(&path);
    let now = clock().now();
    for _ in 0..5 {
        assert!(ledger.reserve("gsc", "property-a", now, 5).unwrap());
    }
    assert!(!ledger.reserve("gsc", "property-a", now, 5).unwrap());
    assert!(ledger.reserve("gsc", "property-b", now, 5).unwrap());
    assert_eq!(
        quota::QuotaLedger::new(&path)
            .remaining("gsc", "property-a", now, 5)
            .unwrap(),
        0
    );
    let reset = quota::next_reset(now);
    assert_eq!(reset.to_db(), "2026-03-09T07:00:00.000Z");
    assert_eq!(ledger.remaining("gsc", "property-a", reset, 5).unwrap(), 5);
    ledger.exhaust("gsc", "property-a", reset, 5).unwrap();
    assert!(!ledger.reserve("gsc", "property-a", reset, 5).unwrap());
}
struct Source {
    failed: std::sync::atomic::AtomicBool,
}
impl ApiSource for Source {
    fn kind(&self) -> ApiKind {
        ApiKind::Psi
    }
    fn status(&self) -> ApiStatus {
        ApiStatus {
            kind: self.kind(),
            state: ApiState::Idle,
            urls_total: 0,
            urls_done: 0,
            urls_failed: 0,
            detail: None,
        }
    }
    fn fetch<'a>(
        &'a self,
        _: &'a [UrlRef],
        _: &'a ApiCtx,
    ) -> os_integrations::BoxFuture<'a, Result<Vec<os_model::ApiValueWrite>, ApiError>> {
        Box::pin(async move {
            if self.failed.load(std::sync::atomic::Ordering::SeqCst) {
                Err(ApiError::Transport("fixture error".into()))
            } else {
                Ok(vec![])
            }
        })
    }
}
// spec: INTEG-007
#[tokio::test]
async fn queue_final_errors_and_rerequest_progress() {
    let source = Arc::new(Source {
        failed: true.into(),
    });
    let queue = Queue::default();
    let urls = vec![UrlRef {
        url_id: None,
        url: fixture()["urls"][0].as_str().unwrap().into(),
    }];
    let ctx = context();
    queue.run(source.clone(), &urls, &ctx, 2).await.unwrap();
    let s = queue.status().unwrap();
    assert_eq!((s.urls_done, s.urls_total, s.urls_failed), (1, 1, 1));
    source
        .failed
        .store(false, std::sync::atomic::Ordering::SeqCst);
    queue.run(source, &urls, &ctx, 2).await.unwrap();
    assert_eq!(queue.status().unwrap().urls_failed, 0);
    queue.wait().await;
}
// spec: INTEG-014
#[test]
fn matching_fixture_preserves_queries_fragments_and_uses_maximum() {
    let options = UrlMatchConfig::default();
    for case in fixture()["matching"].as_array().unwrap() {
        assert_eq!(
            matching::match_key(case[0].as_str().unwrap(), &options)
                == matching::match_key(case[1].as_str().unwrap(), &options),
            case[2].as_bool().unwrap(),
            "{case}"
        );
    }
    let rows = vec![
        ("http://example.test/b", 10., 0.),
        ("https://example.test/b/", 30., 1.),
        ("https://example.test/b/", 30., 2.),
    ];
    assert_eq!(
        matching::best_match(
            "https://example.test/b/",
            &rows,
            &options,
            |r| r.0,
            |r| (r.1, r.2)
        ),
        Some(&rows[2])
    );
    let strict = UrlMatchConfig {
        ignore_trailing_slash: false,
        ignore_case: false,
    };
    assert_ne!(
        matching::match_key("https://x.test/C/", &strict),
        matching::match_key("http://x.test/c", &strict)
    );
    assert_ne!(
        matching::match_key("https://x.test/d?Color=RED#TOP", &strict),
        matching::match_key("https://x.test/d?color=red#top", &strict)
    );
}
// spec: INTEG-018
#[tokio::test]
async fn api_transport_stays_on_configured_loopback() {
    let mock = Arc::new(FixtureApi::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let transport = Transport::new(&Default::default(), ApiLimitsConfig::default()).unwrap();
    transport
        .execute(
            transport.client.get(server.base_url("common")),
            clock().now(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(mock.log.calls().len(), 1);
    assert_eq!(server.addr().ip(), std::net::Ipv4Addr::LOCALHOST);
    server.stop().await;
}
// spec: INTEG-019
#[tokio::test]
async fn four_scenarios_and_release_override_gate() {
    assert_eq!(
        http::resolve_base(Some("configured"), Some("env"), "production", false, false),
        "production"
    );
    assert_eq!(
        http::resolve_base(None, Some("env"), "production", false, true),
        "env"
    );
    let server = MockServer::start(vec![Arc::new(FixtureApi::default())])
        .await
        .unwrap();
    let client = http::client(&Default::default()).unwrap();
    for (scenario, status) in [
        ("ok", 200),
        ("empty", 200),
        ("error", 401),
        ("throttle", 429),
    ] {
        assert_eq!(
            client
                .get(server.base_url("common"))
                .header("X-Scenario", scenario)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            status
        );
    }
    server.stop().await;
}

// spec: INTEG-016 [M2]
#[tokio::test]
async fn api_client_uses_app_proxy_and_custom_truststore() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let proxy = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 4096];
        let n = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8(bytes[..n].to_vec()).unwrap();
        assert!(request.starts_with("GET http://api.mock.invalid/fixture"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    });
    let mut network = os_model::settings::NetworkSettings::default();
    network.proxy.mode = os_model::ProxyMode::Manual;
    network.proxy.host = "127.0.0.1".into();
    network.proxy.port = port;
    let client = http::client(&network).unwrap();
    assert_eq!(
        client
            .get("http://api.mock.invalid/fixture")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        200
    );
    proxy.await.unwrap();
    let site = os_testsite::TestSite::start("crawl-status-codes")
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("fixture.pem");
    std::fs::write(&cert, site.ca_cert_pem()).unwrap();
    let mut network = os_model::settings::NetworkSettings::default();
    network.proxy.mode = os_model::ProxyMode::None;
    assert!(http::client(&network)
        .unwrap()
        .get(format!("{}/code/200", site.https_base_url()))
        .send()
        .await
        .is_err());
    network
        .trusted_certificates
        .push(cert.to_string_lossy().into());
    assert_eq!(
        http::client(&network)
            .unwrap()
            .get(format!("{}/code/200", site.https_base_url()))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        200
    );
}
// spec: CRAWL-123 [M2]
#[tokio::test]
async fn api_resolve_wildcards_preserve_host_and_require_release_permission() {
    let server = MockServer::start(vec![Arc::new(FixtureApi::default())])
        .await
        .unwrap();
    let mut network = os_model::settings::NetworkSettings::default();
    network.proxy.mode = os_model::ProxyMode::None;
    let rules = vec!["*.mock.test=127.0.0.1".into()];
    let target = format!("http://psi.mock.test:{}/common", server.addr().port());
    let body = http::client_with_resolve(&network, &rules, true, false)
        .unwrap()
        .get(target)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("psi.mock.test"));
    let rules = vec!["*.mock.invalid=127.0.0.1".into()];
    let target = format!("http://psi.mock.invalid:{}/common", server.addr().port());
    assert!(http::client_with_resolve(&network, &rules, false, false)
        .unwrap()
        .get(&target)
        .send()
        .await
        .is_err());
    assert!(http::client_with_resolve(&network, &rules, false, true)
        .unwrap()
        .get(target)
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    server.stop().await;
}

// spec: INTEG-002
#[test]
fn locked_keychain_still_allows_session_credentials() {
    struct Locked;
    impl os_fetch::SecretStore for Locked {
        fn get(&self, _: &str) -> Option<String> {
            None
        }
        fn try_get(&self, _: &str) -> Result<Option<String>, os_fetch::secrets::SecretError> {
            Err(os_fetch::secrets::SecretError::new("locked"))
        }
    }
    impl os_fetch::SecretStoreMut for Locked {
        fn set(&self, _: &str, _: &str) -> Result<(), os_fetch::secrets::SecretError> {
            Err(os_fetch::secrets::SecretError::new("locked"))
        }
        fn delete(&self, _: &str) -> Result<(), os_fetch::secrets::SecretError> {
            Err(os_fetch::secrets::SecretError::new("locked"))
        }
        fn backend(&self) -> os_fetch::secrets::SecretBackend {
            os_fetch::secrets::SecretBackend::Keyring
        }
    }
    let store = Credentials::new(Arc::new(Locked));
    assert!(!store.put(ApiKind::Psi, credential(), true).unwrap());
    assert_eq!(store.list(ApiKind::Psi).unwrap().len(), 1);
    assert!(Credentials::new(Arc::new(Locked))
        .list(ApiKind::Psi)
        .is_err());
    assert!(store.remove(ApiKind::Psi, "a").is_err());
    // Ohne Sitzungseintrag wird der Lesefehler wieder sichtbar.
    assert!(store.list(ApiKind::Psi).is_err());
}
// spec: INTEG-005
#[tokio::test]
async fn sliding_window_and_retry_limit_are_observed_by_mock() {
    let mock = Arc::new(FixtureApi::default());
    let server = MockServer::start(vec![mock.clone()]).await.unwrap();
    let limits = ApiLimitsConfig {
        requests_per_window: Some(2),
        window_s: 1,
        concurrency: 2,
        retries: 0,
        ..Default::default()
    };
    let attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rate_clock = attempts.clone();
    let transport = Transport::with_rate_clock(&Default::default(), limits, move || {
        let now = tokio::time::Instant::now();
        rate_clock.lock().unwrap().push(now);
        now
    })
    .unwrap();
    // Keep the runtime runnable so loopback I/O cannot auto-advance paused time
    // to a rate-limit or HTTP timeout. Only this test advances the clock.
    tokio::time::pause();
    let keep_time_fixed = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    let start = tokio::time::Instant::now();
    let request = |seed| {
        transport.execute(
            transport.client.get(server.base_url("common")),
            clock().now(),
            seed,
        )
    };
    let (first, second) = tokio::join!(request(0), request(1));
    first.unwrap();
    second.unwrap();
    assert_eq!(mock.log.calls().len(), 2);
    assert_eq!(start.elapsed(), Duration::ZERO);
    for seed in [2, 4] {
        let next = request(seed);
        tokio::pin!(next);
        assert!(futures_util::poll!(&mut next).is_pending());
        tokio::time::advance(Duration::from_millis(999)).await;
        assert!(futures_util::poll!(&mut next).is_pending());
        assert_eq!(mock.log.calls().len(), seed as usize);
        // Tokio rounds timer deadlines up to its millisecond tick.
        tokio::time::advance(Duration::from_millis(2)).await;
        next.await.unwrap();
        // A socket-pending future alone is no proof of throttling: the limiter
        // must actually recheck its clock when this request leaves the window.
        assert_eq!(
            attempts.lock().unwrap().last().copied(),
            Some(tokio::time::Instant::now())
        );
        if seed == 2 {
            request(3).await.unwrap();
        }
    }
    assert_eq!(start.elapsed(), Duration::from_millis(2002));
    assert_eq!(mock.log.calls().len(), 5);
    keep_time_fixed.abort();
    tokio::time::resume();
    let transport = Transport::new(
        &Default::default(),
        ApiLimitsConfig {
            retries: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(transport
        .execute(
            transport
                .client
                .get(server.base_url("common"))
                .header("X-Scenario", "500-always"),
            clock().now(),
            1
        )
        .await
        .is_err());
    assert_eq!(mock.log.calls().len(), 8);
    server.stop().await;
}

// spec: INTEG-003
#[tokio::test(start_paused = true)]
async fn loopback_listener_closes_after_five_minutes() {
    let endpoints = oauth::Endpoints::default();
    let auth = oauth::Authorization::begin("fixture-client", ApiKind::Ga4, &endpoints)
        .await
        .unwrap();
    let callback = auth
        .url
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .unwrap()
        .1
        .into_owned();
    let client = http::client(&Default::default()).unwrap();
    let before = tokio::time::Instant::now();
    assert!(auth
        .finish(&client, "fixture-client", None, &endpoints, clock())
        .await
        .is_err());
    assert_eq!(before.elapsed(), Duration::from_secs(300));
    assert!(client.get(callback).send().await.is_err());
}

// spec: INTEG-016 [M2]
#[tokio::test]
async fn api_proxy_authentication_uses_the_shared_secret_store() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let proxy = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 4096];
        let n = stream.read(&mut bytes).await.unwrap();
        let request = String::from_utf8(bytes[..n].to_vec()).unwrap();
        assert!(request
            .to_lowercase()
            .contains("proxy-authorization: basic "));
        assert!(request.contains("Zml4dHVyZS11c2VyOmZpeHR1cmUtcGFzc3dvcmQ="));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    });
    let mut ctx = context();
    ctx.network.proxy.mode = os_model::ProxyMode::Manual;
    ctx.network.proxy.host = "127.0.0.1".into();
    ctx.network.proxy.port = port;
    ctx.network.proxy.username = Some("fixture-user".into());
    let secrets = Arc::new(MemorySecretStore::new());
    secrets.set("openspider/proxy/password", "fixture-password");
    ctx.secrets = secrets;
    assert!(http::client_for(&ctx)
        .unwrap()
        .get("http://api.mock.invalid/fixture")
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    proxy.await.unwrap();
}

// spec: INTEG-002
#[test]
fn password_export_roundtrip_rejects_wrong_password_and_tampering() {
    let secret = fixture()["secret"].as_str().unwrap().as_bytes().to_vec();
    let encrypted =
        os_integrations::secret_export::encrypt_with_password(&secret, "fixture-passphrase")
            .unwrap();
    assert_eq!(
        os_integrations::secret_export::decrypt_with_password(&encrypted, "fixture-passphrase")
            .unwrap(),
        secret
    );
    assert!(os_integrations::secret_export::decrypt_with_password(&encrypted, "wrong").is_err());
    let mut changed = encrypted;
    changed[10] ^= 1;
    assert!(
        os_integrations::secret_export::decrypt_with_password(&changed, "fixture-passphrase")
            .is_err()
    );
    assert!(os_integrations::secret_export::encrypt_with_password(&secret, "").is_err());
}

// spec: INTEG-006
#[test]
fn crawl_caps_and_provider_periods_are_reserved_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = quota::QuotaLedger::new(dir.path().join("quota.json"));
    let now = Timestamp::from_db("2026-10-18T12:00:00Z").unwrap();
    let daily = ("gsc".into(), "property".into(), "2026-10-18".into(), 10);
    let crawl = ("gsc".into(), "property".into(), "crawl:a".into(), 2);
    for _ in 0..2 {
        assert!(ledger
            .reserve_limits_at(&[daily.clone(), crawl.clone()], now)
            .unwrap());
    }
    assert!(!ledger
        .reserve_limits_at(&[daily.clone(), crawl], now)
        .unwrap());
    assert_eq!(
        ledger
            .remaining_period("gsc", "property", "2026-10-18", 10)
            .unwrap(),
        8
    );
    assert!(ledger
        .reserve_limits_at(
            &[
                daily,
                ("gsc".into(), "property".into(), "crawl:b".into(), 2)
            ],
            now
        )
        .unwrap());
    assert_eq!(
        ledger
            .remaining_period("gsc", "property", "crawl:a", 2)
            .unwrap(),
        0
    );
    assert_eq!(
        ledger
            .remaining_period("gsc", "property", "crawl:b", 2)
            .unwrap(),
        1
    );
}

// spec: INTEG-002
#[test]
fn credentials_observe_other_instances_updates_and_removals() {
    let store = Arc::new(MemorySecretStore::new());
    let first = Credentials::new(store.clone());
    let second = Credentials::new(store.clone());
    first.put(ApiKind::Ga4, credential(), true).unwrap();
    assert_eq!(second.list(ApiKind::Ga4).unwrap().len(), 1);
    first.remove(ApiKind::Ga4, "a").unwrap();
    assert!(second.list(ApiKind::Ga4).unwrap().is_empty());
    let mut updated = credential();
    updated.label = "Updated by another instance".into();
    first.put(ApiKind::Ga4, updated, true).unwrap();
    assert_eq!(
        second.list(ApiKind::Ga4).unwrap()[0].label,
        "Updated by another instance"
    );
    let mut updated = credential();
    updated.label = "Second instance".into();
    second.put(ApiKind::Ga4, updated, true).unwrap();
    let observed = first.list(ApiKind::Ga4).unwrap();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].label, "Second instance");
    second.remove(ApiKind::Ga4, "a").unwrap();
    assert!(first.list(ApiKind::Ga4).unwrap().is_empty());
}

// spec: INTEG-006
#[test]
fn ledger_prunes_old_days_and_only_the_closed_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = quota::QuotaLedger::new(dir.path().join("quotas.json"));
    let now = Timestamp::from_db("2026-10-18T12:00:00Z").unwrap();
    for period in [
        "2026-10-15",
        "2026-10-16",
        "2026-10-18",
        "crawl:a",
        "crawl:b",
    ] {
        ledger
            .reserve_limits_at(&[("psi".into(), "".into(), period.into(), 100)], now)
            .unwrap();
    }
    assert_eq!(
        ledger
            .remaining_period("psi", "", "2026-10-15", 100)
            .unwrap(),
        100
    );
    assert_eq!(
        ledger
            .remaining_period("psi", "", "2026-10-16", 100)
            .unwrap(),
        99
    );
    ledger.close_crawl("a", now).unwrap();
    assert_eq!(
        ledger.remaining_period("psi", "", "crawl:a", 100).unwrap(),
        100
    );
    assert_eq!(
        ledger.remaining_period("psi", "", "crawl:b", 100).unwrap(),
        99
    );
    assert_eq!(
        ledger
            .remaining_period("psi", "", "2026-10-18", 100)
            .unwrap(),
        99
    );
    let spring = Timestamp::from_db("2026-03-10T07:00:00Z").unwrap();
    ledger
        .reserve_limits_at(
            &[("gsc".into(), "".into(), "2026-03-08".into(), 100)],
            spring,
        )
        .unwrap();
    ledger.reserve("gsc", "", spring, 100).unwrap();
    assert_eq!(
        ledger
            .remaining_period("gsc", "", "2026-03-08", 100)
            .unwrap(),
        99
    );
}

// spec: INTEG-019
#[tokio::test]
async fn registered_source_mocks_obey_all_four_scenarios() {
    // I02–I05 füllen registry() und scenario_path(); jeder registrierte INTEG-001-Dienst
    // wird automatisch geprüft. Solange sie fehlen, bleibt der Rahmenmock aktiv geprüft.
    let kinds = ["ga4", "gsc", "psi", "majestic", "ahrefs", "moz"];
    let mut mocks: Vec<_> = os_testsite::mocks::registry()
        .into_iter()
        .filter(|mock| kinds.contains(&mock.name()))
        .collect();
    mocks.push(Arc::new(FixtureApi::default()));
    let server = MockServer::start(mocks.clone()).await.unwrap();
    let client = http::client(&Default::default()).unwrap();
    for kind in kinds.into_iter().chain(["common"]) {
        let Some(mock) = mocks.iter().find(|mock| mock.name() == kind) else {
            continue;
        };
        for (scenario, status) in [
            ("ok", 200),
            ("empty", 200),
            ("error", 401),
            ("throttle", 429),
        ] {
            let response = client
                .get(format!("{}{}", server.base_url(kind), mock.scenario_path()))
                .header("X-Scenario", scenario)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{kind} / {scenario}");
            if scenario == "empty" && kind == "common" {
                let value: serde_json::Value =
                    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
                assert_eq!(value["items"], serde_json::json!([]), "{kind}");
            }
        }
    }
    server.stop().await;
}

// spec: INTEG-007
#[tokio::test]
async fn slow_completion_keeps_other_network_requests_polled() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct PollSource(Arc<AtomicUsize>);
    impl ApiSource for PollSource {
        fn kind(&self) -> ApiKind {
            ApiKind::Psi
        }
        fn status(&self) -> ApiStatus {
            Source {
                failed: Default::default(),
            }
            .status()
        }
        fn fetch<'a>(
            &'a self,
            _: &'a [UrlRef],
            _: &'a ApiCtx,
        ) -> os_integrations::BoxFuture<'a, Result<Vec<os_model::ApiValueWrite>, ApiError>>
        {
            Box::pin(async move {
                tokio::task::yield_now().await;
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            })
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let urls: Vec<_> = (0..3)
        .map(|i| UrlRef {
            url_id: None,
            url: format!("https://fixture.test/{i}"),
        })
        .collect();
    let queue = Queue::default();
    queue
        .run_with(
            Arc::new(PollSource(count.clone())),
            &urls,
            &context(),
            2,
            |_, _, _| {
                let count = count.clone();
                async move {
                    tokio::time::timeout(Duration::from_secs(1), async {
                        while count.load(Ordering::SeqCst) < 3 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .expect("Netzwerk muss während des Commits weiterlaufen");
                    Ok(())
                }
            },
        )
        .await
        .unwrap();
    queue.wait().await;
    assert_eq!(queue.status().unwrap().urls_done, 3);
}
