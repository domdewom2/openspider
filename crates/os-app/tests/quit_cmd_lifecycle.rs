#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::{dto::*, runtime::hooks::WindowHost};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Default)]
struct Host(AtomicUsize);
impl WindowHost for Host {
    fn open_viz(&self, _: &str) -> Result<(), os_app::CmdError> {
        Ok(())
    }
    fn focus_main(&self) {}
    fn exit(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

// spec: CRAWL-126 [M2]
// spec: UI-001
#[tokio::test]
async fn quit_cancel_keeps_engine_running_and_confirmation_drains_before_exit() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let host = Arc::new(Host::default());
    app.set_window_host(host.clone());
    let fetch = os_engine::testing::SimFetcher::new(
        os_engine::testing::SimSite::new()
            .delay(Duration::from_millis(100))
            .page("https://fixture.test/", "<a href='/a'>A</a>")
            .page("https://fixture.test/a", "A"),
    );
    os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request("https://fixture.test/".into()),
        cfg(),
        fetch.clone(),
    )
    .await
    .unwrap();
    let decision: QuitDecision =
        serde_json::from_value(call(&app, "app.requestQuit", json!(null)).await).unwrap();
    assert_eq!(
        decision,
        QuitDecision::Confirm {
            reasons: vec![QuitReason::Crawl]
        }
    );
    call(&app, "app.confirmQuit", json!({"value":false})).await;
    assert_eq!(host.0.load(Ordering::SeqCst), 0);
    assert!(app
        .sessions()
        .current(&window())
        .unwrap()
        .engine
        .unwrap()
        .handle
        .lock()
        .unwrap()
        .is_some());
    let t = std::time::Instant::now();
    call(&app, "app.confirmQuit", json!({"value":true})).await;
    assert!(t.elapsed() < Duration::from_secs(3));
    assert_eq!(host.0.load(Ordering::SeqCst), 1);
    assert_eq!(fetch.inflight(), 0);
    app.read_pool()
        .run(key(&app), |f, _| {
            assert_eq!(f.get_meta("state")?.as_deref(), Some("Paused"));
            assert_eq!(f.get_meta("owner_pid")?.as_deref(), Some(""));
            Ok(())
        })
        .await
        .unwrap();
}

// spec: UI-001
#[tokio::test]
async fn quit_request_does_not_cancel_work_and_confirmation_waits_for_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let host = Arc::new(Host::default());
    app.set_window_host(host.clone());
    let op = app.ops().spawn("op-export", true, |cancel, _| {
        Box::pin(async move {
            while !cancel.is_cancelled() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(os_app::CmdError::new(
                os_app::ErrorCode::Cancelled,
                "err-cancelled",
            ))
        })
    });
    assert_eq!(
        app.request_quit().await,
        QuitDecision::Confirm {
            reasons: vec![QuitReason::Export]
        }
    );
    assert_eq!(app.ops().status(&op.op_id).unwrap().state, OpState::Running);
    call(&app, "app.confirmQuit", json!({"value":true})).await;
    assert_eq!(app.ops().running(), 0);
    assert_eq!(host.0.load(Ordering::SeqCst), 1);
}

// spec: UI-001
#[tokio::test]
async fn quit_reasons_distinguish_crawl_operations_from_exports_and_analysis() {
    for (label, reason) in [
        ("op-save-crawl", QuitReason::Crawl),
        ("op-remove", QuitReason::Crawl),
        ("op-recrawl", QuitReason::Crawl),
        ("op-export", QuitReason::Export),
        ("op-analysis", QuitReason::Analysis),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let op = app.ops().spawn(label, false, move |_, _| {
            Box::pin(async move {
                rx.await.unwrap();
                Ok(json!(null))
            })
        });
        assert_eq!(
            app.request_quit().await,
            QuitDecision::Confirm {
                reasons: vec![reason]
            },
            "{label}"
        );
        tx.send(()).unwrap();
        app.ops().wait(&op.op_id).await.unwrap();
        assert_eq!(app.request_quit().await, QuitDecision::Quit);
    }
}
