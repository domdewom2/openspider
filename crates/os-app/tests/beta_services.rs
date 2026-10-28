#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;

// spec: UI-129 [P1]
#[tokio::test]
async fn default_read_permissions_have_an_enabled_nonempty_tool_catalogue() {
    let fx = Fx::new();
    let tools = fx.call("mcp.tools", json!(null)).await;
    let enabled = tools
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["enabled"] == true)
        .collect::<Vec<_>>();
    assert!(!enabled.is_empty());
    assert!(enabled.iter().all(|t| t["class"] == "read"));
    assert!(enabled.iter().any(|t| t["name"] == "query_urls"));
}

// spec: UI-080 [M2]
// spec: UI-082 [M2]
#[tokio::test]
async fn a_short_analysis_retains_completion_status_and_publishes_a_notice() {
    let fx = Fx::new();
    fx.write("UPDATE crawl_meta SET value='Completed' WHERE key='state'");
    let handle = fx.call("analysis.start", json!({"steps":null})).await;
    let op_id = handle["opId"].as_str().unwrap();
    let result = fx.app.ops().wait(op_id).await.unwrap();
    assert_eq!(result.state, os_app::dto::OpState::Done);
    let tick = fx.app.assemble_tick().unwrap();
    assert!(!tick.analysis.unwrap().running);
    assert!(tick.notices.iter().any(|n| n.code == "analysis-completed"));
    let status = fx.call("analysis.status", json!(null)).await;
    assert_eq!(status["percent"], 100);
}
