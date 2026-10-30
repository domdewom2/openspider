#![cfg(windows)]
mod store_common;
use os_store::{CrawlFile, OpenMode};

// spec: OUT-262
#[test]
fn crawl_file_can_be_saved_and_reopened_beyond_max_path() {
    let root = tempfile::tempdir().unwrap();
    let mut dir = root.path().to_path_buf();
    while dir.as_os_str().len() < 280 {
        dir.push("long-crawl-fixture");
    }
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("fixture.ospider");
    store_common::build_paused_crawl(&source);
    let target = dir.join("saved.ospider");
    let file = CrawlFile::open(&source, OpenMode::ReadOnly).unwrap();
    file.save_as(&target).unwrap();
    let reopened = CrawlFile::open(&target, OpenMode::ReadOnly).unwrap();
    let expected: i64 = file
        .conn()
        .query_row("SELECT count(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    let actual: i64 = reopened
        .conn()
        .query_row("SELECT count(*) FROM urls", [], |r| r.get(0))
        .unwrap();
    assert!(actual > 0);
    assert_eq!(actual, expected);
}
