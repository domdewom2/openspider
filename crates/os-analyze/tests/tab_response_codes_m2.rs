use os_analyze::{
    details::{detail_source, DetailKind, DetailReq},
    views::{query_view, ViewQuery},
};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, UrlId, Value};

// spec: ONPAGE-059
#[tokio::test]
async fn refresh_header_filter_and_detail_chain_use_stored_facts() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    let page = query_view(
        &fx.file,
        &cfg,
        &ViewQuery::new("Response Codes", "Redirection (HTTP Refresh)"),
    )
    .unwrap();
    assert_eq!(page.total, 1);
    let id: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address = 'http://site.test/refresh'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let params = serde_json::json!({});
    let ids = [UrlId(id)];
    let req = DetailReq {
        url_ids: &ids,
        params: &params,
        sort: &[],
        offset: 1,
        limit: 1,
    };
    let (_, source) = detail_source(DetailKind::RedirectChain);
    let data = source(&fx.file, &cfg, &req).unwrap();
    assert_eq!(data.total, 3);
    assert_eq!(data.rows.len(), 1);
    assert_eq!(
        data.rows[0][2],
        Value::Text("http://site.test/redirect".into())
    );
}

// spec: ONPAGE-064
// spec: CRAWL-018 [M2]
#[tokio::test]
async fn hsts_policy_facts_are_displayed_as_redirection() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/", cfg.clone()).await;
    fx.file.conn().execute("UPDATE urls SET status_code = 307, status = 'HSTS Policy', redirect_type = 'hsts_policy' WHERE address = 'http://site.test/ok'", []).unwrap();
    let page = query_view(
        &fx.file,
        &cfg,
        &ViewQuery::new("Response Codes", "Redirection (3XX)"),
    )
    .unwrap();
    assert!(page
        .rows
        .iter()
        .any(|r| r.contains(&Value::Text("HSTS Policy".into()))));
}

// spec: ONPAGE-059
#[tokio::test]
async fn refresh_filter_includes_internal_and_external_non_html_headers() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from(
        "onpage-m2-rc",
        "http://site.test/external-root",
        cfg.clone(),
    )
    .await;
    for (filter, count) in [
        ("Redirection (HTTP Refresh)", 2),
        ("Internal Redirection (HTTP Refresh)", 1),
        ("External Redirection (HTTP Refresh)", 1),
    ] {
        let page = query_view(&fx.file, &cfg, &ViewQuery::new("Response Codes", filter)).unwrap();
        assert_eq!(page.total, count, "{filter}");
    }
}

fn addresses(file: &os_store::CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new("Response Codes", filter);
    q.columns = os_analyze::views::ColumnSelection::Named(vec!["Address".into()]);
    q.sort = vec![os_analyze::SortKey {
        column: "Address".into(),
        descending: false,
    }];
    query_view(file, cfg, &q)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| match &r[0] {
            Value::Text(address) => address.clone(),
            other => panic!("Address: {other:?}"),
        })
        .collect()
}

// spec: ONPAGE-059
#[tokio::test]
async fn refresh_filter_keeps_header_facts_beside_http_and_meta_redirects() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from(
        "onpage-m2-rc",
        "http://site.test/refresh-cases",
        cfg.clone(),
    )
    .await;
    assert_eq!(
        addresses(&fx.file, &cfg, "Redirection (HTTP Refresh)"),
        [
            "http://site.test/refresh-delay",
            "http://site.test/refresh-meta",
            "http://site.test/refresh-priority",
        ]
    );
    assert_eq!(
        addresses(&fx.file, &cfg, "Internal Redirection (HTTP Refresh)"),
        addresses(&fx.file, &cfg, "Redirection (HTTP Refresh)")
    );
    assert!(addresses(&fx.file, &cfg, "External Redirection (HTTP Refresh)").is_empty());
    // Auch tab() selbst liefert echte Prädikate, unabhängig von der zentralen Stub-Ergänzung.
    let direct = os_analyze::tabs::response_codes::tab();
    let f = direct
        .filters
        .iter()
        .find(|f| f.id == "Internal Redirection (HTTP Refresh)")
        .unwrap();
    let count: i64 = fx
        .file
        .conn()
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM urls u WHERE {} AND ({})",
                direct.population, f.predicate
            ),
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 3);
    for path in ["/redirect", "/error"] {
        assert!(
            fx.site.requests().iter().any(|r| r.path_and_query == path),
            "{path} fetched"
        );
    }
}

// spec: ONPAGE-060
// spec: ONPAGE-061
#[tokio::test]
async fn mixed_refresh_cycles_count_each_hop_once_and_keep_external_views_empty() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/mixed-loops", cfg.clone()).await;
    let expected = [
        "http://site.test/mixed-header",
        "http://site.test/mixed-meta",
        "http://site.test/mixed-tail",
    ];
    assert_eq!(addresses(&fx.file, &cfg, "Redirect Chain"), expected);
    let mut loops = expected.to_vec();
    loops.push("http://site.test/refresh-self");
    assert_eq!(addresses(&fx.file, &cfg, "Redirect Loop"), loops);
    for filter in ["External Redirect Chain", "External Redirect Loop"] {
        assert!(addresses(&fx.file, &cfg, filter).is_empty());
    }
    let mut q = ViewQuery::new("Response Codes", "Internal Redirect Loop");
    q.columns = os_analyze::views::ColumnSelection::Named(vec![
        "Address".into(),
        "Number of Redirects".into(),
    ]);
    for row in query_view(&fx.file, &cfg, &q).unwrap().rows {
        let Value::Text(address) = &row[0] else {
            panic!("address")
        };
        let hops = match address.as_str() {
            "http://site.test/mixed-tail" => 3,
            "http://site.test/refresh-self" => 1,
            _ => 2,
        };
        assert_eq!(row[1], Value::Int(hops), "{address}");
    }
    assert!(fx.site.duplicate_requests().is_empty());
}

// spec: ONPAGE-054 [M2]
#[tokio::test]
async fn no_response_keeps_transport_failure_separate_from_robots_blocking() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-rc", "http://site.test/errors-root", cfg.clone()).await;
    assert_eq!(
        addresses(&fx.file, &cfg, "No Response"),
        ["http://site.test/drop"]
    );
    assert_eq!(
        addresses(&fx.file, &cfg, "Blocked by Robots.txt"),
        ["http://site.test/blocked/a"]
    );
    assert!(!fx
        .site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/blocked/a"));
}
