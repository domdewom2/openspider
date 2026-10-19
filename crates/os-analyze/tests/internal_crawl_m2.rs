#[path = "framework_common.rs"]
mod common;
use os_analyze::defs::ColumnSql;
use os_model::CrawlConfig;

// spec: CRAWL-062
#[test]
fn observed_response_headers_are_case_insensitive_ordered_and_sql_quoted() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "headers", 1);
    common::insert(&file, &[common::row(1, "headers")]);
    file.conn().execute_batch("INSERT INTO http_headers VALUES (1,'response',0,'X-Fixture','first'),(1,'response',1,'x-fixture','second'),(1,'request',0,'X-Request','request'),(1,'response',2,'x-odd''name','quoted');").unwrap();
    let source = os_analyze::tabs::internal_crawl::column_source().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.http_headers = true;
    let columns = source.columns_for_file(&cfg, &file);
    assert_eq!(
        columns
            .iter()
            .filter(|c| c.key == "Header: x-fixture")
            .count(),
        1
    );
    assert!(!columns.iter().any(|c| c.key == "Header: x-request"));
    for (key, expected) in [
        ("Header: x-fixture", "first\nsecond"),
        ("Header: x-odd'name", "quoted"),
    ] {
        let column = columns.iter().find(|c| c.key == key).unwrap();
        let ColumnSql::Expr(sql) = column.sql else {
            panic!("scalar header column");
        };
        let value: String = file
            .conn()
            .query_row(
                &format!("SELECT {sql} FROM urls u WHERE u.id = 1"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, expected);
    }
    cfg.extraction.http_headers = false;
    assert!(!source
        .columns_for_file(&cfg, &file)
        .iter()
        .any(|c| c.key.starts_with("Header:")));
}

// spec: CRAWL-074
// spec: CRAWL-075
#[test]
fn truncation_columns_read_the_engine_facts() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "truncation", 1);
    common::insert(&file, &[common::row(1, "limited")]);
    file.conn().execute_batch("INSERT INTO url_ext (url_id,key,v_int) VALUES (1,'crawl.links_truncated',1),(1,'crawl.truncated',0)").unwrap();
    let source = os_analyze::tabs::internal_crawl::column_source().unwrap();
    for c in source.columns(&CrawlConfig::default()) {
        let ColumnSql::Expr(sql) = c.sql else {
            panic!("scalar fact");
        };
        let value: bool = file
            .conn()
            .query_row(
                &format!("SELECT {sql} FROM urls u WHERE u.id = 1"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, c.key == "Links Truncated");
    }
}
