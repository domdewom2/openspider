//! Chromium-Archiv-Mock für die Bereitstellung nach K3-01 (M3-K01): liefert ein vom Test erzeugtes Archiv mit
//! HTTP-Range-Unterstützung und Störfällen. Nicht in [`super::registry`], weil das Archiv je Test entsteht:
//! `MockServer::start(vec![ChromiumMock::new(bytes).into_api()])`, Basis-URL `base_url("chromium")`.
//!
//! | Pfad | Verhalten |
//! |---|---|
//! | `/archive.zip` | 200 bzw. 206 mit `Content-Range` bei `Range: bytes=<n>-` |
//! | `/cut/archive.zip` | ohne Range: Kopf mit voller Länge, nach `cut` Byte Verbindungsfehler; mit Range wie `/archive.zip` |
//! | `/stall/archive.zip` | ohne Range: nach `cut` Byte keine weiteren Daten (Verbindung bleibt offen); mit Range wie `/archive.zip` |
//! | `/norange/archive.zip` | ignoriert `Range`, immer 200 mit vollem Inhalt |
use super::MockApi;
use axum::{
    body::Body,
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use std::sync::{Arc, Mutex};

/// Eine Anfrage an den Mock: Pfad (ohne `/chromium`) und `Range`-Header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRequest {
    pub path: String,
    pub range: Option<String>,
}

#[derive(Clone)]
pub struct ChromiumMock {
    archive: Arc<Vec<u8>>,
    cut: usize,
    log: Arc<Mutex<Vec<ArchiveRequest>>>,
}

impl ChromiumMock {
    /// Mock mit diesem Archiv; Störfälle brechen nach der Hälfte ab.
    pub fn new(archive: Vec<u8>) -> ChromiumMock {
        let cut = archive.len() / 2;
        ChromiumMock {
            archive: Arc::new(archive),
            cut,
            log: Arc::default(),
        }
    }
    /// Abbruchstelle der Störfälle in Byte.
    pub fn with_cut(mut self, cut: usize) -> ChromiumMock {
        self.cut = cut.min(self.archive.len());
        self
    }
    /// Für [`super::MockServer::start`].
    pub fn into_api(self) -> Arc<dyn MockApi> {
        Arc::new(self)
    }
    /// Bisherige Anfragen (geteilt mit allen Klonen).
    pub fn requests(&self) -> Vec<ArchiveRequest> {
        self.log.lock().unwrap().clone()
    }

    fn record(&self, path: &str, headers: &HeaderMap) -> Option<u64> {
        let range = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        self.log.lock().unwrap().push(ArchiveRequest {
            path: path.into(),
            range: range.clone(),
        });
        range
            .as_deref()
            .and_then(|r| r.strip_prefix("bytes="))
            .and_then(|r| r.strip_suffix('-'))
            .and_then(|n| n.parse().ok())
    }

    fn full(&self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/zip")
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, self.archive.len())
            .body(Body::from(self.archive.as_ref().clone()))
            .unwrap()
    }

    fn ranged(&self, start: Option<u64>) -> Response {
        let len = self.archive.len() as u64;
        match start {
            None => self.full(),
            Some(start) if start >= len => Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                .body(Body::empty())
                .unwrap(),
            Some(start) => Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, "application/zip")
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{}/{len}", len - 1),
                )
                .header(header::CONTENT_LENGTH, len - start)
                .body(Body::from(self.archive[start as usize..].to_vec()))
                .unwrap(),
        }
    }

    fn broken(&self, stall: bool) -> Response {
        let head = bytes::Bytes::copy_from_slice(&self.archive[..self.cut]);
        let first = futures_util::stream::once(async move { Ok::<_, std::io::Error>(head) });
        let body = if stall {
            Body::from_stream(futures_util::StreamExt::chain(
                first,
                futures_util::stream::pending(),
            ))
        } else {
            Body::from_stream(futures_util::StreamExt::chain(
                first,
                futures_util::stream::once(async {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "fixture interruption",
                    ))
                }),
            ))
        };
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/zip")
            .header(header::CONTENT_LENGTH, self.archive.len())
            .body(body)
            .unwrap()
    }
}

impl MockApi for ChromiumMock {
    fn name(&self) -> &'static str {
        "chromium"
    }
    fn base_url_key(&self) -> Option<&'static str> {
        None
    }
    fn router(&self) -> Router {
        let (a, b, c, d) = (self.clone(), self.clone(), self.clone(), self.clone());
        Router::new()
            .route(
                "/archive.zip",
                get(move |headers: HeaderMap| async move {
                    let start = a.record("/archive.zip", &headers);
                    a.ranged(start)
                }),
            )
            .route(
                "/cut/archive.zip",
                get(move |headers: HeaderMap| async move {
                    match b.record("/cut/archive.zip", &headers) {
                        None => b.broken(false),
                        start => b.ranged(start),
                    }
                }),
            )
            .route(
                "/stall/archive.zip",
                get(move |headers: HeaderMap| async move {
                    match c.record("/stall/archive.zip", &headers) {
                        None => c.broken(true),
                        start => c.ranged(start),
                    }
                }),
            )
            .route(
                "/norange/archive.zip",
                get(move |headers: HeaderMap| async move {
                    d.record("/norange/archive.zip", &headers);
                    d.full()
                }),
            )
    }
}
