//! WebSocket-Transport der E2E-Bridge (Feature `e2e-bridge`, `docs/plan/M2.md` 3.6).
//!
//! Lauscht nur auf `127.0.0.1` (ephemerer Port); jede Verbindung muss das Token der Startzeile als Abfrageparameter
//! `?token=<hex>` mitbringen, sonst antwortet der Handshake mit 401. Das Protokoll selbst steht in
//! [`bridge_proto`](super::bridge_proto).

use std::net::{Ipv4Addr, SocketAddr};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::handshake::client::generate_key;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;

use super::bridge_proto::BridgeSession;
use super::App;

/// Länge des Tokens in Hexzeichen (zwei Zufallsschlüssel zu je 22 Base64-Zeichen, je Zeichen zwei Hexziffern).
pub const TOKEN_HEX_LEN: usize = 88;

/// Eine laufende Bridge.
pub struct BridgeHandle {
    /// Port auf `127.0.0.1`.
    pub port: u16,
    /// Zufälliges Token (Hex).
    pub token: String,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl BridgeHandle {
    /// Die Zeile für stdout: `{"port":<n>,"token":"<hex>"}`.
    pub fn startup_line(&self) -> String {
        serde_json::json!({ "port": self.port, "token": self.token }).to_string()
    }

    /// Adresse für den Client.
    pub fn url(&self) -> String {
        format!("ws://127.0.0.1:{}/?token={}", self.port, self.token)
    }

    /// Beendet den Server (laufende Verbindungen enden mit dem Abbruch ihrer Anfragen).
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

/// Zufälliges Token aus der Zufallsquelle des WebSocket-Handshakes (`Sec-WebSocket-Key`, 16 Zufallsbytes), hexadezimal.
fn new_token() -> String {
    let mut hex = String::new();
    for _ in 0..2 {
        for b in generate_key().bytes().filter(|b| *b != b'=') {
            hex.push_str(&format!("{b:02x}"));
        }
    }
    hex
}

fn token_matches(uri_query: Option<&str>, token: &str) -> bool {
    let Some(q) = uri_query else {
        return false;
    };
    q.split('&')
        .filter_map(|kv| kv.strip_prefix("token="))
        .any(|t| constant_time_eq(t.as_bytes(), token.as_bytes()))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Startet die Bridge auf einem freien Loopback-Port.
pub async fn start(app: App) -> std::io::Result<BridgeHandle> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();
    let token = new_token();
    let (stop, stop_rx) = watch::channel(false);
    let task = tokio::spawn(accept_loop(listener, app, token.clone(), stop_rx));
    Ok(BridgeHandle {
        port,
        token,
        stop,
        task,
    })
}

async fn accept_loop(
    listener: TcpListener,
    app: App,
    token: String,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                tokio::spawn(serve(stream, app.clone(), token.clone(), stop.clone()));
            }
        }
    }
}

async fn serve(stream: TcpStream, app: App, token: String, mut stop: watch::Receiver<bool>) {
    // der Fehlertyp ist durch die Callback-Signatur von tungstenite vorgegeben
    #[allow(clippy::result_large_err)]
    let check = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        if token_matches(req.uri().query(), &token) {
            Ok(resp)
        } else {
            let mut reject = ErrorResponse::new(Some("invalid token".to_string()));
            *reject.status_mut() = StatusCode::UNAUTHORIZED;
            Err(reject)
        }
    };
    let Ok(ws) = tokio_tungstenite::accept_hdr_async(stream, check).await else {
        return;
    };
    let (mut sink, mut source) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let session = BridgeSession::new(app, out_tx);
    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            msg = source.next() => match msg {
                Some(Ok(Message::Text(text))) => session.handle_text(text.as_str()),
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
    session.close();
    drop(session);
    writer.abort();
}
