use os_analyze::{
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_engine::testing::crawl_fixture;
use os_model::{CrawlConfig, CustomSearchConfig, Value};

fn config() -> CrawlConfig {
    CrawlConfig {
        custom_search: serde_json::from_str::<CustomSearchConfig>(include_str!(
            "../../../tests/fixtures/integ-custom-search/filters.json"
        ))
        .unwrap(),
        ..Default::default()
    }
}

// spec: INTEG-031
// spec: INTEG-020
#[tokio::test]
async fn columns_filters_and_live_counts_use_the_same_stored_hits() {
    let cfg = config();
    let fx = crawl_fixture("integ-custom-search", cfg.clone()).await;
    let mut query = ViewQuery::new("Custom Search", "All");
    query.columns = ColumnSelection::All;
    let tabs = tabs::configured(&cfg);
    let page = views::query_view_in(&tabs, &fx.file, &cfg, &query).unwrap();
    assert_eq!(
        page.columns,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Stock",
            "Missing stock",
            "Redirect",
            "Assumed"
        ]
    );
    assert_eq!(page.total, 4); // root, 404, empty, 301 with body
    let stock = views::query_view_in(
        &tabs,
        &fx.file,
        &cfg,
        &ViewQuery::new("Custom Search", "Stock"),
    )
    .unwrap();
    assert_eq!(stock.total, 2);
    assert_eq!(
        stock.rows.iter().map(|r| r[4].clone()).collect::<Vec<_>>(),
        [Value::Int(3), Value::Int(1)]
    );
    let negative = views::query_view_in(
        &tabs,
        &fx.file,
        &cfg,
        &ViewQuery::new("Custom Search", "Missing stock"),
    )
    .unwrap();
    assert_eq!(negative.total, 2);
    assert!(negative
        .rows
        .iter()
        .all(|r| r[5] == Value::Text("Does Not Contain".into())));
    assert!(stock
        .rows
        .iter()
        .all(|r| r[5] == Value::Text("Contains".into())));
    let counts = views::filter_counts_in(&tabs, &fx.file, &cfg).unwrap();
    for (filter, count) in [
        ("All", 4),
        ("Stock", 2),
        ("Missing stock", 2),
        ("Redirect", 1),
    ] {
        assert!(counts
            .iter()
            .any(|c| c.tab == "Custom Search" && c.filter == filter && c.count == Some(count)));
    }
    // A later writer batch must be reflected in both the live view and counters.
    fx.file
        .conn()
        .execute(
            "UPDATE custom_search_hits SET count=0 WHERE def_index=0",
            [],
        )
        .unwrap();
    assert_eq!(
        views::query_view_in(
            &tabs,
            &fx.file,
            &cfg,
            &ViewQuery::new("Custom Search", "Stock")
        )
        .unwrap()
        .total,
        0
    );
    assert!(views::filter_counts_in(&tabs, &fx.file, &cfg)
        .unwrap()
        .iter()
        .any(|c| c.tab == "Custom Search" && c.filter == "Stock" && c.count == Some(0)));
}

// spec: INTEG-031
// spec: INTEG-028
#[tokio::test]
async fn errors_are_displayed_and_do_not_satisfy_negative_filters() {
    let cfg = config();
    let fx = crawl_fixture("integ-custom-search", cfg.clone()).await;
    fx.file
        .conn()
        .execute(
            "UPDATE custom_search_hits SET error='Regex timeout' WHERE def_index IN (0,1)",
            [],
        )
        .unwrap();
    let tabs = tabs::configured(&cfg);
    let page = views::query_view_in(
        &tabs,
        &fx.file,
        &cfg,
        &ViewQuery::new("Custom Search", "All"),
    )
    .unwrap();
    assert!(page
        .rows
        .iter()
        .all(|r| r[4] == Value::Text("Regex timeout".into())
            && r[5] == Value::Text("Regex timeout".into())));
    for filter in ["Stock", "Missing stock"] {
        assert_eq!(
            views::query_view_in(
                &tabs,
                &fx.file,
                &cfg,
                &ViewQuery::new("Custom Search", filter)
            )
            .unwrap()
            .total,
            0
        );
    }
}

// spec: INTEG-035
#[tokio::test]
async fn search_scope_includes_error_and_redirect_bodies_but_excludes_resources_and_external_urls()
{
    let cfg = config();
    let fx = crawl_fixture("integ-custom-search", cfg.clone()).await;
    let tabs = tabs::configured(&cfg);
    let page = views::query_view_in(
        &tabs,
        &fx.file,
        &cfg,
        &ViewQuery::new("Custom Search", "All"),
    )
    .unwrap();
    assert!(page.rows.iter().any(|r| r[2] == Value::Int(404)));
    assert!(page.rows.iter().any(|r| r[2] == Value::Int(301)));
    for path in [
        "/external",
        "/script.js",
        "/image.png",
        "/bodyless",
        "/assumed",
    ] {
        assert!(
            !page
                .rows
                .iter()
                .any(|r| matches!(&r[0], Value::Text(s) if s.ends_with(path))),
            "{path}"
        );
    }
    let mut cfg = config();
    cfg.advanced.assume_html = true;
    let fx = crawl_fixture("integ-custom-search", cfg.clone()).await;
    assert_eq!(
        views::query_view_in(
            &tabs::configured(&cfg),
            &fx.file,
            &cfg,
            &ViewQuery::new("Custom Search", "Assumed")
        )
        .unwrap()
        .total,
        1
    );
}
