//! Transport-Abnahme nach M2-E03; die Anzeige gespeicherter HSTS-Fakten läuft bereits ohne Ignore.
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, Value};

// spec: ONPAGE-064
// spec: CRAWL-018 [M2]
#[tokio::test]
async fn hsts_upgrade_is_synthetic_and_respects_the_switch() {
    for enabled in [true, false] {
        let mut cfg = CrawlConfig::default();
        cfg.http.respect_hsts = enabled;
        cfg.speed.threads = 1;
        let fx = crawl_fixture_from("onpage-m2-rc", "https://site.test/hsts", cfg.clone()).await;
        let requests: Vec<_> = fx
            .site
            .requests()
            .into_iter()
            .filter(|r| r.path_and_query == "/hsts-target")
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].tls, enabled);
        let mut q = ViewQuery::new("Response Codes", "Redirection (3XX)");
        q.columns = ColumnSelection::Named(vec![
            "Address".into(),
            "Status Code".into(),
            "Status".into(),
            "Redirect Type".into(),
            "Redirect URL".into(),
        ]);
        let rows = query_view(&fx.file, &cfg, &q).unwrap().rows;
        if enabled {
            assert!(rows.contains(&vec![
                Value::Text("http://site.test/hsts-target".into()),
                Value::Int(307),
                Value::Text("HSTS Policy".into()),
                Value::Text("HSTS Policy".into()),
                Value::Text("https://site.test/hsts-target".into())
            ]));
        } else {
            assert!(rows.is_empty());
        }
    }
    // Ohne Policy erfolgt auch bei eingeschalteter Option ein echter HTTP-Abruf.
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("onpage-m2-rc", "https://site.test/hsts-no-policy", cfg).await;
    assert!(fx
        .site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/hsts-target" && !r.tls));
}
