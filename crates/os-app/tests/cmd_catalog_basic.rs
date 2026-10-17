#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;
// spec: ONPAGE-011 [M2]
// spec: UI-008 [P1]
#[tokio::test]
async fn configured_columns_include_extraction_gates_and_parity_filters() {
    let mut cfg = os_model::CrawlConfig::default();
    cfg.extraction.page_titles = false;
    cfg.extraction.hash = false;
    cfg.ui.parity_filters = true;
    let fx = Fx::config(cfg);
    let columns = fx.call("catalog.columns", json!({"tab":"Internal"})).await;
    let title = columns
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == "Title 1")
        .unwrap();
    assert_eq!(title["needs"]["code"], "FEATURE_DISABLED");
    let hash = columns
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == "Hash")
        .unwrap();
    assert_eq!(hash["needs"]["arg"], "extraction.hash");
    let tabs = fx.call("catalog.tabs", json!(null)).await;
    let internal = tabs
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "Internal")
        .unwrap();
    assert!(!internal["filters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "Fonts"));
    let h = fx.open(tab(), json!(["Address", "Title 1", "Hash"])).await;
    let rows = fx
        .rows(&h, 0, 2, json!(["Address", "Title 1", "Hash"]))
        .await;
    assert!(rows["cells"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r[1].is_null() && r[2].is_null()));
}
// spec: TECH-008
// spec: UI-061
#[tokio::test]
async fn counts_preserve_disabled_and_pending_states_and_overview_uses_same_counts() {
    let mut cfg = os_model::CrawlConfig::default();
    cfg.advanced.html_validation = false;
    let fx = Fx::config(cfg);
    let counts = fx.call("catalog.filterCounts", json!({})).await;
    assert_eq!(counts["version"], 42);
    let overview = fx.call("sidebar.overview", json!({})).await;
    for r in counts["filters"].as_array().unwrap() {
        let t = overview["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["tab"] == r["tab"])
            .unwrap();
        assert!(t["filters"].as_array().unwrap().contains(&r["filter"]));
    }
    assert!(counts["filters"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["filter"]["state"]["kind"] == "needsCrawlAnalysis"));
    let stable = |r: &&serde_json::Value| r["tab"] == "Internal";
    let golden_counts = json!({"version":counts["version"],"filters":counts["filters"].as_array().unwrap().iter().filter(stable).collect::<Vec<_>>()});
    golden("FilterCounts.json", &golden_counts);
    let golden_overview = json!({"summary":overview["summary"],"tabs":overview["tabs"].as_array().unwrap().iter().filter(stable).collect::<Vec<_>>()});
    golden("OverviewDto.json", &golden_overview);
    let summary = overview["summary"].as_array().unwrap();
    assert_eq!(summary[0]["count"], 8);
    assert_eq!(summary[4]["count"], 7);
    assert_eq!(summary[5]["count"], 6);
    assert_eq!(summary[6]["count"], 1);
    assert_eq!(summary[7]["count"], 2);
    assert_eq!(summary[8]["count"], 3);
}
// spec: UI-004
#[tokio::test]
async fn no_crawl_catalogs_and_unknown_names_are_typed() {
    let dir = tempfile::tempdir().unwrap();
    let app = os_app::App::new(os_app::AppConfig::new(dir.path())).unwrap();
    let ctx = os_app::CallCtx::for_window("main");
    assert!(!app
        .dispatch(ctx.clone(), "catalog.tabs", json!(null))
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let e = app
        .dispatch(ctx.clone(), "catalog.filterCounts", json!({}))
        .await
        .unwrap_err();
    assert_eq!(e.code, os_app::dto::ErrorCode::NoCrawl);
    let e = app
        .dispatch(ctx, "catalog.columns", json!({"tab":"not-a-tab"}))
        .await
        .unwrap_err();
    assert_eq!(e.code, os_app::dto::ErrorCode::InvalidInput);
}
