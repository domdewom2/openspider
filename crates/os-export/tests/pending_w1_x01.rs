#[path = "links_support.rs"]
mod support;
use support::*;

// spec: OUT-066
#[test]
fn pagination_filter_inlinks_are_supplied_by_the_pagination_package() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "pagination.ospider");
    exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    exec(
        &file,
        "INSERT INTO pagination(url_id,rel,source,position,raw,url,target_id,in_head)
        VALUES(2,'next','html',1,'https://site.test/c','https://site.test/c',3,1)",
    );
    let files = os_export::export_bulk(
        &file,
        &os_model::CrawlConfig::default(),
        "Pagination:Contains Pagination Inlinks",
        dir.path(),
        os_export::Format::Csv,
    )
    .unwrap();
    assert_eq!(files[0].rows, 3);
    let rows = parse_csv(&read_csv(&files[0].path));
    assert!(rows[1..].iter().all(|row| row[2] == "https://site.test/a"));
}
