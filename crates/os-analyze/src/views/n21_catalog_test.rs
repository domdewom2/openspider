use super::*;
use os_model::CrawlConfig;

// spec: UI-147 [P1]
#[test]
fn grouped_catalog_does_not_scan_urls_without_query_planner_statistics() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE urls(id INTEGER PRIMARY KEY,is_internal INTEGER,in_tables INTEGER,content_class TEXT);
        CREATE TABLE url_counters(url_id INTEGER PRIMARY KEY);
        CREATE TABLE directives(url_id INTEGER,source TEXT,applies INTEGER);
        WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<20000)
        INSERT INTO urls SELECT x, x!=2, 1, 'HTML' FROM n;
        INSERT INTO directives VALUES(1,'meta_robots',1),(1,'meta_robots',1),
        (1,'meta_robots',0),(2,'meta_robots',1),(2,'meta_robots',1),(2,'meta_robots',1);").unwrap();
    let cfg = CrawlConfig::default();
    let tabs = crate::tabs::configured(&cfg);
    let mut q = super::super::ViewQuery::new("Internal", "All");
    q.columns = super::super::ColumnSelection::Named(vec!["Meta Robots".into()]);
    let built = super::super::build::build(&tabs, &q, &cfg).unwrap();
    let (_, parts) = series_parts(built.columns[0].def).unwrap();
    MULTI_FULLSCAN_STEPS.with(|steps| steps.set(0));
    assert_eq!(multi_count(&conn, &built, &parts, false).unwrap(), 2);
    let steps = MULTI_FULLSCAN_STEPS.with(std::cell::Cell::get);
    assert!(
        steps < 100,
        "catalogue scanned {steps} rows for six child facts and 20,000 URLs"
    );
}
