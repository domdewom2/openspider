#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "session_common.rs"]
mod common;
use common::*;
use futures_util::future::BoxFuture;
use os_app::{dto::*, pool::CrawlKey, runtime::hooks::CompletionBarrier};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
struct Barrier {
    entered: AtomicBool,
    release: tokio::sync::Notify,
}
struct SharedBarrier(Arc<Barrier>);
impl CompletionBarrier for SharedBarrier {
    fn wait(&self, _: CrawlKey) -> BoxFuture<'static, ()> {
        let b = self.0.clone();
        Box::pin(async move {
            b.entered.store(true, Ordering::SeqCst);
            b.release.notified().await;
        })
    }
}

// spec: UI-013
// spec: CRAWL-141
#[tokio::test]
async fn analysis_operation_runs_selected_steps_and_publishes_completion() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let f = create(&dir.path().join("analysis.ospider"), cfg());
    f.set_meta("state", "Completed").unwrap();
    insert(&f, 1, 1);
    open(&app, f.path()).await;
    let result = operation(&app, "analysis.start", json!({"steps":["canonicals"]})).await;
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
    let status: AnalysisStatus = serde_json::from_value(result.result.unwrap()).unwrap();
    assert!(!status.running);
    assert_eq!(status.percent, Some(100));
    assert_eq!(status.steps.len(), 1);
    assert_eq!(status.steps[0].step, "canonicals");
    assert!(app
        .dispatch(ctx(), "analysis.start", json!({"steps":["unknown"]}))
        .await
        .is_err());
    assert_eq!(
        f.get_meta("analysis_state").unwrap().as_deref(),
        Some("incomplete")
    );
}

// spec: CRAWL-141
#[tokio::test]
async fn automatic_analysis_waits_for_completion_barriers() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let mut cfg = cfg();
    cfg.analysis.auto_at_end = true;
    let b = Arc::new(Barrier {
        entered: AtomicBool::new(false),
        release: tokio::sync::Notify::new(),
    });
    app.crawl_events()
        .add_barrier("fixture-api", Arc::new(SharedBarrier(b.clone())));
    let fetch = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new().page(
        "https://fixture.test/",
        "<html><title>Fixture</title></html>",
    ));
    os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        request("https://fixture.test/".into()),
        cfg,
        fetch,
    )
    .await
    .unwrap();
    completed(&app).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !b.entered.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let before = app
        .read_pool()
        .run(key(&app), |f, _| f.get_meta("analysis_state"))
        .await
        .unwrap();
    assert_ne!(before.as_deref(), Some("completed"));
    let active = app.ops().active_statuses();
    assert_eq!(active.len(), 1);
    b.release.notify_one();
    let result = app.ops().wait(&active[0].op_id).await.unwrap();
    assert_eq!(result.state, OpState::Done, "{:?}", result.error);
}

// spec: INTEG-012
#[tokio::test]
async fn api_source_delegates_to_i06_and_never_starts_http_crawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let error=app.dispatch(ctx(),"crawl.start",json!({"source":{"kind":"apis","items":["https://fixture.test/"]},"scope":"domain","profile":null})).await.unwrap_err();
    assert_eq!(error.code, os_app::ErrorCode::InvalidInput);
    assert_eq!(error.message_key, "err-api-disconnected");
    assert!(error
        .detail
        .as_deref()
        .unwrap()
        .contains("Mindestens eine Integration"));
    assert!(
        !app.config().paths.crawls.exists()
            || std::fs::read_dir(&app.config().paths.crawls)
                .unwrap()
                .next()
                .is_none()
    );
    assert!(app.sessions().current(&window()).is_none());
}
