//! Gemeinsame Hilfen der `http_*`-Tests (wird per `#[path]` eingebunden, enthält selbst keine Tests).
//!
//! - [`MiniServer`]: kleiner HTTP/1.1-Server auf Loopback, der die Antworten byte-genau steuert
//!   (Komprimierung, Chunked, Abbrüche, Hänger, Tröpfeln) und Verbindungen sowie gesendete Bytes
//!   zählt. Für Fälle, die `os-testsite` noch nicht kann (Wartet auf M1-P09), und für das
//!   Byte-Zählen (CRAWL-022).
//! - [`TokioClock`]: Uhr auf Basis der tokio-Zeit, damit `#[tokio::test(start_paused = true)]`
//!   Zeit- und Ratenkriterien simuliert (M1-Plan 2.8).
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_fetch::{
    Fetch, FetchConfig, FetchOutcome, FetchPurpose, FetchRequest, FetchResult, HttpFetcher,
    HttpResponse, MemorySecretStore,
};
use os_model::{Clock, CrawlConfig, NoResponseReason, Timestamp};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Url;

/// Uhr mit simulierter tokio-Zeit.
pub struct TokioClock {
    start: tokio::time::Instant,
}

impl TokioClock {
    pub fn new() -> Arc<Self> {
        Arc::new(TokioClock {
            start: tokio::time::Instant::now(),
        })
    }
}

impl TokioClock {
    /// Verstrichene (simulierte) Millisekunden seit dem Anlegen der Uhr.
    pub fn monotonic_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

impl Clock for TokioClock {
    fn now(&self) -> Timestamp {
        Timestamp::now_utc()
    }
    fn monotonic(&self) -> Duration {
        self.start.elapsed()
    }
}

/// Eingegangene Anfrage.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    /// Header, Namen klein.
    pub headers: Vec<(String, String)>,
    /// Laufende Nummer der Verbindung (ab 0).
    pub conn: usize,
    /// Zeit seit Serverstart (tokio-Zeit, simuliert unter `start_paused`).
    pub at: Duration,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }
}

/// Was der Server auf eine Anfrage tut.
pub enum Reply {
    /// Genau diese Bytes senden, Verbindung bleibt offen (Keep-Alive).
    Raw(Vec<u8>),
    /// Bytes senden und die Verbindung schließen.
    RawThenClose(Vec<u8>),
    /// Verbindung ohne Antwort schließen.
    Close,
    /// Nie antworten.
    Hang,
    /// Erst warten, dann die Antwort ausführen.
    After(Duration, Box<Reply>),
    /// Kopf sofort, den Body Byte für Byte im Abstand `interval`.
    Trickle {
        head: Vec<u8>,
        body: Vec<u8>,
        interval: Duration,
    },
}

type Handler = Arc<dyn Fn(&Request) -> Reply + Send + Sync>;

struct State {
    connections: AtomicUsize,
    bytes_sent: AtomicU64,
    requests: Mutex<Vec<Request>>,
    start: tokio::time::Instant,
}

/// Laufender Mini-Server.
pub struct MiniServer {
    pub addr: SocketAddr,
    state: Arc<State>,
    task: JoinHandle<()>,
}

impl Drop for MiniServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MiniServer {
    pub async fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(State {
            connections: AtomicUsize::new(0),
            bytes_sent: AtomicU64::new(0),
            requests: Mutex::new(Vec::new()),
            start: tokio::time::Instant::now(),
        });
        let handler: Handler = Arc::new(handler);
        let st = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let conn = st.connections.fetch_add(1, Ordering::SeqCst);
                let st = st.clone();
                let handler = handler.clone();
                tokio::spawn(serve_connection(stream, conn, st, handler));
            }
        });
        MiniServer { addr, state, task }
    }

    pub fn url(&self, path: &str) -> Url {
        Url::parse(&format!("http://{}{}", self.addr, path)).unwrap()
    }

    /// Anzahl angenommener TCP-Verbindungen.
    pub fn connections(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    /// Zahl der an Clients geschriebenen Bytes (Antworten).
    pub fn bytes_sent(&self) -> u64 {
        self.state.bytes_sent.load(Ordering::SeqCst)
    }

    pub fn requests(&self) -> Vec<Request> {
        self.state
            .requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Letzte Anfrage an einen Pfad.
    pub fn last_request(&self, path: &str) -> Option<Request> {
        self.requests().into_iter().rev().find(|r| r.path() == path)
    }
}

async fn serve_connection(mut stream: TcpStream, conn: usize, st: Arc<State>, handler: Handler) {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        // Anfragekopf lesen
        let end = loop {
            if let Some(pos) = find_subsequence(&buf, b"\r\n\r\n") {
                break pos + 4;
            }
            match stream.read(&mut tmp).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
            }
        };
        let head = String::from_utf8_lossy(&buf[..end]).into_owned();
        buf.drain(..end);
        let mut lines = head.split("\r\n");
        let first = lines.next().unwrap_or("");
        let mut parts = first.split(' ');
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("").to_string();
        let headers = lines
            .filter(|l| !l.is_empty())
            .filter_map(|l| l.split_once(':'))
            .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        let req = Request {
            method,
            target,
            headers,
            conn,
            at: st.start.elapsed(),
        };
        st.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(req.clone());
        let reply = handler(&req);
        if !execute(&mut stream, reply, &st).await {
            return;
        }
    }
}

/// Führt eine Antwort aus; `false`, wenn die Verbindung zu beenden ist.
fn execute<'a>(
    stream: &'a mut TcpStream,
    reply: Reply,
    st: &'a Arc<State>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
    Box::pin(async move {
        match reply {
            Reply::Raw(bytes) => {
                st.bytes_sent
                    .fetch_add(bytes.len() as u64, Ordering::SeqCst);
                stream.write_all(&bytes).await.is_ok()
            }
            Reply::RawThenClose(bytes) => {
                st.bytes_sent
                    .fetch_add(bytes.len() as u64, Ordering::SeqCst);
                let _ = stream.write_all(&bytes).await;
                let _ = stream.shutdown().await;
                false
            }
            Reply::Close => false,
            Reply::Hang => {
                std::future::pending::<()>().await;
                false
            }
            Reply::After(delay, inner) => {
                tokio::time::sleep(delay).await;
                execute(stream, *inner, st).await
            }
            Reply::Trickle {
                head,
                body,
                interval,
            } => {
                st.bytes_sent.fetch_add(head.len() as u64, Ordering::SeqCst);
                if stream.write_all(&head).await.is_err() {
                    return false;
                }
                for b in body {
                    tokio::time::sleep(interval).await;
                    st.bytes_sent.fetch_add(1, Ordering::SeqCst);
                    if stream.write_all(&[b]).await.is_err() {
                        return false;
                    }
                }
                true
            }
        }
    })
}

fn find_subsequence(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Baut eine HTTP/1.1-Antwort. `content-length` wird ergänzt, außer er oder `transfer-encoding`
/// steht schon in `headers`. Das Leerzeichen nach dem Code steht immer (auch ohne Reason-Phrase).
pub fn response(status: u16, reason: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} {reason}\r\n").into_bytes();
    let mut has_len = false;
    for (n, v) in headers {
        let lower = n.to_ascii_lowercase();
        if lower == "content-length" || lower == "transfer-encoding" {
            has_len = true;
        }
        out.extend_from_slice(format!("{n}: {v}\r\n").as_bytes());
    }
    if !has_len {
        out.extend_from_slice(format!("content-length: {}\r\n", body.len()).as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

/// Kopf einer Antwort ohne Body (für Tröpfeln und Abbrüche), mit festem `content-length`.
pub fn head_only(
    status: u16,
    reason: &str,
    headers: &[(&str, &str)],
    content_length: usize,
) -> Vec<u8> {
    let len = content_length.to_string();
    let mut h: Vec<(&str, &str)> = headers.to_vec();
    h.push(("content-length", &len));
    response(status, reason, &h, b"")
}

/// Body im Chunked-Format.
pub fn chunked(chunks: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for c in chunks {
        out.extend_from_slice(format!("{:x}\r\n", c.len()).as_bytes());
        out.extend_from_slice(c);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"0\r\n\r\n");
    out
}

/// 200-Antwort mit Text.
pub fn ok_text(body: &str) -> Reply {
    Reply::Raw(response(
        200,
        "OK",
        &[("content-type", "text/plain; charset=utf-8")],
        body.as_bytes(),
    ))
}

/// Standard-Abrufkonfiguration (Standard-Crawl-Konfiguration, Uhr nach Wahl).
pub fn config(clock: Arc<dyn Clock>) -> FetchConfig {
    FetchConfig::from_crawl_config(&CrawlConfig::default(), &MemorySecretStore::new(), clock)
        .expect("Standardkonfiguration ist gültig")
}

/// Abrufer mit Standardkonfiguration und angepasster Konfiguration.
pub fn fetcher_with(clock: Arc<dyn Clock>, tweak: impl FnOnce(&mut FetchConfig)) -> HttpFetcher {
    let mut cfg = config(clock);
    tweak(&mut cfg);
    HttpFetcher::new(cfg).expect("Abrufer baut")
}

pub fn page_request(url: Url) -> FetchRequest {
    FetchRequest {
        url,
        purpose: FetchPurpose::Page,
        cookies: None,
        max_body_bytes: 50 * 1024 * 1024,
        attempt: 0,
        body_limit: Default::default(),
    }
}

pub async fn get(fetcher: &HttpFetcher, url: Url) -> FetchResult {
    fetcher.fetch(page_request(url)).await
}

pub fn expect_response(r: FetchResult) -> HttpResponse {
    match r.outcome {
        FetchOutcome::Response(resp) => resp,
        FetchOutcome::NoResponse { reason, detail, .. } => {
            panic!("Antwort erwartet, erhalten: {reason:?} ({detail})")
        }
    }
}

pub fn expect_no_response(r: FetchResult) -> (NoResponseReason, String, u32) {
    match r.outcome {
        FetchOutcome::NoResponse {
            reason,
            detail,
            elapsed_ms,
        } => (reason, detail, elapsed_ms),
        FetchOutcome::Response(resp) => {
            panic!("keine Antwort erwartet, erhalten: Status {}", resp.status)
        }
    }
}

pub fn header<'a>(resp: &'a HttpResponse, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}
