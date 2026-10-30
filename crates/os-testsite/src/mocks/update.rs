//! Update fixtures and a logging CONNECT proxy; all sockets use loopback ports.
//! HTTP transport is provided by MockServer; HTTPS-required clients must reject it.
use super::MockApi;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use serde_json::Value;
use std::sync::Arc;

pub struct UpdateMock;
pub fn mock() -> Option<Arc<dyn MockApi>> {
    Some(Arc::new(UpdateMock))
}

fn manifest(version: &str, scheme: &str, signature: &str) -> Value {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/update/manifest.json"
    ))
    .expect("update fixture JSON");
    value["version"] = version.into();
    for platform in value["platforms"]
        .as_object_mut()
        .expect("platforms")
        .values_mut()
    {
        platform["url"] = format!("{scheme}://updates.test/update/artifact").into();
        platform["signature"] = signature.into();
    }
    value
}
impl MockApi for UpdateMock {
    fn name(&self) -> &'static str {
        "update"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        None
    }
    fn router(&self) -> Router {
        Router::new()
            .route(
                "/valid/latest.json",
                get(|| async {
                    Json(manifest(
                        "1.0.1",
                        "https",
                        include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
                    ))
                }),
            )
            .route(
                "/tampered/latest.json",
                get(|| async { Json(manifest("9.0.0", "https", "tampered")) }),
            )
            .route(
                "/older/latest.json",
                get(|| async {
                    Json(manifest(
                        "0.0.1",
                        "https",
                        include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
                    ))
                }),
            )
            .route(
                "/http/latest.json",
                get(|| async {
                    Json(manifest(
                        "1.0.1",
                        "http",
                        include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
                    ))
                }),
            )
            .route("/artifact", get(|| async { "fixture update artifact" }))
            .route(
                "/interrupted",
                get(|| async {
                    let chunks = futures_util::stream::unfold(0, |step| async move {
                        match step {
                            0 => Some((
                                Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"partial")),
                                1,
                            )),
                            1 => {
                                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                Some((
                                    Err(std::io::Error::new(
                                        std::io::ErrorKind::ConnectionReset,
                                        "fixture interruption",
                                    )),
                                    2,
                                ))
                            }
                            _ => None,
                        }
                    });
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_LENGTH, "4096")
                        .body(Body::from_stream(chunks))
                        .unwrap()
                }),
            )
    }
}

/// TLS fixture using TestSite's ephemeral CA and full request log.
pub async fn https_fixture() -> std::io::Result<crate::TestSite> {
    crate::TestSite::start("update").await
}
/// Loopback-only CONNECT proxy: forwards TLS unchanged, records CONNECT request headers.
pub struct FixtureProxy {
    pub addr: std::net::SocketAddr,
    log: Arc<std::sync::Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl FixtureProxy {
    pub async fn start(target: std::net::SocketAddr) -> std::io::Result<Self> {
        assert!(target.ip().is_loopback());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let requests = log.clone();
        let task = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            while let Ok((mut client, _)) = listener.accept().await {
                let requests = requests.clone();
                tokio::spawn(async move {
                    let mut head = Vec::new();
                    while !head.ends_with(b"\r\n\r\n") && head.len() < 16384 {
                        let mut byte = [0];
                        if client.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        head.push(byte[0]);
                    }
                    requests
                        .lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&head).into_owned());
                    if !head.starts_with(b"CONNECT ") {
                        return;
                    }
                    let Ok(mut upstream) = tokio::net::TcpStream::connect(target).await else {
                        return;
                    };
                    if client
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await
                        .is_ok()
                    {
                        let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
                    }
                });
            }
        });
        Ok(Self { addr, log, task })
    }
    pub fn requests(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}
impl Drop for FixtureProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::MockServer;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn request(server: &MockServer, route: &str) -> String {
        let mut stream = tokio::net::TcpStream::connect(server.addr()).await.unwrap();
        stream
            .write_all(
                format!(
                    "GET /update/{route} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut data = Vec::new();
        stream.read_to_end(&mut data).await.unwrap();
        String::from_utf8(data).unwrap()
    }

    // spec: OUT-250 [M3]
    #[tokio::test]
    async fn update_routes_serve_fixture_scenarios() {
        let server = MockServer::start(vec![mock().unwrap()]).await.unwrap();
        for (route, version, scheme, signature) in [
            (
                "valid/latest.json",
                "1.0.1",
                "https",
                include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
            ),
            ("tampered/latest.json", "9.0.0", "https", "tampered"),
            (
                "older/latest.json",
                "0.0.1",
                "https",
                include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
            ),
            (
                "http/latest.json",
                "1.0.1",
                "http",
                include_str!("../../../../tests/fixtures/update/artifact.sig").trim(),
            ),
        ] {
            let raw = request(&server, route).await;
            assert!(raw.starts_with("HTTP/1.1 200"));
            let value: Value = serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(value["version"], version);
            assert_eq!(value["platforms"]["darwin-aarch64"]["signature"], signature);
            assert!(value["platforms"]["darwin-aarch64"]["url"]
                .as_str()
                .unwrap()
                .starts_with(scheme));
        }
        assert!(request(&server, "artifact")
            .await
            .contains("fixture update artifact"));
        let interrupted = request(&server, "interrupted").await;
        let (headers, body) = interrupted.split_once("\r\n\r\n").unwrap();
        assert!(headers.contains("content-length: 4096"));
        assert_eq!(body, "partial");
        server.stop().await;
    }
}
