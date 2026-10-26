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
// spec: CRAWL-037
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
                .all(|entry| entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_none_or(|ext| ext != "ospider"))
    );
    assert!(app.sessions().current(&window()).is_none());

    // Replace exactly the former I06 stub contract with a connected API-only upload.
    let ga = os_testsite::mocks::ga4::Ga4Mock::default();
    ga.configure(|cfg| {
        cfg.rows = vec![
            os_testsite::mocks::ga4::MockRow::sessions("https://integ.test/keep/a", 12.0),
            os_testsite::mocks::ga4::MockRow::sessions("https://integ.test/keep/b", 8.0),
        ]
    });
    let server = os_testsite::mocks::MockServer::start(vec![Arc::new(ga)])
        .await
        .unwrap();
    os_integrations::credentials::Credentials::new(app.config().secrets.clone())
        .put(
            os_model::ApiKind::Ga4,
            os_integrations::credentials::Credential {
                account_id: "fixture".into(),
                label: "Fixture".into(),
                api_key: None,
                access_token: Some("fixture-token".into()),
                refresh_token: None,
                expires_at: None,
                client_secret: None,
                scopes: vec![],
            },
            true,
        )
        .unwrap();
    let mut config = cfg();
    config.api.ga4.enabled = true;
    config.api.ga4.property = Some(os_testsite::mocks::ga4::STREAM.into());
    config.api.ga4.base_url = Some(server.base_url("ga4"));
    config.scope.include = vec!["https://integ.test/keep/.*".into()];
    config.scope.exclude = vec![".*/private/.*".into()];
    let text = include_str!("../../os-integrations/src/crawl_glue/api-input-counts.txt");
    let upload = dir.path().join("urls.txt");
    std::fs::write(&upload, text).unwrap();
    let fetch = os_engine::testing::SimFetcher::new(os_engine::testing::SimSite::new());
    os_app::session::control::start_with_fetcher(
        &app,
        ctx(),
        StartRequest {
            source: StartSource::Apis {
                items: vec![upload.to_string_lossy().into_owned()],
            },
            scope: StartScope::Domain,
            profile: None,
        },
        config,
        fetch.clone(),
    )
    .await
    .unwrap();
    app.read_pool()
        .run(key(&app), |file, _| {
            let counters = file
                .conn()
                .prepare(
                    "SELECT key,value FROM crawl_counters WHERE key LIKE 'list_%' ORDER BY key",
                )?
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                counters,
                vec![
                    ("list_duplicates".into(), 2),
                    ("list_excluded".into(), 1),
                    ("list_ignored_lines".into(), 2),
                    ("list_invalid".into(), 1),
                    ("list_not_included".into(), 1),
                    ("list_read".into(), 7),
                    ("list_sitemaps".into(), 0),
                    ("list_to_crawl".into(), 2),
                ]
            );
            let entries = file
                .conn()
                .prepare("SELECT position,outcome,url_id FROM list_input ORDER BY position")?
                .query_map([], |row| {
                    Ok((
                        row.get::<_, u64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let outcomes: Vec<_> = entries
                .iter()
                .map(|(position, outcome, _)| (*position, outcome.as_str()))
                .collect();
            assert_eq!(
                outcomes,
                vec![
                    (1, "ignored_line"),
                    (2, "ok"),
                    (3, "duplicate"),
                    (4, "excluded"),
                    (5, "not_included"),
                    (6, "invalid"),
                    (7, "ignored_line"),
                    (8, "ok"),
                    (9, "duplicate"),
                ]
            );
            assert!(entries[1].2.is_some());
            assert_eq!(entries[1].2, entries[2].2);
            assert_eq!(entries[1].2, entries[8].2);
            assert!(entries[3..7].iter().all(|(_, _, id)| id.is_none()));
            let targets: u64 = file
                .conn()
                .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))?;
            assert_eq!(targets, 2);
            let values: u64 = file.conn().query_row(
                "SELECT COUNT(*) FROM api_values WHERE source='ga4' AND metric='sessions'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(values, 2);
            Ok(())
        })
        .await
        .unwrap();
    assert!(fetch.requests().is_empty());
    server.stop().await;
}
