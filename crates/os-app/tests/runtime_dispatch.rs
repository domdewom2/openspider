//! Dispatcher (M2-F03): Namensauflösung, Kontext, Argumentfehler, Panic-Schutz, Abbruchmarke im Aufruf, Stubs der
//! Domänen, die später Pakete füllen.

#[path = "runtime_common.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use futures_util::future::BoxFuture;
use os_app::dto::AppInfo;
use os_app::registry::{CmdResult, CommandMeta, Registry};
use os_app::runtime::{current_cancel, App, AppConfig, CallCtx};
use os_app::ErrorCode;
use serde_json::{json, Value};

fn echo<'a>(_app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { Ok(json!({ "window": ctx.window.0, "args": args })) })
}

fn boom<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async { panic!("handler kaputt") })
}

fn boom_early<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    panic!("schon vor dem Future")
}

fn wait_for_cancel<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        // die Marke des Aufrufs steht ohne Weiterreichen bereit (Task-Kontext)
        let token = current_cancel();
        while !token.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        Err(os_app::CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
    })
}

fn registry() -> Registry {
    let mut r = Registry::new();
    for (name, h) in [
        ("t.echo", echo as os_app::registry::Handler),
        ("t.boom", boom),
        ("t.boomEarly", boom_early),
        ("t.waitForCancel", wait_for_cancel),
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
    r
}

fn app(dir: &std::path::Path) -> App {
    App::with_registry(AppConfig::new(dir), registry()).unwrap()
}

// spec: UI-140
#[tokio::test]
async fn dispatch_passes_window_and_arguments_to_the_handler() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let ctx = CallCtx::for_window("settings");
    let v = app
        .dispatch(ctx, "t.echo", json!({ "a": 1 }))
        .await
        .unwrap();
    assert_eq!(v, json!({ "window": "settings", "args": { "a": 1 } }));
}

// spec: UI-140
#[tokio::test]
async fn unknown_command_is_not_found_and_a_cancelled_call_never_starts() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let e = app
        .dispatch(ctx(), "t.nope", Value::Null)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code, e.message_key.as_str()),
        (ErrorCode::NotFound, "err-unknown-command")
    );
    assert_eq!(e.args["command"], "t.nope");

    let c = ctx();
    c.cancel.cancel();
    let e = app.dispatch(c, "t.echo", Value::Null).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Cancelled);
}

// spec: UI-140
#[tokio::test]
async fn a_panicking_handler_becomes_an_internal_error_and_the_app_survives() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    for name in ["t.boom", "t.boomEarly"] {
        let e = app.dispatch(ctx(), name, Value::Null).await.unwrap_err();
        assert_eq!(e.code, ErrorCode::Internal, "{name}");
        assert_eq!(e.args["where"], name);
        assert!(e
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("kaputt") || d.contains("vor dem Future")));
    }
    app.dispatch(ctx(), "t.echo", Value::Null).await.unwrap();
}

// spec: UI-141
#[tokio::test]
async fn the_cancel_token_of_the_call_is_visible_to_the_handler() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = ctx();
    let token = c.cancel.clone();
    let a2 = app.clone();
    let call = tokio::spawn(async move { a2.dispatch(c, "t.waitForCancel", Value::Null).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!call.is_finished());
    token.cancel();
    let e = call.await.unwrap().unwrap_err();
    assert_eq!(e.code, ErrorCode::Cancelled);
}

// spec: UI-140
#[tokio::test]
async fn concurrent_calls_do_not_block_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let c = ctx();
    let token = c.cancel.clone();
    let a2 = app.clone();
    let slow = tokio::spawn(async move { a2.dispatch(c, "t.waitForCancel", Value::Null).await });
    let n = Arc::new(AtomicUsize::new(0));
    for _ in 0..20 {
        app.dispatch(ctx(), "t.echo", Value::Null).await.unwrap();
        n.fetch_add(1, Ordering::SeqCst);
    }
    assert_eq!(n.load(Ordering::SeqCst), 20);
    token.cancel();
    slow.await.unwrap().unwrap_err();
}

// spec: UI-127
#[tokio::test]
async fn app_info_reports_version_platform_and_the_effective_paths() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let v = app.dispatch(ctx(), "app.info", Value::Null).await.unwrap();
    let info: AppInfo = serde_json::from_value(v).unwrap();
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.platform, std::env::consts::OS);
    assert_eq!(info.arch, std::env::consts::ARCH);
    // alles unter dem Basisordner (OUT-227)
    let home = dir.path().display().to_string();
    for p in [
        &info.paths.settings,
        &info.paths.crawls,
        &info.paths.logs,
        &info.paths.cache,
        &info.paths.exports,
    ] {
        assert!(p.starts_with(&home), "{p} liegt nicht unter {home}");
    }
    assert!(info.paths.crawls.ends_with("crawls"));
    assert!(info.paths.exports.ends_with("Exports"));
}

// Domänen späterer Pakete antworten weiter mit dem Stub; die der Laufzeit nicht mehr
// spec: UI-140
#[tokio::test]
async fn only_domains_of_later_packages_are_still_stubs() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let implemented = [
        "app.info",
        "settings.get",
        "settings.set",
        "uistate.load",
        "uistate.save",
        "op.cancel",
        "events.subscribe",
        "events.setVisibility",
        "events.setInterval",
    ];
    for name in implemented {
        if let Err(e) = app.dispatch(ctx(), name, json!({})).await {
            assert_ne!(
                e.message_key, "err-not-implemented",
                "{name} ist noch ein Stub"
            );
        }
    }
    let e = app
        .dispatch(ctx(), "view.rows", json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.message_key, "err-not-implemented");
    assert_eq!(e.args["owner"], "M2-K06");
}
