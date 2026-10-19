//! Deterministic synthetic crawl fixture shared by K01 tests and criterion.
#![allow(dead_code)]
#[path = "framework_common.rs"]
mod common;
use os_analyze::views::{session::SessionSpec, ColumnSelection};
use os_analyze::SortKey;
use os_model::CrawlConfig;
use os_store::CrawlFile;

pub fn synthetic(count: u32) -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "synthetic.ospider", 192);
    file.conn().execute(
        "WITH RECURSIVE n(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM n WHERE id < ?1) \
         INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, folder_depth, url_length, content_class, status_code, title_1, title_count, size_bytes) \
         SELECT id, printf('https://fixture.test/%08d', id), printf('https://fixture.test/%08d', id), 'https://fixture.test', 1, 1, 'Done', 'Link', 0, 38, 'HTML', \
         200 + (id % 5), CASE WHEN id % 17 = 0 THEN NULL ELSE printf('Title %08d', (id * 7919) % ?1) END, 1, (id * 3571) % 100000 FROM n",
        [count],
    ).unwrap();
    common::set_change_seq(&file, 1);
    file.set_meta("state", "Completed").unwrap();
    (dir, file)
}
pub fn specification(column: &str, descending: bool) -> SessionSpec {
    let tab = os_analyze::tabs::configured(&CrawlConfig::default())
        .into_iter()
        .find(|t| t.id == "Internal")
        .unwrap();
    let columns = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .take(20)
        .map(|c| c.key.to_string())
        .collect::<Vec<_>>();
    assert_eq!(columns.len(), 20);
    SessionSpec {
        tab: "Internal".into(),
        filter: "All".into(),
        search: None,
        columns: ColumnSelection::Named(columns),
        sort: vec![SortKey {
            column: column.into(),
            descending,
        }],
        restrict: vec![],
    }
}
