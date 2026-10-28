mod store_common;

use os_model::WriteBatch;
use os_store::{url_index::DEFERRED_INDEXES, CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

// spec: PERF-002
#[test]
fn deferred_indexes_return_at_completion() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "indexes.ospider", StorageProfile::Balanced);
    let mut writer = sink(&path, StorageProfile::Balanced).with_url_index_drop_at(1);
    writer.write(batch_with_urls([(1, true)])).unwrap();
    let check = |expected| {
        let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
        for index in DEFERRED_INDEXES {
            let present: bool = file
                .conn()
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1)",
                    [index.name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(present, expected, "{}", index.name);
        }
    };
    check(false);
    writer
        .write(WriteBatch {
            meta: vec![("state".into(), "Completed".into())],
            ..Default::default()
        })
        .unwrap();
    check(true);
}
