//! N01 aktiviert die Pfad-Parität nach der Integration von K09 (K2-26).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;
#[path = "../../os-analyze/tests/graph_fixture.rs"]
mod fx;
use os_analyze::graph::{GraphOptions, Snapshot};
use os_app::{
    dto::CrawlPathDto, pool::CrawlKey, runtime::sessions::SessionInfo, CallCtx, WindowId,
};
use os_model::{CrawlConfig, CrawlState};
use std::sync::Arc;

// spec: UI-091
#[tokio::test]
async fn depth_twelve_graph_path_equals_crawl_path() {
    let (dir, file) = fx::depth_twelve();
    let app = common::new_app(&dir.path().join("app"));
    let key = CrawlKey("fx-ui-depth".into());
    app.register_crawl(key.clone(), file.path());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path: file.path().into(),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Paused,
            writable: true,
            engine: None,
        },
    );
    let graph = Snapshot::capture(&file)
        .unwrap()
        .graph(&GraphOptions {
            expand_depth: 12,
            ..Default::default()
        })
        .unwrap();
    let response = app
        .dispatch(
            CallCtx::for_window("main"),
            "crawl.path",
            serde_json::json!({"urlId":13}),
        )
        .await
        .unwrap();
    let path: Vec<CrawlPathDto> = serde_json::from_value(response).unwrap();
    assert_eq!(path.len(), 13);
    assert_eq!(
        path.iter()
            .map(|p| (p.url_id, p.depth, p.address.as_str()))
            .collect::<Vec<_>>(),
        graph
            .nodes
            .iter()
            .map(|n| (n.id, n.depth, n.address.as_deref().unwrap()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        graph.edges,
        path.windows(2)
            .map(|p| (p[0].url_id, p[1].url_id))
            .collect::<Vec<_>>()
    );
}
