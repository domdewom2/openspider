#[path = "http_support.rs"]
mod support;
use os_fetch::{net, HttpFetcher};
use os_model::{ProxyKind, ProxyMode, SystemClock};
use std::sync::Arc;
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// spec: CRAWL-122
// spec: OUT-269
// spec: INTEG-016
#[tokio::test]
async fn shared_builder_authenticates_api_and_crawl_and_bypass_is_direct() {
    let proxy = MiniServer::start(|r| {
        if r.header("proxy-authorization") == Some("Basic dXNlcjpwYXNz") {
            ok_text("proxy")
        } else {
            Reply::Raw(response(407, "Proxy Authentication Required", &[], b""))
        }
    })
    .await;
    let target = MiniServer::start(|_| ok_text("direct")).await;
    let mut cfg = config(Arc::new(SystemClock));
    cfg.network.proxy.mode = ProxyMode::Manual;
    cfg.network.proxy.host = "127.0.0.1".into();
    cfg.network.proxy.port = proxy.url("/").port().unwrap();
    cfg.network.proxy.username = Some("user".into());
    cfg.proxy_password = Some("pass".into());
    net::probe_proxy(&cfg.network).await.unwrap();
    let f = HttpFetcher::new(cfg.clone()).unwrap();
    assert_eq!(
        expect_response(get(&f, "http://unresolved.invalid/api".parse().unwrap()).await).body,
        "proxy"
    );
    let api = net::client_builder_with_password(&cfg.network, Some("pass"))
        .build()
        .unwrap();
    assert_eq!(
        api.get("http://unresolved.invalid/api")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "proxy"
    );
    cfg.proxy_password = Some("wrong".into());
    assert_eq!(
        expect_response(
            get(
                &HttpFetcher::new(cfg.clone()).unwrap(),
                "http://unresolved.invalid/".parse().unwrap()
            )
            .await
        )
        .status,
        407
    );
    cfg.network.proxy.bypass = vec!["127.0.0.1".into()];
    assert_eq!(
        expect_response(get(&HttpFetcher::new(cfg.clone()).unwrap(), target.url("/")).await).body,
        "direct"
    );
    cfg.network.proxy.mode = ProxyMode::None;
    assert_eq!(
        expect_response(get(&HttpFetcher::new(cfg).unwrap(), target.url("/")).await).body,
        "direct"
    );
}

// spec: CRAWL-122
#[tokio::test]
async fn https_uses_connect_and_socks5_uses_remote_hostname() {
    let site = os_testsite::TestSite::start("crawl-m2-proxy")
        .await
        .unwrap();
    for socks in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let target = site.addr();
        let task = tokio::spawn(async move {
            let (mut client, _) = listener.accept().await.unwrap();
            if socks {
                let mut greeting = [0; 2];
                client.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting[0], 5);
                let mut methods = vec![0; greeting[1] as usize];
                client.read_exact(&mut methods).await.unwrap();
                client.write_all(&[5, 0]).await.unwrap();
                let mut request = [0; 4];
                client.read_exact(&mut request).await.unwrap();
                assert_eq!(request, [5, 1, 0, 3]);
                let n = client.read_u8().await.unwrap();
                let mut name = vec![0; n as usize];
                client.read_exact(&mut name).await.unwrap();
                assert_eq!(name, b"site.test");
                let _ = client.read_u16().await.unwrap();
                client
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                    .await
                    .unwrap();
            } else {
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    head.push(client.read_u8().await.unwrap());
                }
                assert!(String::from_utf8_lossy(&head).starts_with("CONNECT site.test:"));
                client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await
                    .unwrap();
            }
            let mut upstream = tokio::net::TcpStream::connect(target).await.unwrap();
            let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        });
        let mut cfg = config(Arc::new(SystemClock));
        cfg.extra_root_certs = vec![site.ca_cert_pem().into_bytes()];
        cfg.network.proxy.mode = ProxyMode::Manual;
        cfg.network.proxy.kind = if socks {
            ProxyKind::Socks5
        } else {
            ProxyKind::Http
        };
        cfg.network.proxy.host = addr.ip().to_string();
        cfg.network.proxy.port = addr.port();
        let f = HttpFetcher::new(cfg).unwrap();
        let r = expect_response(
            get(
                &f,
                format!("https://site.test:{}/", site.addr().port())
                    .parse()
                    .unwrap(),
            )
            .await,
        );
        assert_eq!(r.status, 200);
        assert_eq!(r.body, "proxy target fixture");
        drop(f);
        task.abort();
    }
}

// spec: CRAWL-122
#[tokio::test]
async fn connect_auth_failure_is_reported_as_407() {
    let proxy =
        MiniServer::start(|_| Reply::Raw(response(407, "Proxy Authentication Required", &[], b"")))
            .await;
    let mut cfg = config(Arc::new(SystemClock));
    cfg.network.proxy.mode = ProxyMode::Manual;
    cfg.network.proxy.host = "127.0.0.1".into();
    cfg.network.proxy.port = proxy.addr.port();
    let f = HttpFetcher::new(cfg).unwrap();
    let r = expect_response(get(&f, "https://site.test/".parse().unwrap()).await);
    assert_eq!(r.status, 407);
    assert!(r.status_text.contains("Proxy"));
}
