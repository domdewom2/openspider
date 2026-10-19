//! Vollständiger Integrationstest gegen die View-Commands aus M2-K06 (C9/K2-26).
#[path = "cmd_api_support.rs"]
mod support;
use support::*;
// spec: INTEG-009
#[tokio::test]
#[ignore = "M2: wartet auf M2-K06"]
async fn selection_all_resolves_real_view_version_and_exceptions() {
    let h = Harness::new().await;
    h.connect().await;
    let source = h.source(ApiKind::Psi);
    let view=h.call("view.open",json!({"spec":{"kind":"tab","tab":"Internal","filter":"All","sort":[],"search":null,"segment":null,"compare":null},"columns":["Address"]})).await.unwrap();
    let result=h.op("api.fetch",json!({"kinds":["psi"],"scope":{"kind":"selection","selection":{"kind":"all","viewId":view["viewId"],"version":view["version"],"except":[1]}}})).await.unwrap();
    assert_eq!(result["requested"], 5);
    assert_eq!(source.calls.load(Ordering::SeqCst), 5);
    h.server.stop().await;
}
