//! Cross-package integration; implementations owned by K03/K06/K07.
#[path = "session_common.rs"]
mod common;
use common::*;
use serde_json::json;

// spec: UI-013 [M2]
#[tokio::test]
#[ignore = "wartet auf M2-K03"]
async fn analysis_status_lists_all_step_states_after_a_real_run() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let f = create(&dir.path().join("analysis.ospider"), cfg());
    f.set_meta("state", "Completed").unwrap();
    open(&app, f.path()).await;
    let op = operation(&app, "analysis.start", json!({"steps":null})).await;
    assert_eq!(op.state, os_app::dto::OpState::Done);
    let status: os_app::dto::AnalysisStatus =
        serde_json::from_value(call(&app, "analysis.status", json!(null)).await).unwrap();
    assert_eq!(status.steps.len(), os_model::AnalysisStep::ALL.len());
    assert!(!status.running);
}

// spec: CRAWL-126 [M2]
#[tokio::test]
#[ignore = "wartet auf M2-K07"]
async fn named_profile_is_loaded_by_crawl_start() {
    let site = os_testsite::TestSite::start_dir(fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let config = serde_json::to_value(cfg()).unwrap();
    call(
        &app,
        "profile.save",
        json!({"name":"fixture","config":{"schemaVersion":2,"generator":"fixture","config":config},"overwrite":false}),
    )
    .await;
    let mut req = request(site.url("/"));
    req.profile = Some("fixture".into());
    call(&app, "crawl.start", serde_json::to_value(req).unwrap()).await;
    completed(&app).await;
    assert!(
        !app.sessions()
            .current(&window())
            .unwrap()
            .config
            .analysis
            .auto_at_end
    );
}

// spec: CRAWL-131
#[tokio::test]
#[ignore = "wartet auf M2-K06"]
async fn all_of_a_versioned_view_except_one_can_be_removed() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path());
    let f = create(&dir.path().join("selection.ospider"), cfg());
    f.set_meta("state", "Paused").unwrap();
    insert(&f, 1, 1);
    insert(&f, 2, 2);
    open(&app, f.path()).await;
    let view:os_app::dto::ViewHandle=serde_json::from_value(call(&app,"view.open",json!({"spec":{"kind":"tab","tab":"Internal","filter":"All","search":null,"sort":[],"segment":null,"compare":null},"columns":["Address"]})).await).unwrap();
    let result=operation(&app,"crawl.remove",json!({"selection":{"kind":"all","viewId":view.view_id,"version":view.version,"except":[1]}})).await;
    assert_eq!(result.state, os_app::dto::OpState::Done);
    assert_eq!(result.result.unwrap()["count"], 1);
}
