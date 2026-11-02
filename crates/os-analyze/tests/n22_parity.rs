//! N22: independent SQL oracle on a deterministic Unicode store fixture.
#[path = "framework_common.rs"]
mod common;

use os_analyze::views::session::{Budget, RowsReq, SessionSpec, ViewSession};
use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_analyze::SortKey;
use os_model::{CrawlConfig, UrlId};

// spec: UI-141
#[test]
fn extracted_sort_keys_equal_sql_collation_and_tie_order_on_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = common::create(dir.path(), "n22-sort.ospider", 19);
    let texts = [
        "A", "a", "ä", "a\u{308}", "Å", "ß", "ss", "İ", "ı", "ς", "σ", "東京", "😀", "", "Z", "é",
        "e\u{301}", "\u{fffe}",
    ];
    let tx = file.conn().unchecked_transaction().unwrap();
    // Crosses the block-sort and merge boundary; repeated collation ties and NULLs.
    for i in 1..=33_001 {
        let address = format!("https://n22.test/{:06}", 40_000 - i);
        let mut row = common::row(i, &address)
            .text("content_class", "HTML")
            .text("title_1", texts[i as usize % texts.len()])
            .int("status_code", 200 + i % 5);
        if i % 23 == 0 {
            row = row.null("title_1").null("status_code");
        }
        common::insert(&file, &[row]);
    }
    tx.commit().unwrap();
    common::set_change_seq(&file, 1);
    let cfg = CrawlConfig::default();
    for column in ["Title 1", "Status Code", "Address"] {
        let mut session = None;
        for descending in [false, true] {
            let sort = vec![SortKey {
                column: column.into(),
                descending,
            }];
            let mut q = ViewQuery::new("Internal", "All");
            q.columns = ColumnSelection::Named(vec!["Address".into()]);
            q.sort = sort.clone();
            let mut expected = Vec::new();
            views::stream_view(&file, &cfg, &q, &mut |row| {
                expected.push(row[0].clone());
                Ok(())
            })
            .unwrap();
            let spec = SessionSpec {
                tab: "Internal".into(),
                filter: "All".into(),
                search: None,
                sort,
                columns: q.columns,
                restrict: vec![],
            };
            let view = session.get_or_insert_with(|| {
                ViewSession::open(
                    &file,
                    &cfg,
                    SessionSpec {
                        tab: "Internal".into(),
                        filter: "All".into(),
                        search: None,
                        sort: vec![],
                        columns: ColumnSelection::Named(vec!["Address".into()]),
                        restrict: vec![],
                    },
                    Budget::default(),
                )
                .unwrap()
            });
            view.update(&file, &cfg, spec).unwrap();
            let mut actual = Vec::new();
            for offset in (0..expected.len()).step_by(500) {
                let page = view
                    .rows(
                        &file,
                        RowsReq {
                            version: view.version(),
                            offset: offset as u64,
                            limit: 500,
                        },
                    )
                    .unwrap();
                for (id, row) in page.row_keys.iter().zip(page.rows) {
                    assert!(matches!(id, UrlId(1..=33_001)));
                    actual.push(row[0].clone());
                }
            }
            assert_eq!(actual, expected, "{column} descending={descending}");
        }
    }
}
