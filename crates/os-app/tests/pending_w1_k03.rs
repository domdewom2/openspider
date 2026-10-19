//! Vollständige Integrationstests gegen die Commands anderer Welle-1-Pakete.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
use os_app::{pool::CrawlKey, runtime::sessions::SessionInfo, CallCtx, WindowId};
use os_model::{AnalysisStep, CrawlConfig, CrawlState, StepState};
use os_store::{CrawlFile, OpenMode};
use serde_json::json;
use std::sync::Arc;

// spec: UI-082 [M2]
// spec: ONPAGE-140 [M2]
#[tokio::test]
#[ignore = "M2: wartet auf M2-K04"]
async fn analysis_status_command_exposes_stale_and_needs_recrawl() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let path = common::make_crawl(dir.path(), "status.ospider");
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let old = CrawlConfig::default();
    file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code)
        VALUES(1,'https://site.test/','https://site.test/','https://site.test',1,'done','hyperlink',0,18,'HTML',200)",[]).unwrap();
    os_analyze::analysis::persist::write_step(
        &file,
        AnalysisStep::Content,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            AnalysisStep::Content,
            &old,
        )),
    )
    .unwrap();
    let mut cfg = old.clone();
    cfg.content.area.include_elements = vec!["main".into()];
    let key = CrawlKey("pending".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(cfg),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    let status = app
        .dispatch(CallCtx::for_window("main"), "analysis.status", json!(null))
        .await
        .unwrap();
    let content = status["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["step"] == "content")
        .unwrap();
    assert_eq!(content["state"], "needsRecrawl");
    assert_eq!(content["stale"], true);
}

// spec: UI-082 [M2]
#[tokio::test]
#[ignore = "M2: wartet auf M2-K07"]
async fn config_set_invalidates_only_the_near_duplicate_step() {
    let dir = tempfile::tempdir().unwrap();
    let app = common::new_app(dir.path());
    let path = common::make_crawl(dir.path(), "config.ospider");
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let mut old = CrawlConfig::default();
    old.analysis.near_duplicates = true;
    file.set_config(&old).unwrap();
    for step in [AnalysisStep::Content, AnalysisStep::NearDuplicates] {
        os_analyze::analysis::persist::write_step(
            &file,
            step,
            StepState::Done,
            None,
            Some(&os_analyze::state::step_config_hash(step, &old)),
        )
        .unwrap();
    }
    let key = CrawlKey("pending-config".into());
    app.register_crawl(key.clone(), &path);
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path,
            config: Arc::new(old.clone()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    let mut new = old;
    new.duplicates.near_threshold_pct = 83;
    let result = app
        .dispatch(
            common::ctx(),
            "config.set",
            json!({"schemaVersion":1,"generator":"K03 fixture","config":new}),
        )
        .await
        .unwrap();
    assert_eq!(result["invalidatedSteps"], json!(["near_duplicates"]));
    assert!(
        common::eventually(std::time::Duration::from_secs(5), || {
            os_analyze::aggregates::status::analysis_status(&file, &new).stale
                == vec![AnalysisStep::NearDuplicates]
        })
        .await
    );
    assert!(!os_analyze::analysis::persist::read_stale(&file)
        .unwrap()
        .contains(AnalysisStep::Content));
}
