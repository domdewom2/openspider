#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use os_app::CallCtx;
use serde_json::json;

// spec: UI-033
// spec: UI-018
#[tokio::test]
async fn link_count_and_first_rows_share_the_opened_generation_until_update() {
    let fx = Fx::new();
    let spec = links(false);
    let h = fx.open(spec.clone(), json!([])).await;
    fx.write("INSERT INTO links(source_id,target_id,target_raw,link_type,position,anchor,follow) SELECT source_id,target_id,target_raw,link_type,position,anchor,follow FROM links WHERE id=1; UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'");
    let rows = fx.rows(&h, 0, 20, json!([])).await;
    assert_eq!(
        rows["rowKeys"].as_array().unwrap().len() as u64,
        h["total"].as_u64().unwrap()
    );
    let fresh = fx
        .call("view.update", json!({"viewId":h["viewId"],"spec":spec}))
        .await;
    assert_eq!(
        fresh["total"].as_u64().unwrap(),
        h["total"].as_u64().unwrap() + 1
    );
    let rows = fx.rows(&fresh, 0, 20, json!([])).await;
    assert_eq!(
        rows["rowKeys"].as_array().unwrap().len() as u64,
        fresh["total"].as_u64().unwrap()
    );
}

// spec: UI-032
// spec: UI-033
// spec: UI-145
#[tokio::test]
async fn opened_and_updated_link_generations_serve_first_rows_without_more_reads() {
    for lane in ["detail-link-ids", "detail-window"] {
        for incoming in [true, false] {
            for update in [false, true] {
                let fx = Fx::new();
                let spec = links(incoming);
                let mut h = fx.open(spec.clone(), json!([])).await;
                if update {
                    h = fx
                        .call("view.update", json!({"viewId":h["viewId"],"spec":spec}))
                        .await;
                }
                // Hold either reader after open/update. The initial bounded page
                // must already be ready, even under reader load.
                let app = fx.app.clone();
                let (started, ready) = tokio::sync::oneshot::channel();
                let (release, held) = std::sync::mpsc::channel();
                let blocker = tokio::spawn(async move {
                    app.read_pool()
                        .run_cached(
                            os_app::pool::CrawlKey("fx-ui-basic".into()),
                            lane,
                            os_model::CancelToken::default(),
                            move |_, _| {
                                started.send(()).unwrap();
                                held.recv().unwrap();
                                Ok(())
                            },
                        )
                        .await
                        .unwrap();
                });
                ready.await.unwrap();
                let rows = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    fx.rows(&h, 0, 20, json!([])),
                )
                .await;
                release.send(()).unwrap();
                blocker.await.unwrap();
                let rows = rows.expect("first rows must not queue another database read");
                assert_eq!(
                    rows["rowKeys"].as_array().unwrap().len() as u64,
                    h["total"].as_u64().unwrap()
                );
                assert!(!rows["cells"].as_array().unwrap().is_empty());
            }
        }
    }
}

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
