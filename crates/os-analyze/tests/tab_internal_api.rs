use os_analyze::{
    defs::ColumnSql,
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_engine::testing::{crawl_dir_with, test_deps};
use os_model::{CrawlConfig, Value};

// spec: INTEG-013
#[tokio::test]
async fn internal_reuses_integration_values_and_does_not_duplicate_link_metrics() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("index.html"),
        "<title>Fixture</title><p>Hello</p>",
    )
    .unwrap();
    let fx = crawl_dir_with(
        dir.path(),
        "http://integ.test/",
        CrawlConfig::default(),
        test_deps(),
    )
    .await;
    let id: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE url='http://integ.test/'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    for (source, metric, value) in [
        ("ga4", "sessions", 42.0),
        ("gsc", "clicks", 7.0),
        ("psi", "Performance Score", 95.0),
    ] {
        fx.file.conn().execute("INSERT INTO api_values(url_id,source,metric,level,v_real,fetched_at) VALUES(?1,?2,?3,'url',?4,'2026-10-19T12:00:00Z')",(id,source,metric,value)).unwrap();
    }
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.enabled = true;
    cfg.api.gsc.enabled = true;
    cfg.api.psi.enabled = true;
    cfg.api.ahrefs.enabled = true;
    cfg.api.ahrefs.levels = vec![os_model::ApiLevel::Url];
    cfg.api.ahrefs.metrics = vec!["backlinks".into()];
    let all = tabs::configured_for_file(&cfg, &fx.file);
    let internal = all.iter().find(|t| t.id == "Internal").unwrap();
    let keys: Vec<_> = internal.columns.iter().map(|c| c.key).collect();
    assert_eq!(keys.iter().filter(|k| k.starts_with("Ahrefs")).count(), 1);
    for name in ["Analytics", "Search Console", "PageSpeed", "Link Metrics"] {
        let tab = all.iter().find(|t| t.id == name).unwrap();
        for col in tab.columns.iter().filter(|c| c.needs.is_some()) {
            let other = internal
                .columns
                .iter()
                .find(|c| c.key == col.key)
                .unwrap_or_else(|| panic!("Internal is missing {name}:{}", col.key));
            if let (ColumnSql::Expr(a), ColumnSql::Expr(b)) = (&col.sql, &other.sql) {
                assert_eq!(a, b);
            }
        }
    }
    let mut query = ViewQuery::new("Internal", "All");
    query.columns = ColumnSelection::Named(vec![
        "Address".into(),
        "GA4 Sessions".into(),
        "Clicks".into(),
    ]);
    let page = views::query_view_in(&all, &fx.file, &cfg, &query).unwrap();
    assert!(
        page.rows
            .iter()
            .any(|row| row.contains(&Value::Int(42)) && row.contains(&Value::Int(7))),
        "{:?}",
        page.rows
    );
}

// spec: INTEG-013
#[test]
fn unconfigured_integrations_do_not_add_empty_columns() {
    let cfg = CrawlConfig::default();
    assert!(tabs::dynamic_api::column_source()
        .unwrap()
        .columns(&cfg)
        .is_empty());
}

// spec: INTEG-012
// spec: CRAWL-037
#[tokio::test]
async fn api_input_expands_files_and_nested_sitemaps_without_fetching_target_pages() {
    use os_engine::testing::{SimFetcher, SimSite};
    let fetcher = SimFetcher::new(SimSite::new()
        .asset("http://integ.test/index.xml", "application/xml", "<sitemapindex><sitemap><loc>http://integ.test/child.xml</loc></sitemap></sitemapindex>")
        .asset("http://integ.test/child.xml", "application/xml", "<urlset><url><loc>http://integ.test/a</loc></url><url><loc>http://integ.test/b</loc></url></urlset>")
        .page("http://integ.test/a","<title>A</title>"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("urls.txt");
    std::fs::write(&path, "http://integ.test/a\nhttp://integ.test/a\n").unwrap();
    let items = os_engine::list_input::api_items(
        &[
            path.to_string_lossy().into_owned(),
            "http://integ.test/index.xml".into(),
        ],
        &CrawlConfig::default(),
        fetcher.as_ref(),
    )
    .await
    .unwrap();
    let plan = os_engine::list_input::plan(&items, &CrawlConfig::default()).unwrap();
    assert_eq!(plan.counts.to_crawl, 2);
    assert!(fetcher
        .requests()
        .iter()
        .all(|r| r.purpose == os_model::FetchPurpose::Sitemap));
    assert_eq!(fetcher.count("http://integ.test/index.xml"), 1);
    assert_eq!(fetcher.count("http://integ.test/child.xml"), 1);
}

// spec: INTEG-015
// spec: CRAWL-089
#[tokio::test]
async fn opted_in_orphan_filters_require_analysis_and_respect_indexability() {
    use os_model::FilterState;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<title>Start</title>").unwrap();
    let fx = crawl_dir_with(
        dir.path(),
        "http://integ.test/",
        CrawlConfig::default(),
        test_deps(),
    )
    .await;
    fx.file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,indexability) VALUES(100,'http://integ.test/orphan','http://integ.test/orphan','http://integ.test',1,'done','link',1,24,'HTML',200,'Non-Indexable')",[]).unwrap();
    for source in ["ga4", "gsc"] {
        fx.file.conn().execute("INSERT INTO api_values(url_id,source,metric,level,v_real,fetched_at) VALUES(100,?1,?2,'url',1,'2026-10-19T12:00:00Z')",(source,if source=="ga4" {"sessions"} else {"impressions"})).unwrap();
    }
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.enabled = true;
    cfg.api.ga4.crawl_new_urls = true;
    cfg.api.gsc.enabled = true;
    cfg.api.gsc.crawl_new_urls = true;
    for tab in ["Analytics", "Search Console"] {
        let counts = views::filter_counts(&fx.file, &cfg).unwrap();
        assert_eq!(
            counts
                .iter()
                .find(|c| c.tab == tab && c.filter == "Orphan URLs")
                .unwrap()
                .state,
            FilterState::NeedsCrawlAnalysis
        );
    }
    os_analyze::analysis::run(&fx.file, &cfg, &Default::default(), &|_, _| {}).unwrap();
    for (ignore, want) in [(true, 0), (false, 1)] {
        cfg.issues.ignore_non_indexable = ignore;
        let counts = views::filter_counts(&fx.file, &cfg).unwrap();
        for tab in ["Analytics", "Search Console"] {
            let c = counts
                .iter()
                .find(|c| c.tab == tab && c.filter == "Orphan URLs")
                .unwrap();
            assert_eq!(c.state, FilterState::Ready);
            assert_eq!(c.count, Some(want));
        }
    }
}
