#[path = "framework_common.rs"]
mod common;
use os_analyze::{
    tabs::accessibility as a11y,
    views::{query_view, ColumnSelection, ViewQuery},
};
use os_model::{CrawlConfig, RenderingMode, UrlId, Value};
// K2-82: Beide Chromium-Crawls starten sonst gleichzeitig je einen Browser; auf langsamen
// CI-Runnern (macOS x64) überschreitet die Startprüfung dann ihre Zeitgrenze.
static CHROMIUM: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
// spec: UI-064 [P1]
#[tokio::test]
async fn requires_chromium_engine_persists_audit_in_the_crawl_file() {
    let _chromium = CHROMIUM.lock().await;
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
    for (rule, priority) in [
        ("image-alt", os_model::IssuePriority::High),
        ("heading-order", os_model::IssuePriority::Medium),
        ("empty-heading", os_model::IssuePriority::Low),
    ] {
        assert_eq!(
            query_view(&fx.file, &cfg, &ViewQuery::new("Accessibility", rule))
                .unwrap()
                .row_keys,
            [url]
        );
        let issue = os_analyze::issues::catalog()
            .iter()
            .find(|e| e.tab == "Accessibility" && e.filter == rule)
            .unwrap();
        assert_eq!(issue.issue_type, os_model::IssueType::Warning);
        assert_eq!(issue.priority, priority);
    }
    assert_eq!(
        query_view(
            &fx.file,
            &cfg,
            &ViewQuery::new("Accessibility", "color-contrast")
        )
        .unwrap()
        .total,
        0
    );
    let status: String = fx
        .file
        .conn()
        .query_row(
            "SELECT status FROM a11y_rules WHERE url_id=?1 AND rule_id='color-contrast'",
            [url.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "incomplete");
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

// spec: TECH-252
// spec: TECH-253
// spec: TECH-254
// spec: TECH-255
// spec: TECH-256
// spec: TECH-257
// spec: UI-064 [P1]
#[tokio::test]
async fn requires_chromium_group_fixtures_drive_rule_filters_and_counters() {
    let _chromium = CHROMIUM.lock().await;
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "best-practice",
            "Best Practice",
            &[
                "tabindex",
                "region",
                "landmark-one-main",
                "page-has-heading-one",
                "heading-order",
                "empty-heading",
                "image-redundant-alt",
                "accesskeys",
            ],
        ),
        (
            "wcag20a",
            "WCAG 2.0 A",
            &[
                "image-alt",
                "link-name",
                "button-name",
                "label",
                "html-has-lang",
                "listitem",
                "marquee",
                "frame-title",
            ],
        ),
        ("wcag20a-document-title", "WCAG 2.0 A", &["document-title"]),
        ("wcag20a-meta-refresh", "WCAG 2.0 A", &["meta-refresh"]),
        (
            "wcag20aa",
            "WCAG 2.0 AA",
            &["meta-viewport", "valid-lang", "color-contrast"],
        ),
        (
            "wcag20aaa",
            "WCAG 2.0 AAA",
            &["meta-refresh-no-exceptions", "color-contrast-enhanced"],
        ),
        (
            "wcag21aa",
            "WCAG 2.1 AA",
            &["autocomplete-valid", "avoid-inline-spacing"],
        ),
        ("wcag22aa", "WCAG 2.2 AA", &["target-size"]),
    ];
    for (fixture, group, expected) in cases {
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = RenderingMode::JavaScript;
        cfg.rendering.ajax_timeout_s = 0.0;
        cfg.rendering.page_timeout_s = 60;
        cfg.rendering.screenshots.enabled = false;
        cfg.extraction.accessibility = true;
        cfg.limits.max_urls = 1;
        let fx = os_engine::testing::crawl_fixture_from(
            "ui/fx-ui-a11y",
            &format!("/{fixture}.html"),
            cfg.clone(),
        )
        .await;
        let mut q = ViewQuery::new("Accessibility", "All");
        q.columns = ColumnSelection::All;
        let page = query_view(&fx.file, &cfg, &q).unwrap();
        assert_eq!(page.total, 1, "{fixture}");
        let url = page.row_keys[0];
        for rule in *expected {
            let status: String = fx
                .file
                .conn()
                .query_row(
                    "SELECT status FROM a11y_rules WHERE url_id=?1 AND rule_id=?2",
                    rusqlite::params![url.0, rule],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(status, "violation", "{fixture}: {rule}");
            let filtered =
                query_view(&fx.file, &cfg, &ViewQuery::new("Accessibility", *rule)).unwrap();
            assert_eq!(filtered.row_keys, [url], "{fixture}: {rule}");
        }
        if *fixture == "wcag20aaa" {
            // axe cannot decide whether different destinations have the same purpose.
            let status: String = fx.file.conn().query_row("SELECT status FROM a11y_rules WHERE url_id=?1 AND rule_id='identical-links-same-purpose'", [url.0], |r| r.get(0)).unwrap();
            assert_eq!(status, "incomplete");
            assert_eq!(
                query_view(
                    &fx.file,
                    &cfg,
                    &ViewQuery::new("Accessibility", "identical-links-same-purpose")
                )
                .unwrap()
                .total,
                0
            );
        }
        // Count against the independent specification snapshot, not production groups.
        let snapshot = include_str!("../../../tests/fixtures/ui/fx-ui-a11y/spec-rules.tsv");
        let mut total = 0;
        for expected_group in [
            "Best Practice",
            "WCAG 2.0 A",
            "WCAG 2.0 AA",
            "WCAG 2.0 AAA",
            "WCAG 2.1 AA",
            "WCAG 2.2 AA",
        ] {
            let mut count = 0;
            for line in snapshot.lines().filter(|l| !l.starts_with('#')) {
                let fields: Vec<_> = line.split('\t').collect();
                if fields[1] == expected_group {
                    let status: String = fx
                        .file
                        .conn()
                        .query_row(
                            "SELECT status FROM a11y_rules WHERE url_id=?1 AND rule_id=?2",
                            rusqlite::params![url.0, fields[0]],
                            |r| r.get(0),
                        )
                        .unwrap();
                    count += i64::from(status == "violation");
                }
            }
            let column = format!("{expected_group} Violations");
            assert_eq!(
                page.rows[0][page.columns.iter().position(|c| c == &column).unwrap()],
                Value::Int(count),
                "{fixture}: {expected_group}"
            );
            total += count;
        }
        assert_eq!(
            page.rows[0][page
                .columns
                .iter()
                .position(|c| c == "All Violations")
                .unwrap()],
            Value::Int(total)
        );
        let column = format!("{group} Violations");
        assert_eq!(
            page.rows[0][page.columns.iter().position(|c| c == &column).unwrap()],
            Value::Int(expected.len() as i64),
            "{fixture}: group count"
        );
    }
}
