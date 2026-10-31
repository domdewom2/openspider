//! Vollständiger Integrationstest gegen die View-Commands aus M2-K06 (C9/K2-26).
#[path = "cmd_api_support.rs"]
mod support;
use support::*;
// spec: INTEG-009
#[tokio::test]
async fn selection_all_resolves_real_view_version_and_exceptions() {
    let h = Harness::new().await;
    h.connect().await;
    let source = h.source(ApiKind::Psi);
    // A slow build returns a Building handle; selections require its Ready result.
    let readers = h.app.read_pool().exclusive(&h.key).await;
    let view=h.call("view.open",json!({"spec":{"kind":"tab","tab":"Internal","filter":"All","sort":[],"search":null,"segment":null,"compare":null},"columns":["Address"]})).await.unwrap();
    assert_eq!(view["state"], "building");
    drop(readers);
    let build = h
        .app
        .ops()
        .wait(view["op"]["opId"].as_str().unwrap())
        .await
        .unwrap();
    assert_eq!(build.state, os_app::dto::OpState::Done, "{build:?}");
    let view = build.result.unwrap();
    assert_eq!(view["state"], "ready");
    let result=h.op("api.fetch",json!({"kinds":["psi"],"scope":{"kind":"selection","selection":{"kind":"all","viewId":view["viewId"],"version":view["version"],"except":[1]}}})).await.unwrap();
    assert_eq!(result["requested"], 5);
    assert_eq!(source.calls.load(Ordering::SeqCst), 5);
    h.server.stop().await;
}
