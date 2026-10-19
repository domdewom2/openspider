#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use os_app::{dto::ErrorCode, CallCtx};
use serde_json::json;
// spec: UI-140
// spec: OUT-020 [M2]
#[tokio::test]
async fn view_pages_search_sort_locate_update_and_close() {
    let fx = Fx::new();
    let mut spec = tab();
    spec["search"] = json!({"kind":"simple","text":"blog"});
    spec["sort"] = json!([{"column":"Word Count","dir":"desc"}]);
    let h = fx.open(spec, json!(["Address", "Word Count"])).await;
    assert_eq!(h["total"], 1);
    let rows = fx.rows(&h, 0, 500, json!(["Address", "Word Count"])).await;
    assert_eq!(rows["rowKeys"], json!([2]));
    assert_eq!(rows["cells"][0][1], 200);
    golden("ViewHandle.json", &h);
    golden("RowWindow.json", &rows);
    assert_eq!(
        fx.call("view.locate", json!({"viewId":h["viewId"],"urlId":2}))
            .await,
        0
    );
    assert!(fx
        .call("view.locate", json!({"viewId":h["viewId"],"urlId":99}))
        .await
        .is_null());
    let h = fx
        .call("view.update", json!({"viewId":h["viewId"],"spec":tab()}))
        .await;
    assert_eq!(h["total"], 6);
    let a = fx.rows(&h, 0, 2, json!(["Address"])).await;
    let b = fx.rows(&h, 2, 2, json!(["Address"])).await;
    assert_ne!(a["rowKeys"][0], b["rowKeys"][0]);
    let e = fx
        .app
        .dispatch(
            CallCtx::for_window("other"),
            "view.rows",
            json!({"viewId":h["viewId"],"version":42,"offset":0,"limit":1,"columns":[]}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
    fx.call("view.close", json!({"viewId":h["viewId"]})).await;
    let e = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "view.locate",
            json!({"viewId":h["viewId"],"urlId":2}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound);
}
// spec: UI-140
#[tokio::test]
async fn stale_versions_limits_no_selection_and_window_cleanup() {
    let fx = Fx::new();
    let h = fx.open(tab(), json!(["Address"])).await;
    fx.write("UPDATE crawl_meta SET value='43' WHERE key='change_seq'");
    // M2-K05: double buffer – version 42 stays readable until the change feed has maintained the view (K-1c).
    assert_eq!(fx.rows(&h, 0, 1, json!([])).await["version"], 42);
    let s = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    os_app::session::feed::refresh(&fx.app, &s).await.unwrap();
    let start = std::time::Instant::now();
    let e = loop {
        let r = fx
            .app
            .dispatch(
                CallCtx::for_window("main"),
                "view.rows",
                json!({"viewId":h["viewId"],"version":42,"offset":0,"limit":1,"columns":[]}),
            )
            .await;
        match r {
            Err(e) => break e,
            Ok(_) => assert!(start.elapsed() < std::time::Duration::from_secs(10)),
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    };
    assert_eq!(e.code, ErrorCode::Stale);
    let e = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "view.rows",
            json!({"viewId":h["viewId"],"version":42,"offset":0,"limit":501,"columns":[]}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let mut spec = links(true);
    spec["urlIds"] = json!([]);
    let empty = fx.open(spec, json!([])).await;
    assert_eq!(empty["emptyState"]["code"], "NO_SELECTION");
    fx.app.window_closed(&os_app::WindowId("main".into()));
    assert!(fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "view.close",
            json!({"viewId":h["viewId"]})
        )
        .await
        .is_err());
}
// spec: UI-032
// spec: UI-033
// spec: INTEG-084
// spec: ONPAGE-036
// spec: CRAWL-103
// spec: CRAWL-104
#[tokio::test]
async fn links_keep_occurrences_raw_target_and_filter_before_paging() {
    let mut cfg = os_model::CrawlConfig::default();
    cfg.spider.store_uncrawlable_links = true;
    let fx = Fx::config(cfg);
    let mut spec = links(true);
    spec["urlIds"] = json!([2, 2]);
    spec["filter"] =
        json!({"id":"Hyperlinks","position":"Content","excludeSelf":true,"excludeNofollow":true});
    let h = fx
        .open(
            spec,
            json!([
                "From",
                "To",
                "Target",
                "Link Path",
                "Link Position",
                "Original URL"
            ]),
        )
        .await;
    assert_eq!(h["total"], 1);
    let rows = fx
        .rows(
            &h,
            0,
            1,
            json!([
                "From",
                "To",
                "Target",
                "Link Path",
                "Link Position",
                "Original URL"
            ]),
        )
        .await;
    assert_eq!(rows["rowKeys"], json!([1]));
    assert_eq!(rows["cells"][0][2], "_blank");
    assert_eq!(rows["cells"][0][4], "Content");
    assert_eq!(rows["cells"][0][5], "/blog/a?sid=1");
    let h = fx
        .open(
            links(false),
            json!(["Anchor Text", "Status Code", "Status", "Link Crawlability"]),
        )
        .await;
    let r = fx
        .rows(
            &h,
            0,
            500,
            json!(["Anchor Text", "Status Code", "Status", "Link Crawlability"]),
        )
        .await;
    assert!(r["cells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r[0] == "Script" && r[1].is_null() && r[3] == "Uncrawlable"));
    golden("Outlinks.json", &r);
}
// spec: ONPAGE-172
// spec: TECH-089
// spec: TECH-146
#[tokio::test]
async fn fach_details_route_to_their_registered_owner_instead_of_fabricating_rows() {
    let fx = Fx::new();
    // K2-60: D03 (Bilder), D07 (Hreflang) und D08 (Anker) liefern echte Zeilen; M2-D02 (N-Gramme) ist noch Stub.
    for (spec, owner) in [(
        json!({"kind":"detail","detail":"nGrams","urlIds":[2],"params":{},"sort":[]}),
        "M2-D02",
    )] {
        let e = fx
            .app
            .dispatch(
                CallCtx::for_window("main"),
                "view.open",
                json!({"spec":spec,"columns":[]}),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert_eq!(e.args["owner"], owner);
    }
}

// spec: UI-140
// spec: ONPAGE-011 [M2]
#[tokio::test]
async fn update_refreshes_config_while_old_rows_remain_stale() {
    let fx = Fx::new();
    let h = fx.open(tab(), json!(["Address", "Title 1"])).await;
    let old = fx.open(tab(), json!(["Address"])).await;
    let mut cfg = os_model::CrawlConfig::default();
    cfg.extraction.http_headers = true;
    cfg.extraction.page_titles = false;
    fx.app.sessions().update_config(
        &os_app::pool::CrawlKey("fx-ui-basic".into()),
        std::sync::Arc::new(cfg),
    );
    let e = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "view.rows",
            json!({"viewId":h["viewId"],"version":42,"offset":0,"limit":1,"columns":[]}),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::Stale);
    fx.call("view.close", json!({"viewId":old["viewId"]})).await;
    let gone = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "view.close",
            json!({"viewId":old["viewId"]}),
        )
        .await
        .unwrap_err();
    assert_eq!(gone.code, ErrorCode::NotFound);
    let refreshed = fx
        .call("view.update", json!({"viewId":h["viewId"],"spec":tab()}))
        .await;
    assert_eq!(refreshed["viewId"], h["viewId"]);
    assert_eq!(
        refreshed["columns"][1]["needs"]["arg"],
        "extraction.page_titles"
    );
    let rows = fx.rows(&refreshed, 0, 1, json!([])).await;
    assert!(rows["cells"][0][1].is_null());
}

// spec: UI-140
#[tokio::test]
async fn empty_tab_only_offers_clear_search_when_a_search_is_active() {
    let fx = Fx::new();
    let mut spec = tab();
    spec["search"] = json!({"kind":"simple","text":"absent"});
    let h = fx.open(spec, json!(["Address"])).await;
    assert_eq!(h["emptyState"]["code"], "NO_MATCH");
    assert_eq!(h["emptyState"]["action"]["kind"], "clearSearch");
    fx.write("UPDATE urls SET in_tables=0");
    let h = fx.open(tab(), json!(["Address"])).await;
    assert_eq!(h["emptyState"]["code"], "NO_DATA_YET");
    assert!(h["emptyState"]["action"].is_null());
}
