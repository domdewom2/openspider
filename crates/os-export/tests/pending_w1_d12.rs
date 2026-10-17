#[path = "reports_common.rs"]
mod common;

// spec: ONPAGE-064
// spec: CRAWL-018 [M2]
#[tokio::test]
#[ignore = "wartet auf M2-E03"]
async fn learned_hsts_policy_is_exported_without_an_http_target_request() {
    let cfg = os_model::CrawlConfig::default();
    let fx = os_engine::testing::crawl_fixture_from(
        "onpage-m2-rc",
        "https://site.test/hsts",
        cfg.clone(),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let result = os_export::save_report(
        &fx.file,
        &cfg,
        "Redirects:Redirects to Errors",
        dir.path(),
        os_export::Format::Csv,
    )
    .unwrap();
    let t = common::Table::read(&result.files[0].path);
    let row = t.row("Address", "/hsts-target");
    assert_eq!(t.get(row, "Chain Type"), "HSTS Policy");
    assert_eq!(t.get(row, "Status Code 1"), "307");
    assert_eq!(t.get(row, "Status 1"), "HSTS Policy");
    assert_eq!(t.get(row, "Final Status Code"), "404");
    assert!(!fx
        .site
        .requests()
        .iter()
        .any(|r| r.path_and_query == "/hsts-target" && !r.tls));
}

// spec: OUT-085
#[tokio::test]
#[ignore = "wartet auf M2-E06"]
async fn always_follow_list_redirects_exports_the_complete_chain_at_depth_zero() {
    let site = os_testsite::TestSite::start("onpage-m2-rc").await.unwrap();
    let mut cfg = os_model::CrawlConfig {
        mode: os_model::CrawlMode::List,
        ..Default::default()
    };
    cfg.advanced.always_follow_redirects = true;
    cfg.limits.max_depth = Some(0);
    let deps = os_engine::testing::test_deps();
    let fetcher = os_engine::testing::site_fetcher(&site, &cfg, &deps);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("list.ospider");
    let handle = os_engine::start_crawl(
        fetcher,
        os_engine::CrawlRequest {
            config: cfg.clone(),
            source: os_engine::CrawlSource::List {
                items: vec!["http://site.test/refresh".into()],
                label: "D12 fixture".into(),
            },
            file: path.clone(),
            project: "test".into(),
            name: "D12".into(),
        },
        deps,
    )
    .await
    .unwrap();
    handle.wait().await.unwrap();
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    let result = os_export::save_report(
        &file,
        &cfg,
        "Redirects:Redirect Chains",
        dir.path(),
        os_export::Format::Csv,
    )
    .unwrap();
    let t = common::Table::read(&result.files[0].path);
    let row = t.row("Address", "/refresh");
    assert_eq!(t.get(row, "Number of Redirects"), "2");
    assert_eq!(t.get(row, "Final Status Code"), "404");
}
