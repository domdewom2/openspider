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
