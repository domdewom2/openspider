#[path = "framework_common.rs"]
mod common;
use os_analyze::{
    tabs::accessibility as a11y,
    views::{query_view, ColumnSelection, ViewQuery},
};
use os_model::{CrawlConfig, RenderingMode, UrlId, Value};
fn fixture() -> (tempfile::TempDir, os_store::CrawlFile, CrawlConfig) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "a11y.ospider", 21);
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/ui/fx-ui-a11y/store.sql"
        ))
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.accessibility = true;
    cfg.rendering.mode = RenderingMode::JavaScript;
    (dir, file, cfg)
}
// spec: TECH-251
#[test]
fn population_counts_rules_not_nodes_and_excludes_incomplete() {
    let (_d, file, cfg) = fixture();
    let mut q = ViewQuery::new("Accessibility", "All");
    q.columns = ColumnSelection::All;
    let p = query_view(&file, &cfg, &q).unwrap();
    assert_eq!(p.total, 2);
    let row = &p.rows[p.row_keys.iter().position(|id| id.0 == 1).unwrap()];
    for (name, n) in [
        ("All Violations", 3),
        ("Best Practice Violations", 2),
        ("WCAG 2.0 A Violations", 1),
        ("WCAG 2.0 AA Violations", 0),
    ] {
        assert_eq!(
            row[p.columns.iter().position(|c| c == name).unwrap()],
            Value::Int(n)
        );
    }
    assert_eq!(
        query_view(&file, &cfg, &ViewQuery::new("Accessibility", "image-alt"))
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        query_view(
            &file,
            &cfg,
            &ViewQuery::new("Accessibility", "color-contrast")
        )
        .unwrap()
        .total,
        0
    );
}
// spec: TECH-259
#[test]
fn details_support_all_results_and_paginate() {
    let (_d, file, cfg) = fixture();
    let ids = [UrlId(1)];
    let params = serde_json::json!({"result":"All"});
    let req = os_analyze::details::DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &[],
        offset: 0,
        limit: 500,
    };
    let rows = a11y::rule_details(&file, &cfg, &req).unwrap();
    assert_eq!(rows.total, 6);
    assert_eq!(rows.rows[0][2], Value::from("Violation"));
    assert_eq!(rows.rows[0][5], Value::Int(2));
    let params = serde_json::json!({"result":"Incomplete","ruleId":"color-contrast"});
    let req = os_analyze::details::DetailReq {
        params: &params,
        ..req
    };
    let rows = a11y::node_details(&file, &cfg, &req).unwrap();
    assert_eq!(rows.total, 1);
    assert_eq!(rows.rows[0][2], Value::from("p"));
}

// spec: TECH-250
#[test]
fn rendering_and_extraction_gates_hide_the_population() {
    let (_d, file, mut cfg) = fixture();
    cfg.rendering.mode = RenderingMode::Text;
    assert!(matches!(
        os_analyze::views::state_of(&file, &cfg, "Accessibility", "All").unwrap(),
        os_model::FilterState::NeedsRendering
    ));
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.extraction.accessibility = false;
    assert!(matches!(
        os_analyze::views::state_of(&file, &cfg, "Accessibility", "All").unwrap(),
        os_model::FilterState::NeedsConfig(_)
    ));
}
// spec: TECH-259
#[test]
fn ui_parameters_and_stable_node_keys_survive_sorting_and_paging() {
    let (_d, file, cfg) = fixture();
    let ids = [UrlId(1)];
    let params =
        serde_json::json!({"rule":a11y::rule("image-alt").unwrap().issue,"result":"Violation"});
    let sort = [os_analyze::SortKey {
        column: "Location on Page".into(),
        descending: true,
    }];
    let req = os_analyze::details::DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &sort,
        offset: 1,
        limit: 1,
    };
    let page = a11y::node_details(&file, &cfg, &req).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.rows[0][2], Value::from("#a"));
    let keys = a11y::node_keys(&file, &req).unwrap();
    assert_eq!(keys, [1]);
    let node = a11y::node(&file, 1, keys[0]).unwrap().unwrap();
    assert_eq!(node.target, "#a");
    assert!(a11y::node(&file, 2, keys[0]).unwrap().is_none());
    let params = serde_json::json!({});
    let req = os_analyze::details::DetailReq {
        params: &params,
        sort: &[],
        offset: 0,
        limit: 500,
        ..req
    };
    assert_eq!(a11y::rule_details(&file, &cfg, &req).unwrap().total, 6);
}

// spec: TECH-250
// spec: TECH-251
// spec: TECH-259
#[tokio::test]
async fn requires_chromium_engine_persists_audit_in_the_crawl_file() {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.rendering.ajax_timeout_s = 0.0;
    cfg.rendering.page_timeout_s = 60;
    cfg.rendering.screenshots.enabled = false;
    cfg.extraction.accessibility = true;
    let fx =
        os_engine::testing::crawl_fixture_from("ui/fx-ui-a11y", "/result-types.html", cfg.clone())
            .await;
    let page = query_view(
        &fx.file,
        &cfg,
        &ViewQuery::new("Accessibility", "image-alt"),
    )
    .unwrap();
    assert_eq!(page.total, 1);
    let url = page.row_keys[0];
    let count: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM a11y_rules WHERE url_id=?1",
            [url.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 93);
    let params = serde_json::json!({"ruleId":"image-alt","result":"Violation"});
    let ids = [url];
    let nodes = a11y::node_details(
        &fx.file,
        &cfg,
        &os_analyze::details::DetailReq {
            url_ids: &ids,
            params: &params,
            sort: &[],
            offset: 0,
            limit: 500,
        },
    )
    .unwrap();
    assert_eq!(nodes.total, 2);
}
