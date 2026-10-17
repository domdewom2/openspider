use os_model::{ProxyKind, ProxyMode};
use os_render::api::Credentials;
use os_render::{LaunchOpts, RenderOpts, Renderer};
use os_testsite::TestSite;
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

struct Proxy {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Proxy {
    async fn start(target_port: u16, auth: bool) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut client,_)) = accepted else { break; }; let log = log.clone();
                        tasks.spawn(async move {
                            loop {
                                let mut bytes = Vec::new();
                                while !bytes.ends_with(b"\r\n\r\n") {
                                    let mut byte = [0]; if client.read_exact(&mut byte).await.is_err() { return; }
                                    bytes.push(byte[0]); if bytes.len() > 64*1024 { return; }
                                }
                                let request = String::from_utf8(bytes).unwrap();
                                let line = request.lines().next().unwrap().to_owned();
                                log.lock().unwrap().push(request.clone());
                                if auth && !request.to_ascii_lowercase().contains("proxy-authorization: basic chjvehk6c2vjcmv0") {
                                    if client.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"fixture\"\r\nContent-Length: 0\r\n\r\n").await.is_err() { return; }
                                    continue;
                                }
                                let mut target = TcpStream::connect(("127.0.0.1",target_port)).await.unwrap();
                                if line.starts_with("CONNECT ") {
                                    client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
                                } else {
                                    let mut parts = line.split_whitespace(); let method = parts.next().unwrap(); let url = parts.next().unwrap();
                                    let path = url.split_once("://").unwrap().1.split_once('/').map(|(_,p)| format!("/{p}")).unwrap_or_else(|| "/".into());
                                    let headers = request.lines().skip(1).filter(|l| !l.to_ascii_lowercase().starts_with("proxy-") && !l.to_ascii_lowercase().starts_with("connection:") && !l.is_empty()).collect::<Vec<_>>().join("\r\n");
                                    target.write_all(format!("{method} {path} HTTP/1.1\r\n{headers}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
                                }
                                let _ = tokio::io::copy_bidirectional(&mut client,&mut target).await;
                                return;
                            }
                        });
                    },
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => {}
                }
            }
        });
        Self {
            port,
            requests,
            task,
        }
    }
}
fn quick() -> RenderOpts {
    RenderOpts {
        ajax_timeout_ms: 200,
        screenshots: false,
        ..Default::default()
    }
}

#[tokio::test]
// spec: CRAWL-122
async fn requires_chromium_manual_proxy_auth_connect_and_bypass() {
    let site = TestSite::start("render-r01").await.unwrap();
    let proxy = Proxy::start(site.port(), true).await;
    let dir = tempfile::tempdir().unwrap();
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, site.ca_cert_pem()).unwrap();
    let mut launch = LaunchOpts::default();
    launch.network.proxy.mode = ProxyMode::Manual;
    launch.network.proxy.kind = ProxyKind::Http;
    launch.network.proxy.host = "127.0.0.1".into();
    launch.network.proxy.port = proxy.port;
    launch.network.proxy.username = Some("proxy".into());
    launch.proxy_credentials = Some(Credentials {
        username: "proxy".into(),
        password: "secret".into(),
    });
    launch
        .network
        .trusted_certificates
        .push(ca.to_string_lossy().into());
    let renderer = Renderer::launch(launch.clone())
        .await
        .expect("requires_chromium");
    let result = renderer
        .render(&site.url("/blocked.html"), &quick())
        .await
        .unwrap();
    assert!(result.dom_html.contains("data-private-script=\"loaded\""));
    {
        let log = proxy.requests.lock().unwrap();
        for path in [
            "/blocked.html",
            "/private-js/app.js",
            "/missing.css",
            "/failed.js",
        ] {
            assert!(
                log.iter().any(|r| r.lines().next().unwrap().contains(path)),
                "{log:?}"
            );
        }
        assert!(log.iter().any(|r| r
            .to_ascii_lowercase()
            .contains("proxy-authorization: basic chjvehk6c2vjcmv0")));
    }
    let auth_opts = RenderOpts {
        credentials: Some(Credentials {
            username: "fixture".into(),
            password: "password".into(),
        }),
        ..quick()
    };
    renderer
        .render(&site.url("/auth"), &auth_opts)
        .await
        .unwrap();
    assert!(site.requests().iter().any(|r| r
        .header("authorization")
        .is_some_and(|v| v == "Basic Zml4dHVyZTpwYXNzd29yZA==")));
    assert!(!site
        .requests()
        .iter()
        .any(|r| r.header("proxy-authorization").is_some()));
    let secure = renderer
        .render(&format!("{}/csr.html", site.https_base_url()), &quick())
        .await
        .unwrap();
    assert!(secure.dom_html.contains("<title>rendered</title>"));
    assert!(proxy
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.starts_with("CONNECT ")));
    renderer.close().await;
    launch.network.proxy.bypass.push("127.0.0.1".into());
    let direct = Renderer::launch(launch).await.unwrap();
    proxy.requests.lock().unwrap().clear();
    direct
        .render(&site.url("/csr.html"), &quick())
        .await
        .unwrap();
    assert!(!proxy.requests.lock().unwrap().iter().any(|r| r
        .lines()
        .next()
        .unwrap()
        .contains("/csr.html")));
    direct.close().await;
}

#[tokio::test]
// spec: CRAWL-122
async fn unreachable_proxy_fails_at_launch() {
    let socket = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = socket.local_addr().unwrap().port();
    drop(socket);
    let mut launch = LaunchOpts::default();
    launch.network.proxy.mode = ProxyMode::Manual;
    launch.network.proxy.host = "127.0.0.1".into();
    launch.network.proxy.port = port;
    let error = Renderer::launch(launch).await.unwrap_err();
    assert!(error.to_string().contains("Proxy nicht erreichbar"));
}

#[tokio::test]
// spec: CRAWL-122
async fn requires_chromium_reports_bad_proxy_credentials() {
    let site = TestSite::start("render-r01").await.unwrap();
    let proxy = Proxy::start(site.port(), true).await;
    let mut launch = LaunchOpts::default();
    launch.network.proxy.mode = ProxyMode::Manual;
    launch.network.proxy.host = "127.0.0.1".into();
    launch.network.proxy.port = proxy.port;
    launch.proxy_credentials = Some(Credentials {
        username: "proxy".into(),
        password: "wrong".into(),
    });
    let cert_dir = tempfile::tempdir().unwrap();
    let ca = cert_dir.path().join("ca.pem");
    std::fs::write(&ca, site.ca_cert_pem()).unwrap();
    launch
        .network
        .trusted_certificates
        .push(ca.to_string_lossy().into());
    let renderer = Renderer::launch(launch).await.expect("requires_chromium");
    let error = renderer
        .render(&site.url("/csr.html"), &quick())
        .await
        .unwrap_err();
    assert_eq!(error, os_render::RenderError::ProxyAuthentication);
    let secure_error = renderer
        .render(&format!("{}/csr.html", site.https_base_url()), &quick())
        .await
        .unwrap_err();
    assert_eq!(secure_error, os_render::RenderError::ProxyAuthentication);
    renderer.close().await;
}
