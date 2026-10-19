//! Protokoll der E2E-Bridge (M2-F03, `docs/plan/M2.md` 3.6) ohne Transport: Anfrage/Antwort, Ereignisse, Abbruch.
//! Der WebSocket-Transport selbst steht in `bridge_ws.rs` (Feature `e2e-bridge`).

use std::time::Duration;

use futures_util::future::BoxFuture;
use os_app::registry::{CmdResult, CommandMeta};
use os_app::runtime::bridge_proto::BridgeSession;
use os_app::runtime::{current_cancel, App, AppConfig, CallCtx};
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

fn echo<'a>(_app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { Ok(json!({ "window": ctx.window.0, "args": args })) })
}

fn hang<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let token = current_cancel();
        while !token.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        Err(os_app::CmdError::new(
            os_app::ErrorCode::Cancelled,
            "err-cancelled",
        ))
    })
}

fn app_with_test_commands(dir: &std::path::Path) -> App {
    let mut r = os_app::default_registry();
    for (name, h) in [
        ("t.echo", echo as os_app::registry::Handler),
        ("t.hang", hang),
    ] {
        r.register(
            CommandMeta {
                name,
                args: "void",
                ret: "void",
                long_running: false,
            },
            h,
        );
    }
    App::with_registry(AppConfig::new(dir), r).unwrap()
}

async fn next(rx: &mut UnboundedReceiver<String>) -> Value {
    let text = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("Zeitüberschreitung")
        .expect("Kanal geschlossen");
    serde_json::from_str(&text).unwrap()
}

// spec: UI-140
#[tokio::test]
async fn a_request_gets_an_ok_response_with_the_same_id() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_with_test_commands(dir.path());
    let (tx, mut rx) = unbounded_channel();
    let session = BridgeSession::new(app, tx);
    session.handle_text(r#"{"id":1,"cmd":"t.echo","args":{"x":[1,2]}}"#);
    session.handle_text(r#"{"id":"abc","cmd":"t.echo","window":"settings"}"#);
    let mut got = [next(&mut rx).await, next(&mut rx).await];
    got.sort_by_key(|v| v["id"].is_string());
    assert_eq!(
        got[0],
        json!({ "id": 1, "ok": { "window": "main", "args": { "x": [1, 2] } } })
    );
    assert_eq!(
        got[1],
        json!({ "id": "abc", "ok": { "window": "settings", "args": null } })
    );
}

// spec: UI-140
#[tokio::test]
async fn errors_come_back_as_cmd_error_objects() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_with_test_commands(dir.path());
    let (tx, mut rx) = unbounded_channel();
    let session = BridgeSession::new(app, tx);

    session.handle_text(r#"{"id":2,"cmd":"gibt.es.nicht"}"#);
    let v = next(&mut rx).await;
    assert_eq!(v["id"], 2);
    assert_eq!(v["err"]["code"], "NotFound");
    assert_eq!(v["err"]["messageKey"], "err-unknown-command");
    assert_eq!(v["err"]["args"]["command"], "gibt.es.nicht");
    assert_eq!(v["err"]["retryable"], false);

    // K06 validates the implemented command arguments.
    session.handle_text(r#"{"id":3,"cmd":"view.rows","args":{}}"#);
    let v = next(&mut rx).await;
    assert_eq!(v["err"]["code"], "InvalidInput");

    session.handle_text("das ist kein JSON");
    let v = next(&mut rx).await;
    assert_eq!(v["id"], Value::Null);
    assert_eq!(v["err"]["messageKey"], "err-bridge-bad-json");

    session.handle_text(r#"{"id":4}"#);
    let v = next(&mut rx).await;
    assert_eq!(v["id"], 4);
    assert_eq!(v["err"]["messageKey"], "err-bridge-missing-cmd");
}

// spec: UI-018
#[tokio::test]
async fn subscribing_delivers_ticks_settings_changes_and_notices_as_events() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_with_test_commands(dir.path());
    let (tx, mut rx) = unbounded_channel();
    let session = BridgeSession::new(app.clone(), tx);

    session.handle_text(r#"{"id":1,"cmd":"__subscribe","window":"main"}"#);
    let v = next(&mut rx).await;
    assert_eq!(v["id"], 1);
    assert!(v["ok"]["id"].as_str().unwrap().starts_with("sub-"));

    // settingsChanged an alle Fenster
    session.handle_text(
        r#"{"id":2,"cmd":"settings.set","args":{"patch":{"appearance":{"zebra":false}}}}"#,
    );
    let mut seen_event = None;
    let mut seen_reply = false;
    for _ in 0..2 {
        let v = next(&mut rx).await;
        if v.get("event").is_some() {
            seen_event = Some(v);
        } else {
            assert_eq!(v["id"], 2);
            seen_reply = true;
        }
    }
    assert!(seen_reply);
    assert_eq!(
        seen_event.unwrap(),
        json!({ "event": "settingsChanged", "window": "main", "data": { "keys": ["appearance.zebra"] } })
    );

    // Tick mit einer Meldung
    app.push_notice(os_app::dto::Notice {
        level: os_app::dto::NoticeLevel::Info,
        code: "hallo".into(),
        args: Default::default(),
    });
    assert_eq!(app.flush_tick(), 1);
    let v = next(&mut rx).await;
    assert_eq!(v["event"], "tick");
    assert_eq!(v["window"], "main");
    assert_eq!(v["data"]["seq"], 1);
    assert_eq!(v["data"]["notices"][0]["code"], "hallo");

    // events.subscribe über den Katalognamen ist dasselbe wie __subscribe (ersetzt das Abonnement)
    session.handle_text(r#"{"id":5,"cmd":"events.subscribe","args":{"window":"settings"}}"#);
    assert_eq!(next(&mut rx).await["id"], 5);
    assert_eq!(app.subscribed_windows().len(), 2);

    session.handle_text(r#"{"id":6,"cmd":"__unsubscribe"}"#);
    assert_eq!(next(&mut rx).await["ok"]["removed"], 2);
    assert!(app.subscribed_windows().is_empty());
}

// spec: UI-141
#[tokio::test]
async fn abort_cancels_a_running_request_and_closing_cancels_all_of_them() {
    let dir = tempfile::tempdir().unwrap();
    let app = app_with_test_commands(dir.path());
    let (tx, mut rx) = unbounded_channel();
    let session = BridgeSession::new(app.clone(), tx);

    session.handle_text(r#"{"id":10,"cmd":"t.hang"}"#);
    session.handle_text(r#"{"id":11,"cmd":"t.hang"}"#);
    tokio::time::sleep(Duration::from_millis(30)).await;
    session.handle_text(r#"{"id":12,"cmd":"__abort","target":10}"#);
    let mut by_id = std::collections::HashMap::new();
    for _ in 0..2 {
        let v = next(&mut rx).await;
        by_id.insert(v["id"].to_string(), v);
    }
    assert_eq!(by_id["12"]["ok"]["aborted"], true);
    assert_eq!(by_id["10"]["err"]["code"], "Cancelled");

    // Abbruch einer unbekannten (oder bereits beendeten) Anfrage
    session.handle_text(r#"{"id":13,"cmd":"__abort","target":999}"#);
    assert_eq!(next(&mut rx).await["ok"]["aborted"], false);

    // Verbindung schließen: die noch laufende Anfrage 11 endet
    let (tx2, mut rx2) = unbounded_channel();
    let second = BridgeSession::new(app.clone(), tx2);
    second.handle_text(r#"{"id":1,"cmd":"t.hang"}"#);
    second.handle_text(r#"{"id":2,"cmd":"__subscribe","window":"viz-1"}"#);
    assert_eq!(next(&mut rx2).await["id"], 2);
    tokio::time::sleep(Duration::from_millis(30)).await;
    second.close();
    assert!(
        !app.subscribed_windows().iter().any(|w| w.0 == "viz-1"),
        "Abonnement beendet"
    );
    let v = next(&mut rx2).await;
    assert_eq!(v["err"]["code"], "Cancelled");
    // und die erste Sitzung räumt ihre Anfrage 11 beim Verwerfen ab
    drop(session);
    let v = next(&mut rx).await;
    assert_eq!(
        (v["id"].clone(), v["err"]["code"].clone()),
        (json!(11), json!("Cancelled"))
    );
}
