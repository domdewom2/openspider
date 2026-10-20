use os_analyze::{
    tabs,
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{ApiLevel, CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, OpenMode, StorageProfile};
// spec: INTEG-151
#[test]
fn provider_columns_are_adjacent_and_exact_i64_values_survive_reopen_and_sort() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metrics.ospider");
    let mut cfg = CrawlConfig::default();
    cfg.api.ahrefs.enabled = true;
    cfg.api.ahrefs.metrics = vec!["backlinks".into()];
    cfg.api.ahrefs.levels = vec![ApiLevel::Url];
    cfg.api.moz.enabled = true;
    cfg.api.moz.metrics = vec!["domain_authority".into()];
    cfg.api.moz.levels = vec![ApiLevel::Domain];
    let now = Timestamp::from_db("2026-10-19T12:00:00Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(1).new_crawl_id(now),
        mode: CrawlMode::Spider,
        start_url: Some("https://integ.test/".into()),
        list_source: None,
        project: "fixture".into(),
        name: "fixture".into(),
        config: cfg.clone(),
        created_at: now,
    };
    let file = CrawlFile::create(&path, &init, StorageProfile::default()).unwrap();
    file.conn().execute_batch("INSERT INTO urls(id,url,address,origin,is_internal,in_tables,content_class,state,discovered_via,folder_depth,url_length,title_1,status_code) VALUES (1,'https://integ.test/a','https://integ.test/a','https://integ.test',1,1,'HTML','fetched','seed',0,20,'A',200),(2,'https://integ.test/b','https://integ.test/b','https://integ.test',1,1,'HTML','fetched','seed',0,20,'B',200),(3,'https://external.test/','https://external.test/','https://external.test',0,1,'HTML','fetched','seed',0,20,'External',200),(4,'https://integ.test/a.pdf','https://integ.test/a.pdf','https://integ.test',1,1,'PDF','fetched','seed',0,20,'PDF',200);
    INSERT INTO api_values(url_id,source,metric,level,v_text,fetched_at) VALUES(1,'ahrefs','backlinks','url','5000000000','2026-10-19T12:00:00Z'),(2,'ahrefs','backlinks','url','9007199254740993','2026-10-19T12:00:00Z'),(1,'moz','domain_authority','domain','65','2026-10-19T12:00:00Z');").unwrap();
    let mut query = ViewQuery::new("Link Metrics", "All");
    query.sort = vec![os_analyze::defs::SortKey {
        column: "Ahrefs Backlinks (Exact URL)".into(),
        descending: true,
    }];
    let page = views::query_view(&file, &cfg, &query).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(
        page.columns,
        [
            "Address",
            "Status Code",
            "Title 1",
            "Ahrefs Backlinks (Exact URL)",
            "Moz Domain Authority (Domain)"
        ]
    );
    assert_eq!(page.rows[0][3], Value::Int(9007199254740993));
    assert_eq!(page.rows[1][3], Value::Int(5000000000));
    let internal = tabs::dynamic_columns("Internal", &cfg);
    assert!(internal
        .iter()
        .any(|c| c.key == "Ahrefs Backlinks (Exact URL)"));
    let mut internal = ViewQuery::new("Internal", "All");
    internal.columns = ColumnSelection::Named(vec!["Ahrefs Backlinks (Exact URL)".into()]);
    assert!(views::query_view(&file, &cfg, &internal)
        .unwrap()
        .rows
        .iter()
        .any(|row| row.contains(&Value::Int(9007199254740993))));
    drop(file);
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        views::query_view(&file, &cfg, &query).unwrap().rows,
        page.rows
    );
    // INTEG-010 keep: retained provider values remain visible after disabling their source.
    cfg.api.ahrefs.enabled = false;
    cfg.api.moz.enabled = false;
    assert!(views::query_view(&file, &cfg, &query)
        .unwrap()
        .columns
        .contains(&"Ahrefs Backlinks (Exact URL)".into()));
}
