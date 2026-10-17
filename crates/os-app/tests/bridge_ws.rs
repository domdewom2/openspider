//! WebSocket-Transport der E2E-Bridge (Feature `e2e-bridge`): Startzeile, Token, Rundreise mit der echten Laufzeit und
//! der Binärdatei `os-app-bridge`. Läuft mit `cargo test -p os-app --features e2e-bridge`.
#![cfg(feature = "e2e-bridge")]

#[path = "runtime_common.rs"]
mod common;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

use common::*;
use futures_util::{SinkExt, StreamExt};
use os_app::runtime::bridge;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str) -> Result<Ws, tokio_tungstenite::tungstenite::Error> {
    tokio_tungstenite::connect_async(url)
        .await
        .map(|(ws, _)| ws)
}

async fn call(ws: &mut Ws, id: u64, cmd: &str, args: Value) -> Value {
    ws.send(Message::text(
        json!({ "id": id, "cmd": cmd, "args": args }).to_string(),
    ))
    .await
    .unwrap();
    next_msg(ws, |v| v["id"] == json!(id)).await
}

async fn next_msg(ws: &mut Ws, pick: impl Fn(&Value) -> bool) -> Value {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("Zeitüberschreitung")
            .expect("Verbindung zu")
            .expect("WebSocket-Fehler");
        if let Message::Text(t) = m {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if pick(&v) {
                return v;
            }
        }
    }
}

// spec: UI-140
#[tokio::test]
async fn startup_line_binds_loopback_and_carries_a_random_hex_token() {
    let dir = tempfile::tempdir().unwrap();
    let a = bridge::start(new_app(dir.path())).await.unwrap();
    let b = bridge::start(new_app(dir.path())).await.unwrap();
    let line: Value = serde_json::from_str(&a.startup_line()).unwrap();
    assert_eq!(line["port"], a.port);
    assert_eq!(line["token"], a.token);
    assert_eq!(a.token.len(), bridge::TOKEN_HEX_LEN);
    assert!(a.token.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a.token, b.token, "Zufallstoken");
    assert_ne!(a.port, b.port);
    assert!(a.url().starts_with("ws://127.0.0.1:"));
    a.shutdown().await;
    b.shutdown().await;
}

// spec: UI-140
#[tokio::test]
async fn a_missing_or_wrong_token_is_refused_with_401() {
    let dir = tempfile::tempdir().unwrap();
    let handle = bridge::start(new_app(dir.path())).await.unwrap();
    for url in [
        format!("ws://127.0.0.1:{}/", handle.port),
        format!("ws://127.0.0.1:{}/?token=falsch", handle.port),
        format!("ws://127.0.0.1:{}/?token={}x", handle.port, handle.token),
    ] {
        match connect(&url).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
                assert_eq!(resp.status().as_u16(), 401, "{url}");
            }
            other => panic!("{url}: {:?}", other.map(|_| ())),
        }
    }
    assert!(connect(&handle.url()).await.is_ok());
    handle.shutdown().await;
}

// spec: UI-140
#[tokio::test]
async fn requests_roundtrip_over_the_websocket_with_the_real_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let handle = bridge::start(new_app(dir.path())).await.unwrap();
    let mut ws = connect(&handle.url()).await.unwrap();

    let v = call(&mut ws, 1, "app.info", Value::Null).await;
    assert_eq!(v["ok"]["version"], env!("CARGO_PKG_VERSION"));

    let v = call(
        &mut ws,
        2,
        "settings.set",
        json!({ "patch": { "appearance": { "zoomPct": 125 } } }),
    )
    .await;
    assert_eq!(v["ok"]["appearance"]["zoomPct"], 125);

    let v = call(
        &mut ws,
        3,
        "settings.set",
        json!({ "patch": { "appearance": { "zoomPct": 3 } } }),
    )
    .await;
    assert_eq!(v["err"]["code"], "InvalidInput");
    assert_eq!(v["err"]["field"], "appearance.zoom_pct");

    let v = call(&mut ws, 4, "nope.nope", Value::Null).await;
    assert_eq!(v["err"]["code"], "NotFound");

    // mehrere Anfragen gleichzeitig: jede Antwort trägt ihre Kennung
    for id in 10..20u64 {
        ws.send(Message::text(
            json!({ "id": id, "cmd": "uistate.load" }).to_string(),
        ))
        .await
        .unwrap();
    }
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < 10 {
        let v = next_msg(&mut ws, |v| {
            v["id"].as_u64().is_some_and(|i| (10..20).contains(&i))
        })
        .await;
        assert_eq!(v["ok"]["version"], 1);
        seen.insert(v["id"].as_u64().unwrap());
    }
    handle.shutdown().await;
}

// spec: UI-018
#[tokio::test]
async fn subscribe_replaces_the_tauri_channel_and_events_arrive_in_protocol_form() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let handle = bridge::start(app.clone()).await.unwrap();
    let mut ws = connect(&handle.url()).await.unwrap();
    ws.send(Message::text(
        json!({ "cmd": "__subscribe", "window": "main" }).to_string(),
    ))
    .await
    .unwrap();
    assert!(
        eventually(Duration::from_secs(3), || !app
            .subscribed_windows()
            .is_empty())
        .await,
        "Abonnement angelegt"
    );

    // eine Operation liefert Fortschritt und Ergebnis über den Tick
    let h = app.ops().spawn("op-ws", false, |_c, p| {
        Box::pin(async move {
            p.report(2, Some(2));
            Ok(json!("fertig"))
        })
    });
    app.ops().wait(&h.op_id).await.unwrap();
    assert_eq!(app.flush_tick(), 1);
    let v = next_msg(&mut ws, |v| v["event"] == "tick").await;
    assert_eq!(v["window"], "main");
    assert_eq!(v["data"]["ops"][0]["state"], "done");
    assert_eq!(v["data"]["ops"][0]["result"], "fertig");

    // Verbindung schließen → Abonnement endet
    ws.close(None).await.unwrap();
    assert!(
        eventually(Duration::from_secs(3), || app
            .subscribed_windows()
            .is_empty())
        .await,
        "Abonnement beendet"
    );
    handle.shutdown().await;
}

// spec: UI-140
#[tokio::test]
async fn the_bridge_binary_prints_the_startup_line_and_serves_requests() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_os-app-bridge"))
        .arg("--home")
        .arg(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let start: Value = serde_json::from_str(line.trim()).expect("Startzeile ist JSON");
    let url = format!(
        "ws://127.0.0.1:{}/?token={}",
        start["port"].as_u64().unwrap(),
        start["token"].as_str().unwrap()
    );
    let mut ws = connect(&url).await.unwrap();
    let v = call(&mut ws, 1, "app.info", Value::Null).await;
    // alles unter --home (OUT-227)
    let home = dir.path().display().to_string();
    assert!(v["ok"]["paths"]["settings"]
        .as_str()
        .unwrap()
        .starts_with(&home));
    assert!(v["ok"]["paths"]["exports"]
        .as_str()
        .unwrap()
        .starts_with(&home));
    let _ = child.kill();
    let _ = child.wait();
}

// spec: UI-140
#[test]
fn the_bridge_binary_rejects_bad_arguments_with_exit_code_2() {
    for args in [vec![], vec!["--home"], vec!["--home", "x", "--nope"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_os-app-bridge"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}
