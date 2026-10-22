//! Local SMTP fixture coverage; no OS keychain and no external services.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{App, AppConfig};
use os_fetch::secrets::{MemorySecretStore, SecretStore};
use os_model::{settings::NetworkSettings, ProxyKind, ProxyMode};
use os_sched::mail::{self, Account, Auth, Completion, Policy, Tls};
use os_testsite::mocks::smtp::{Config, Security, SmtpMock};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn fixture() -> Completion {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/out-m2-mail/completion.json"
    ))
    .unwrap()
}
fn network() -> NetworkSettings {
    let mut n = NetworkSettings::default();
    n.proxy.mode = ProxyMode::None;
    n
}
fn account(mock: &SmtpMock, tls: Tls) -> Account {
    Account {
        id: "fixture".into(),
        label: "Local".into(),
        host: "localhost".into(),
        port: mock.addr.port(),
        tls,
        username: "crawler".into(),
        from: "crawler@site.test".into(),
        auth: Auth::Password,
        allow_insecure: tls == Tls::None,
        client_id: None,
    }
}
async fn app_call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(common::ctx(), name, args)
        .await
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}
async fn test_account(app: &App) -> Value {
    let op = app_call(
        app,
        "mail.testAccount",
        json!({"id":"fixture","to":"recipient@site.test"}),
    )
    .await;
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        app.ops().wait(op["opId"].as_str().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.error.is_none(), "{result:?}");
    result.result.unwrap()
}
fn input(mock: &SmtpMock, password: Value) -> Value {
    json!({"account":{"id":"fixture","label":"Local SMTP","host":"localhost","port":mock.addr.port(),"tls":"none","username":"crawler","from":"crawler@site.test","password":password,"allowInsecure":true}})
}
// spec: OUT-280
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn commands_persist_only_metadata_keep_null_password_and_deliver_or_report_535() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets.clone())).unwrap();
    app_call(
        &app,
        "settings.set",
        json!({"patch":{"network":{"proxy":{"mode":"none"}}}}),
    )
    .await;
    let info = app_call(
        &app,
        "mail.saveAccount",
        input(&mock, json!("fixture-password")),
    )
    .await;
    assert_eq!(info["hasSecret"], true);
    assert!(info.get("password").is_none());
    let disk = std::fs::read_to_string(
        app.config()
            .paths
            .settings
            .join("mail")
            .join("fixture.json"),
    )
    .unwrap();
    assert!(!disk.contains("fixture-password"));
    app_call(&app, "mail.saveAccount", input(&mock, Value::Null)).await;
    assert_eq!(
        secrets.get("mail/fixture").as_deref(),
        Some("fixture-password")
    );
    assert_eq!(test_account(&app).await["ok"], true);
    let messages = mock.messages();
    assert_eq!(messages.len(), 1);
    assert!(messages[0].data.contains("multipart/alternative"));
    assert!(messages[0].data.contains("text/plain"));
    assert!(messages[0].data.contains("text/html"));
    secrets.set("mail/fixture", "wrong-password");
    let failure = test_account(&app).await;
    assert_eq!(failure["ok"], false);
    assert_eq!(failure["error"]["args"]["smtpCode"], "535");
    assert!(!failure.to_string().contains("wrong-password"));
    app_call(&app, "mail.deleteAccount", json!({"id":"fixture"})).await;
    assert!(secrets.get("mail/fixture").is_none());
    assert_eq!(
        app_call(&app, "mail.listAccounts", Value::Null).await,
        json!([])
    );
}
// spec: OUT-280
#[tokio::test]
async fn cleartext_requires_explicit_consent_and_tls_modes_validate_test_ca() {
    for (security, tls) in [
        (Security::Starttls, Tls::Starttls),
        (Security::Tls, Tls::Tls),
    ] {
        let mock = SmtpMock::start(Config {
            security,
            ..Default::default()
        })
        .await
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let ca = dir.path().join("ca.pem");
        std::fs::write(&ca, &mock.ca_pem).unwrap();
        let secrets = MemorySecretStore::new();
        secrets.set("mail/fixture", "fixture-password");
        let mut n = network();
        n.trusted_certificates
            .push(ca.to_string_lossy().into_owned());
        let a = account(&mock, tls);
        mail::send(
            &a,
            &secrets,
            &n,
            &["recipient@site.test".into()],
            &fixture(),
        )
        .await
        .unwrap();
        assert_eq!(mock.messages().len(), 1);
        assert!(mail::send(
            &a,
            &secrets,
            &network(),
            &["recipient@site.test".into()],
            &fixture()
        )
        .await
        .is_err());
    }
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let mut a = account(&mock, Tls::None);
    a.allow_insecure = false;
    assert!(a.validate().unwrap_err().message.contains("bestätigen"));
}
// spec: OUT-282
#[test]
fn completion_fixture_matches_plaintext_golden_and_escapes_html() {
    let c = fixture();
    let (text, html) = c.render();
    assert_eq!(
        text,
        include_str!("../../../tests/fixtures/out-m2-mail/completion.txt")
    );
    assert!(!text.contains("private-value"));
    assert!(!text.contains("Unchanged"));
    assert!(html.contains("font-weight:bold"));
    let mut c = c;
    c.project = "<script>alert('x')</script>".into();
    c.changes = None;
    let (text, html) = c.render();
    assert!(text.contains("kein vorheriger Crawl"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    c.skip_empty = false;
    c.changes = fixture().changes;
    assert!(c.render().0.contains("Unchanged | 5 | 5 | +0 | 0.00%"));
}
// spec: OUT-281
#[tokio::test]
async fn scheduled_failure_prefix_and_empty_recipients_suppress_mail() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = MemorySecretStore::new();
    let a = account(&mock, Tls::None);
    mail::AccountStore::new(dir.path())
        .save(&a, Some("fixture-password"), &secrets)
        .unwrap();
    let mut c = fixture();
    c.error = Some("Crawl failed: fixture error".into());
    let mut p = Policy {
        account: Some(a.id),
        to: vec!["recipient@site.test".into()],
        ..Default::default()
    };
    assert!(mail::notify(dir.path(), &secrets, &network(), &p, &c)
        .await
        .unwrap());
    assert!(mock.messages()[0].data.contains("[FAILED]"));
    p.to.clear();
    assert!(!mail::notify(dir.path(), &secrets, &network(), &p, &c)
        .await
        .unwrap());
    assert_eq!(mock.messages().len(), 1);
}
// spec: CRAWL-122 [M2]
// spec: OUT-269
#[tokio::test]
async fn smtp_uses_http_connect_and_proxy_basic_auth_without_resolving_destination() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let destination = mock.addr;
    let proxy = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();
        let mut request = vec![];
        while !request.ends_with(b"\r\n\r\n") {
            request.push(client.read_u8().await.unwrap());
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("CONNECT unresolvable.invalid:"));
        assert!(request.contains("Proxy-Authorization: Basic cHJveHk6cHJveHktc2VjcmV0"));
        let mut target = tokio::net::TcpStream::connect(destination).await.unwrap();
        client
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let _ = tokio::io::copy_bidirectional(&mut client, &mut target).await;
    });
    let mut a = account(&mock, Tls::None);
    a.host = "unresolvable.invalid".into();
    let mut n = network();
    n.proxy.mode = ProxyMode::Manual;
    n.proxy.kind = ProxyKind::Http;
    n.proxy.host = "127.0.0.1".into();
    n.proxy.port = port;
    n.proxy.username = Some("proxy".into());
    let secrets = MemorySecretStore::new();
    secrets.set("mail/fixture", "fixture-password");
    secrets.set(os_fetch::net::PROXY_PASSWORD, "proxy-secret");
    mail::send(
        &a,
        &secrets,
        &n,
        &["recipient@site.test".into()],
        &fixture(),
    )
    .await
    .unwrap();
    proxy.await.unwrap();
    assert_eq!(mock.messages().len(), 1);
}

// spec: OUT-280
#[tokio::test]
async fn gmail_and_microsoft_oauth_use_pkce_send_scope_and_xoauth2_over_tls() {
    use os_integrations::oauth::{Authorization, Endpoints};
    let oauth =
        os_testsite::mocks::MockServer::start(vec![os_testsite::mocks::oauth::mock().unwrap()])
            .await
            .unwrap();
    let endpoints = Endpoints {
        authorize: format!("{}/authorize", oauth.base_url("oauth")),
        token: format!("{}/token", oauth.base_url("oauth")),
        revoke: format!("{}/revoke", oauth.base_url("oauth")),
    };
    for auth_kind in [Auth::Gmail, Auth::Microsoft365] {
        let auth = Authorization::begin_mail(
            "fixture-client",
            auth_kind == Auth::Microsoft365,
            &endpoints,
        )
        .await
        .unwrap();
        let scope = auth
            .url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .unwrap()
            .1
            .into_owned();
        assert!(if auth_kind == Auth::Gmail {
            scope == "https://mail.google.com/"
        } else {
            scope.contains("SMTP.Send") && scope.contains("offline_access")
        });
        let client = os_integrations::http::client(&network()).unwrap();
        let redirect = client
            .get(auth.url.clone())
            .send()
            .await
            .unwrap()
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let callback = tokio::spawn(async move { client.get(redirect).send().await.unwrap() });
        let clock = Arc::new(os_model::ManualClock::new(
            os_model::Timestamp::from_db("2099-01-01T00:00:00.000Z").unwrap(),
        ));
        let token = auth
            .finish(
                &os_integrations::http::client(&network()).unwrap(),
                "fixture-client",
                Some("fixture-client-secret"),
                &endpoints,
                clock,
            )
            .await
            .unwrap();
        callback.await.unwrap();
        assert!(token.scopes.iter().any(|s| scope.contains(s)));
        let mock = SmtpMock::start(Config {
            security: Security::Starttls,
            password: "fixture-access".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let ca = dir.path().join("ca.pem");
        std::fs::write(&ca, &mock.ca_pem).unwrap();
        let mut n = network();
        n.trusted_certificates
            .push(ca.to_string_lossy().into_owned());
        let mut a = account(&mock, Tls::Starttls);
        a.auth = auth_kind;
        a.client_id = Some("fixture-client".into());
        let secrets = MemorySecretStore::new();
        secrets.set("mail/fixture", &serde_json::to_string(&token).unwrap());
        let mut c = fixture();
        c.project = "fixture-access".into();
        c.error = Some("fixture-refresh".into());
        mail::send(&a, &secrets, &n, &["recipient@site.test".into()], &c)
            .await
            .unwrap();
        let messages = mock.messages();
        assert_eq!(messages.len(), 1);
        assert!(!messages[0].data.contains("fixture-access"));
        assert!(!messages[0].data.contains("fixture-refresh"));
    }
}

// spec: CRAWL-122 [M2]
// spec: OUT-269
#[tokio::test]
async fn socks5_authentication_and_manual_proxy_bypass_are_honored() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let destination = mock.addr;
    let proxy = tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();
        let mut greeting = [0; 3];
        client.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 1, 2]);
        client.write_all(&[5, 2]).await.unwrap();
        assert_eq!(client.read_u8().await.unwrap(), 1);
        let len = client.read_u8().await.unwrap();
        let mut username = vec![0; len as usize];
        client.read_exact(&mut username).await.unwrap();
        assert_eq!(username, b"proxy");
        let len = client.read_u8().await.unwrap();
        let mut password = vec![0; len as usize];
        client.read_exact(&mut password).await.unwrap();
        assert_eq!(password, b"proxy-secret");
        client.write_all(&[1, 0]).await.unwrap();
        let mut head = [0; 5];
        client.read_exact(&mut head).await.unwrap();
        assert_eq!(&head[..4], &[5, 1, 0, 3]);
        let mut hostname = vec![0; head[4] as usize];
        client.read_exact(&mut hostname).await.unwrap();
        assert_eq!(hostname, b"unresolvable.invalid");
        let _ = client.read_u16().await.unwrap();
        client
            .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
            .await
            .unwrap();
        let mut target = tokio::net::TcpStream::connect(destination).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut client, &mut target).await;
    });
    let secrets = MemorySecretStore::new();
    secrets.set("mail/fixture", "fixture-password");
    secrets.set(os_fetch::net::PROXY_PASSWORD, "proxy-secret");
    let mut n = network();
    n.proxy.mode = ProxyMode::Manual;
    n.proxy.kind = ProxyKind::Socks5;
    n.proxy.host = "127.0.0.1".into();
    n.proxy.port = port;
    n.proxy.username = Some("proxy".into());
    let mut a = account(&mock, Tls::None);
    a.host = "unresolvable.invalid".into();
    mail::send(
        &a,
        &secrets,
        &n,
        &["recipient@site.test".into()],
        &fixture(),
    )
    .await
    .unwrap();
    proxy.await.unwrap();
    a.host = "localhost".into();
    n.proxy.bypass.push("localhost".into());
    mail::send(
        &a,
        &secrets,
        &n,
        &["recipient@site.test".into()],
        &fixture(),
    )
    .await
    .unwrap();
    assert_eq!(mock.messages().len(), 2);
    n.proxy.bypass.clear();
    assert!(mail::send(
        &a,
        &secrets,
        &n,
        &["recipient@site.test".into()],
        &fixture()
    )
    .await
    .is_err());
    assert_eq!(
        mock.messages().len(),
        2,
        "failed proxy never falls back to direct SMTP"
    );
}

// spec: OUT-280
#[tokio::test]
async fn cancelling_a_stalled_tls_handshake_closes_the_socket() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut data = Vec::new();
        let count = tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut data))
            .await
            .unwrap()
            .unwrap();
        assert!(count > 0, "the TLS ClientHello reached the peer");
        count
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(50),
        os_fetch::net::tls_tunnel(tcp, "localhost", &network())
    )
    .await
    .is_err());
    assert!(peer.await.unwrap() > 0);
}

// spec: OUT-280
#[tokio::test]
async fn failed_metadata_replacement_preserves_the_previous_keychain_secret() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = mail::AccountStore::new(dir.path());
    let secrets = MemorySecretStore::new();
    secrets.set("mail/fixture", "previous-password");
    std::fs::create_dir_all(dir.path().join("mail").join("fixture.json")).unwrap();
    assert!(store
        .save(
            &account(&mock, Tls::None),
            Some("replacement-password"),
            &secrets
        )
        .is_err());
    assert_eq!(
        secrets.get("mail/fixture").as_deref(),
        Some("previous-password")
    );
}

// spec: OUT-280
#[tokio::test]
async fn oauth_application_secret_is_kept_only_in_the_injected_secret_store() {
    let mock = SmtpMock::start(Config::default()).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::new());
    let app = App::new(AppConfig::new(dir.path()).with_secrets(secrets.clone())).unwrap();
    let mut args = input(&mock, Value::Null);
    args["account"]["auth"] = json!("gmail");
    args["account"]["tls"] = json!("starttls");
    args["account"]["clientId"] = json!("fixture-client");
    args["account"]["clientSecret"] = json!("fixture-client-secret");
    let reply = app_call(&app, "mail.saveAccount", args).await;
    assert!(!reply.to_string().contains("fixture-client-secret"));
    let saved: Value = serde_json::from_str(&secrets.get("mail/fixture").unwrap()).unwrap();
    assert_eq!(saved["client_secret"], "fixture-client-secret");
    let disk = std::fs::read_to_string(
        app.config()
            .paths
            .settings
            .join("mail")
            .join("fixture.json"),
    )
    .unwrap();
    assert!(!disk.contains("fixture-client-secret"));
}

// spec: OUT-280
// spec: CRAWL-122 [M2]
// spec: OUT-269
#[tokio::test]
async fn oauth_token_requests_share_the_smtp_proxy_and_injected_proxy_secret() {
    let oauth =
        os_testsite::mocks::MockServer::start(vec![os_testsite::mocks::oauth::mock().unwrap()])
            .await
            .unwrap();
    let target = oauth.addr();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let proxy = tokio::spawn(async move {
        let (mut source, _) = listener.accept().await.unwrap();
        let mut request = vec![];
        while !request.ends_with(b"\r\n\r\n") {
            request.push(source.read_u8().await.unwrap());
        }
        let header = String::from_utf8(request.clone()).unwrap();
        assert!(header.starts_with("POST http://unresolvable.invalid/oauth/token "));
        assert!(
            header.contains("proxy-authorization: Basic cHJveHk6cHJveHktc2VjcmV0")
                || header.contains("Proxy-Authorization: Basic cHJveHk6cHJveHktc2VjcmV0")
        );
        let mut destination = tokio::net::TcpStream::connect(target).await.unwrap();
        destination.write_all(&request).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut source, &mut destination).await;
    });
    let secrets = MemorySecretStore::new();
    secrets.set(os_fetch::net::PROXY_PASSWORD, "proxy-secret");
    let mut n = network();
    n.proxy.mode = ProxyMode::Manual;
    n.proxy.host = "127.0.0.1".into();
    n.proxy.port = port;
    n.proxy.username = Some("proxy".into());
    let response = os_fetch::net::mail_token_response(
        &n,
        secrets.get(os_fetch::net::PROXY_PASSWORD).as_deref(),
        "http://unresolvable.invalid/oauth/token",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", "fixture"),
            ("refresh_token", "fixture-refresh"),
        ],
    )
    .await
    .unwrap();
    let token: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(token["access_token"], "fixture-access");
    proxy.abort();
}

// spec: OUT-282
#[tokio::test]
async fn credentials_are_masked_in_html_even_when_their_characters_are_escaped() {
    let mock = SmtpMock::start(Config {
        password: "x&y".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let secrets = MemorySecretStore::new();
    secrets.set("mail/fixture", "x&y");
    let mut completion = fixture();
    completion.project = "x&y".into();
    mail::send(
        &account(&mock, Tls::None),
        &secrets,
        &network(),
        &["recipient@site.test".into()],
        &completion,
    )
    .await
    .unwrap();
    let messages = mock.messages();
    assert_eq!(messages.len(), 1);
    assert!(!messages[0].data.contains("x&y"));
    assert!(!messages[0].data.contains("x&amp;y"));
    assert!(messages[0].data.contains("&lt;redacted&gt;"));
}
