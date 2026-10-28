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

// spec: UI-033
// spec: UI-042
// spec: UI-145
#[tokio::test]
async fn offset_details_invalidate_and_reload_after_real_insertions() {
    for (spec, insertion) in [
        (json!({"kind":"images","urlIds":[1],"search":null,"sort":[{"column":"Alt Text","dir":"asc"}]}),
         "INSERT INTO image_instances(page_id,image_id,src_raw,source,alt_present,alt,position) VALUES(1,5,'/new.png','img',1,'AAA',1)"),
        (json!({"kind":"detail","detail":"headers","urlIds":[1],"params":{},"sort":[{"column":"Header Name","dir":"asc"}]}),
         "INSERT INTO http_headers(url_id,direction,position,name,value) VALUES(1,'response',10,'AAA','new')"),
    ] {
        let fx = Fx::new();
        let h = fx.open(spec.clone(), json!([])).await;
        let first = fx.rows(&h, 0, 1, json!([])).await;
        assert_eq!(first["cells"].as_array().unwrap().len(), 1);
        fx.write(&format!("{insertion}; UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'"));
        let error = fx.app.dispatch(CallCtx::for_window("main"), "view.rows",
            json!({"viewId":h["viewId"],"version":h["version"],"offset":1,"limit":1,"columns":[]})).await.unwrap_err();
        assert_eq!(error.code, os_app::ErrorCode::Stale);
        let fresh = fx.call("view.update", json!({"viewId":h["viewId"],"spec":spec})).await;
        let all = fx.rows(&fresh, 0, 20, json!([])).await;
        assert_eq!(all["cells"].as_array().unwrap().len() as u64, fresh["total"].as_u64().unwrap());
        let mut pages = vec![];
        for offset in 0..fresh["total"].as_u64().unwrap() {
            let page = fx.rows(&fresh, offset, 1, json!([])).await;
            pages.extend(page["cells"].as_array().unwrap().clone());
        }
        assert_eq!(pages, *all["cells"].as_array().unwrap());
        assert_ne!(first["cells"][0], all["cells"][0], "insert must precede the old first row");
    }
}
