use os_analyze::{
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{CrawlConfig, JsSnippet, JsSnippetKind, RenderingMode, Value};
use os_store::{CrawlFile, OpenMode};
#[path = "../../../tests/fixtures/integ-custom-js/store.rs"]
mod fixture;

// spec: INTEG-060
// spec: INTEG-061
// spec: INTEG-071
#[test]
fn dynamic_columns_include_empty_and_failed_executions_and_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("js.ospider");
    let file = fixture::create(&path);
    let cfg = file.config().unwrap();
    let all = views::query_view(&file, &cfg, &ViewQuery::new("Custom JavaScript", "All")).unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(
        all.columns,
        [
            "Address",
            "Content Type",
            "Status Code",
            "Status",
            "Snippet 2 1",
            "Snippet 2 2",
            "Snippet 2 3",
            "Snippet 2 Error",
            "empty 1",
            "empty Error"
        ]
    );
    let tab = tabs::configured(&cfg)
        .into_iter()
        .find(|t| t.id == "Custom JavaScript")
        .unwrap();
    assert_eq!(
        tab.filters.iter().map(|f| f.id).collect::<Vec<_>>(),
        ["All", "Snippet 2", "empty"]
    );
    let mut q = ViewQuery::new("Custom JavaScript", "Snippet 2");
    q.limit = 1;
    q.start = views::PageStart::Offset(1);
    let page = views::query_view(&file, &cfg, &q).unwrap();
    assert_eq!(page.total, 2); // The timed-out execution is still present.
    assert_eq!(
        page.columns,
        [
            "Address",
            "Content Type",
            "Status Code",
            "Status",
            "Snippet 2 1",
            "Snippet 2 2",
            "Snippet 2 3",
            "Snippet 2 Error"
        ]
    );
    assert_eq!(
        &page.rows[0][4..7],
        &[
            Value::Text("".into()),
            Value::Text("".into()),
            Value::Text("".into())
        ]
    );
    assert_eq!(page.rows[0][7], Value::Text("Timeout".into()));
    let empty =
        views::query_view(&file, &cfg, &ViewQuery::new("Custom JavaScript", "empty")).unwrap();
    assert_eq!(empty.total, 1);
    let counts = views::filter_counts_in(&tabs::configured(&cfg), &file, &cfg).unwrap();
    assert!(counts
        .iter()
        .any(|c| c.tab == "Custom JavaScript" && c.filter == "Snippet 2" && c.count == Some(2)));
    let mut internal = ViewQuery::new("Internal", "All");
    internal.columns = ColumnSelection::Named(vec![
        "Address".into(),
        "Snippet 2".into(),
        "Snippet 2 Error".into(),
    ]);
    assert_eq!(
        views::query_view(&file, &cfg, &internal)
            .unwrap()
            .columns
            .len(),
        5
    );
    drop(file);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        views::query_view(&file, &file.config().unwrap(), &q)
            .unwrap()
            .rows,
        page.rows
    );
}

// spec: INTEG-060
// spec: INTEG-062
// spec: INTEG-071
#[tokio::test]
async fn requires_chromium_crawl_persists_values_trace_and_discovers_action_links() {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.rendering.ajax_timeout_s = 0.0;
    cfg.rendering.screenshots.enabled = false;
    cfg.custom_js.snippets = vec![
        JsSnippet {
            name: "heads".into(),
            code: "return seoSpider.data(document.querySelectorAll('h2').length);".into(),
            ..Default::default()
        },
        JsSnippet {
            kind: JsSnippetKind::Action,
            timeout_s: 1,
            code: "window.dispatchEvent(new Event('scroll'));".into(),
            ..Default::default()
        },
    ];
    let fx = os_engine::testing::crawl_fixture_from("integ-custom-js", "/page.html", cfg).await;
    let file = &fx.file;
    let cfg = file.config().unwrap();
    let page =
        views::query_view(file, &cfg, &ViewQuery::new("Custom JavaScript", "heads")).unwrap();
    assert!(page.total >= 4);
    assert!(page.rows.iter().any(|r| r[4] == Value::Text("3".into())));
    for path in ["/p2", "/p3"] {
        let count: i64 = file
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM urls WHERE address LIKE '%' || ?1 AND state = 'done'",
                [path],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
    let executed: i64 = file
        .conn()
        .query_row(
            "SELECT MIN(v_int) FROM url_ext WHERE key='custom_js.executed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(executed, 2);
    let log: String = file
        .conn()
        .query_row(
            "SELECT v_text FROM url_ext WHERE key='custom_js.log' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let log: serde_json::Value = serde_json::from_str(&log).unwrap();
    assert_eq!(log[0][0], 1); // Action before extraction, although declared second.
    assert_eq!(log[1][0], 0);
    assert!(log[0][3].as_u64().unwrap() < log[1][2].as_u64().unwrap());
}

// spec: INTEG-060
#[tokio::test(start_paused = true)]
async fn text_mode_executes_no_snippets_and_persists_zero_counter() {
    let mut cfg = CrawlConfig::default();
    cfg.custom_js.snippets = vec![JsSnippet {
        code: "while(true){}".into(),
        ..Default::default()
    }];
    let fx = os_engine::testing::crawl_fixture_from("integ-custom-js", "/page.html", cfg).await;
    let values: i64 = fx
        .file
        .conn()
        .query_row("SELECT COUNT(*) FROM custom_js_values", [], |r| r.get(0))
        .unwrap();
    assert_eq!(values, 0);
    let executed: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT SUM(v_int) FROM url_ext WHERE key='custom_js.executed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(executed, 0);
}

// spec: INTEG-060
#[tokio::test]
async fn requires_chromium_scope_and_content_type_filters_gate_actual_execution() {
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.rendering.ajax_timeout_s = 0.0;
    cfg.rendering.screenshots.enabled = false;
    cfg.spider.external.crawl = true;
    cfg.custom_js.snippets = vec![JsSnippet {
        name: "path".into(),
        code: "return openspider.data(location.pathname);".into(),
        ..Default::default()
    }];
    let fx =
        os_engine::testing::crawl_fixture_from("integ-custom-js", "/scope.html", cfg.clone()).await;
    let mut stmt = fx.file.conn().prepare("SELECT j.value FROM custom_js_values j JOIN urls u ON u.id=j.url_id WHERE u.is_internal=1 ORDER BY j.value").unwrap();
    let paths = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(paths, ["/charset", "/p1", "/page.html", "/scope.html"]);
    let outside: i64 = fx.file.conn().query_row("SELECT COUNT(*) FROM custom_js_values j JOIN urls u ON u.id=j.url_id WHERE u.status_code<>200 OR u.is_internal=0 OR u.content_class='PDF'",[],|r|r.get(0)).unwrap();
    assert_eq!(outside, 0);
    cfg.custom_js.snippets[0].content_types = vec!["*".into()];
    let fx = os_engine::testing::crawl_fixture_from("integ-custom-js", "/scope.html", cfg).await;
    for path in ["/image.png", "/plain.txt"] {
        let count: i64 = fx.file.conn().query_row("SELECT COUNT(*) FROM custom_js_values j JOIN urls u ON u.id=j.url_id WHERE u.address LIKE '%' || ?1",[path],|r|r.get(0)).unwrap();
        assert_eq!(count, 1, "{path}");
    }
    let outside: i64 = fx.file.conn().query_row("SELECT COUNT(*) FROM custom_js_values j JOIN urls u ON u.id=j.url_id WHERE u.status_code<>200 OR u.is_internal=0 OR u.content_class='PDF'",[],|r|r.get(0)).unwrap();
    assert_eq!(outside, 0);
}

// spec: INTEG-062
#[tokio::test(start_paused = true)]
async fn renderer_overrides_receive_the_same_additional_action_budget() {
    use os_engine::{
        render_glue::{RenderError, RenderOpts, RenderedPage, Renderer},
        start_crawl,
        testing::{test_deps, SimFetcher, SimSite},
        CrawlRequest, CrawlSource,
    };
    use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
    struct ActionRenderer;
    impl Renderer for ActionRenderer {
        fn render<'a>(
            &'a self,
            url: &'a str,
            opts: &'a RenderOpts,
        ) -> Pin<Box<dyn Future<Output = Result<RenderedPage, RenderError>> + Send + 'a>> {
            Box::pin(async move {
                let mut snippets = Vec::new();
                if !opts.snippets.snippets.is_empty() {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    snippets = serde_json::from_value(serde_json::json!([{
                        "def_index":0, "action":true, "values":[], "error":null,
                        "started_us":0, "finished_us":3_000_000
                    }]))
                    .unwrap();
                }
                Ok(RenderedPage {
                    snippets,
                    dom_html: "<h1>Fixture</h1>".into(),
                    final_url: url.into(),
                    ..Default::default()
                })
            })
        }
    }
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.rendering.page_timeout_s = 1;
    cfg.custom_js.snippets = vec![JsSnippet {
        kind: JsSnippetKind::Action,
        timeout_s: 3,
        code: "window.scrollTo(0,100)".into(),
        ..Default::default()
    }];
    let fetcher = SimFetcher::new(SimSite::new().page(
        "https://budget.test/",
        include_str!("../../../tests/fixtures/integ-custom-js/plain.txt"),
    ));
    let mut deps = test_deps();
    deps.renderer = Some(Arc::new(ActionRenderer));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("budget.ospider");
    start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: "https://budget.test/".into(),
            },
            file: path.clone(),
            project: "D22".into(),
            name: "budget".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let executed: i64 = file
        .conn()
        .query_row(
            "SELECT v_int FROM url_ext WHERE key='custom_js.executed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(executed, 1); // A one-second parent deadline must allow this three-second action.
}

// spec: INTEG-060
// spec: INTEG-061
// spec: INTEG-071
#[test]
fn colliding_names_keep_all_filter_and_both_tabs_value_and_error_columns_distinct() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("collisions.ospider");
    let file = fixture::create(&path);
    let mut cfg = file.config().unwrap();
    cfg.custom_js = serde_json::from_str(include_str!(
        "../../../tests/fixtures/integ-custom-js/collisions.json"
    ))
    .unwrap();
    cfg.custom_extraction.extractors = vec![os_model::Extractor {
        name: "shared".into(),
        ..Default::default()
    }];
    file.set_config_with_clock(
        &cfg,
        &os_model::ManualClock::new(os_model::Timestamp::UNIX_EPOCH),
    )
    .unwrap();
    file.conn()
        .execute("DELETE FROM custom_js_values", [])
        .unwrap();
    for i in 0..8 {
        file.conn().execute(
            "INSERT INTO custom_js_values(url_id,def_index,number,value,error) VALUES (1,?1,0,?2,?3)",
            (i, format!("value-{i}"), format!("error-{i}")),
        ).unwrap();
    }
    let names = [
        "All (JavaScript 1)",
        "Address (JavaScript 1)",
        "x",
        "x Error (JavaScript 1)",
        "x 1 (JavaScript 1)",
        "Title 1 (JavaScript 1)",
        "shared (JavaScript 1)",
        "All (JavaScript 1) (JavaScript 1)",
    ];
    let configured = tabs::configured(&cfg);
    let tab = configured
        .iter()
        .find(|t| t.id == "Custom JavaScript")
        .unwrap();
    assert_eq!(
        tab.filters.iter().map(|f| f.id).collect::<Vec<_>>(),
        std::iter::once("All")
            .chain(names.iter().copied())
            .collect::<Vec<_>>()
    );
    let all = views::query_view(&file, &cfg, &ViewQuery::new("Custom JavaScript", "All")).unwrap();
    assert_eq!(all.total, 3); // Only one URL executed snippets; All must retain the whole population.
    assert_eq!(all.columns.len(), 20); // Four base columns + eight distinct pairs, no action.
    assert_eq!(
        all.columns
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        20
    );
    for (i, name) in names.iter().enumerate() {
        let filtered =
            views::query_view(&file, &cfg, &ViewQuery::new("Custom JavaScript", *name)).unwrap();
        assert_eq!(filtered.total, 1);
        assert_eq!(
            &filtered.columns[4..],
            [format!("{name} 1"), format!("{name} Error")]
        );
        assert_eq!(
            &filtered.rows[0][4..],
            [
                Value::Text(format!("value-{i}")),
                Value::Text(format!("error-{i}"))
            ]
        );
        for tab in ["Custom JavaScript", "Internal"] {
            // Test both the multi-column base key and its numbered projection.
            for value_key in [name.to_string(), format!("{name} 1")] {
                let mut query = ViewQuery::new(tab, "All");
                query.columns = ColumnSelection::Named(vec![
                    "Address".into(),
                    value_key,
                    format!("{name} Error"),
                ]);
                let page = views::query_view(&file, &cfg, &query).unwrap();
                let row = page
                    .rows
                    .iter()
                    .find(|r| r[0] == Value::Text("https://extract.test/a".into()))
                    .unwrap();
                assert_eq!(
                    &row[1..],
                    [
                        Value::Text(format!("value-{i}")),
                        Value::Text(format!("error-{i}"))
                    ],
                    "{tab}: {name}"
                );
            }
        }
    }
    drop(file);
    let reopened = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let cfg = reopened.config().unwrap();
    assert_eq!(cfg.custom_js.snippets[0].name, "All"); // No silent config rewrite.
    let restored =
        views::query_view(&reopened, &cfg, &ViewQuery::new("Custom JavaScript", "All")).unwrap();
    assert_eq!(restored.columns, all.columns);
    assert_eq!(restored.rows, all.rows);
}
