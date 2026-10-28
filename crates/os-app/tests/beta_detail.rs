#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use os_app::CallCtx;
use serde_json::json;

// spec: UI-030
// spec: UI-032
// spec: UI-033
// spec: UI-035
// spec: UI-036
// spec: UI-145
#[tokio::test]
async fn detail_rows_remain_readable_when_writer_advances_between_open_and_rows() {
    let fx = Fx::new();
    for spec in [
        links(true),
        links(false),
        json!({"kind":"resources","urlIds":[1],"filter":{"id":"All"},"search":null,"sort":[]}),
        json!({"kind":"images","urlIds":[1],"search":null,"sort":[]}),
    ] {
        let h = fx.open(spec, json!([])).await;
        fx.write("UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'");
        let rows = fx.rows(&h, 0, 20, json!([])).await;
        assert_eq!(rows["version"], h["version"]);
        assert_eq!(
            rows["cells"].as_array().unwrap().len(),
            rows["rowKeys"].as_array().unwrap().len()
        );
        let error = fx
            .app
            .dispatch(
                CallCtx::for_window("main"),
                "view.rows",
                json!({"viewId":h["viewId"],"version":0,"offset":0,"limit":1,"columns":[]}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, os_app::ErrorCode::Stale);
        fx.call("view.close", json!({"viewId":h["viewId"]})).await;
    }
}
