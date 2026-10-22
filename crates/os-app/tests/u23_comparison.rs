//! U23 opt-in comparison read adapters against the unchanged C01 crawl fixtures.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "../../os-analyze/tests/compare_common.rs"]
mod fixture;
use fixture::*;
use os_app::{dto::*, App, AppConfig, CallCtx};
use serde_json::{json, Value as Json};

async fn open_pair(app: &App, old: &std::path::Path, new: &std::path::Path) -> CompareSession {
    serde_json::from_value(
        app.dispatch(
            CallCtx::for_window("main"),
            "compare.open",
            json!({"current":old,"previous":new,"mapping":[],"elements":[]}),
        )
        .await
        .unwrap(),
    )
    .unwrap()
}
fn args(s: &CompareSession, query: Json) -> Json {
    let mut query = query;
    query["current"] = json!(s.current.crawl_id);
    query["previous"] = json!(s.previous.crawl_id);
    json!({"viewId":"comparison-test","version":s.version,"offset":0,"limit":100,"columns":[],"comparison":query})
}
async fn read(app: &App, args: Json) -> Json {
    app.dispatch(CallCtx::for_window("main"), "view.rows", args)
        .await
        .unwrap()
}

// spec: OUT-142
#[tokio::test(flavor = "multi_thread")]
async fn overview_counts_and_every_class_page_use_the_same_fixture_population() {
    let old = v1().await;
    let new = v2().await;
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let s = open_pair(&app, old.file.path(), new.file.path()).await;
    let overview = app
        .dispatch(
            CallCtx::for_window("main"),
            "sidebar.overview",
            json!({"comparison":true}),
        )
        .await
        .unwrap();
    for row in overview["comparison"]["overview"].as_array().unwrap() {
        if let (Some(c), Some(p), Some(k)) = (
            row["current"].as_i64(),
            row["previous"].as_i64(),
            row["classes"].as_array(),
        ) {
            assert_eq!(
                c - p,
                k[0].as_i64().unwrap() + k[1].as_i64().unwrap()
                    - k[2].as_i64().unwrap()
                    - k[3].as_i64().unwrap()
            );
        }
    }
    let html = overview["comparison"]["overview"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["tab"] == "Internal" && r["filter"] == "HTML")
        .unwrap();
    for (i, class) in ["Added", "New", "Removed", "Missing"].iter().enumerate() {
        let page = read(
            &app,
            args(&s, json!({"tab":"Internal","filter":"HTML","class":class})),
        )
        .await;
        assert_eq!(page["total"], html["classes"][i]);
        assert_eq!(
            page["cells"].as_array().unwrap().len() as u64,
            page["total"].as_u64().unwrap()
        );
    }
    // Removed rows must bypass the old 4xx predicate and show the repaired Current row.
    let page = read(
        &app,
        args(
            &s,
            json!({"tab":"Response Codes","filter":"Client Error (4xx)","class":"Removed"}),
        ),
    )
    .await;
    assert!(page["total"].as_u64().unwrap() > 0);
    assert!(page["cells"].to_string().contains("missing.html"));
    let mut stale = args(&s, json!({"tab":"Internal","filter":"HTML"}));
    stale["comparison"]["current"] = json!("other-crawl");
    assert_eq!(
        app.dispatch(CallCtx::for_window("main"), "view.rows", stale)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Stale
    );
    assert_eq!(
        app.dispatch(
            CallCtx::for_window("other"),
            "view.rows",
            args(&s, json!({"tab":"Internal","filter":"HTML"}))
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
}
// spec: CRAWL-144
#[tokio::test(flavor = "multi_thread")]
async fn link_pages_preserve_anchor_changes_sources_and_one_sided_urls() {
    let old = v1().await;
    let new = v2().await;
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let s = open_pair(&app, old.file.path(), new.file.path()).await;
    let id = id_of(&new.file, &www("/c.html"));
    let page=read(&app,args(&s,json!({"tab":"inlinks","filter":"All","urlId":id,"source":"Auto","change":"Anchor Changed"}))).await;
    assert_eq!(page["total"], 1);
    let row = &page["cells"][0];
    assert_eq!(row[5], "Neu");
    assert_eq!(row[15], "Anchor Changed");
    assert_eq!(row[16], "Current");
    assert_eq!(row[17], "Alt");
    let id = id_of(&new.file, &www("/start.html"));
    let removed=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Current","change":"Removed"}))).await;
    assert!(removed["cells"].to_string().contains("b.html"));
    let before = read(
        &app,
        args(
            &s,
            json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Previous","change":"All"}),
        ),
    )
    .await;
    let after = read(
        &app,
        args(
            &s,
            json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Current","change":"All"}),
        ),
    )
    .await;
    assert!(before["cells"].to_string().contains("b.html"));
    assert!(!after["cells"].to_string().contains("b.html"));
    let id = id_of(&new.file, &www("/new1.html"));
    let one=read(&app,args(&s,json!({"tab":"outlinks","filter":"All","urlId":id,"source":"Previous","change":"Removed"}))).await;
    assert_eq!(one["filtersEnabled"], false);
}
// spec: TECH-301
#[tokio::test(flavor = "multi_thread")]
async fn change_projection_has_only_paired_html_and_numeric_deltas() {
    let old = v1().await;
    let new = v2().await;
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let s = open_pair(&app, old.file.path(), new.file.path()).await;
    let page = read(
        &app,
        args(&s, json!({"tab":"Change Detection","filter":"All"})),
    )
    .await;
    let cols = page["columns"].as_array().unwrap();
    assert_eq!(cols[0], "Address");
    assert!(cols.contains(&json!("Title 1 (Previous)")));
    assert!(cols.contains(&json!("Word Count Change %")));
    assert!(page["total"].as_u64().unwrap() > 0);
    for row in page["cells"].as_array().unwrap() {
        assert!(!row[0].as_str().unwrap().contains("new1.html"));
        assert_eq!(row.as_array().unwrap().len(), cols.len());
    }
    let cur = cols
        .iter()
        .position(|c| c == "Word Count (Current)")
        .unwrap();
    let prev = cols
        .iter()
        .position(|c| c == "Word Count (Previous)")
        .unwrap();
    let delta = cols.iter().position(|c| c == "Word Count Change").unwrap();
    let pct = cols
        .iter()
        .position(|c| c == "Word Count Change %")
        .unwrap();
    for row in page["cells"].as_array().unwrap() {
        if let (Some(c), Some(p)) = (row[cur].as_f64(), row[prev].as_f64()) {
            assert_eq!(row[delta].as_f64(), Some(c - p));
            if p == 0. {
                assert!(row[pct].is_null());
            }
        }
    }
}
