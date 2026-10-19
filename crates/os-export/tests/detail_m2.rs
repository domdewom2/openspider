#[path = "links_support.rs"]
mod support;
use os_export::{
    detail::{export_detail, export_detail_with},
    types::ExportCtx,
    DetailKind, Format,
};
use os_model::{CancelToken, UrlId};
use support::*;

// spec: OUT-022
#[test]
fn multiple_selected_urls_stream_links_into_one_file_and_deduplicate_selection() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "details.ospider");
    exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let path = dir.path().join("details.csv");
    let info = export_detail(
        &file,
        DetailKind::Inlinks,
        &[UrlId(1), UrlId(2), UrlId(3), UrlId(1)],
        &path,
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info.rows, 6);
    let rows = parse_csv(&read_csv(&path));
    assert_eq!(rows.len(), 7);
    assert_eq!(rows[0], os_export::links_export::link_header());
    assert!(rows[1..4].iter().all(|r| r[2] == "https://site.test/b"));
    assert!(rows[4..].iter().all(|r| r[2] == "https://site.test/a"));
}

// spec: OUT-010
// spec: OUT-022
#[test]
fn detail_cancellation_cleans_up_output() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "details.ospider");
    exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let path = dir.path().join("details.csv");
    let cancel = CancelToken::new();
    let progress = |_| cancel.cancel();
    let mut ctx = ExportCtx {
        cancel: &cancel,
        progress: &progress,
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    assert!(export_detail_with(
        &file,
        DetailKind::Inlinks,
        &[UrlId(1), UrlId(2)],
        &path,
        Format::Csv,
        &mut ctx
    )
    .is_err());
    assert!(!path.exists());
}

// spec: OUT-022
// spec: OUT-010
#[test]
fn live_selection_without_target_index_matches_indexed_rows_and_selection_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "live-details.ospider");
    exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let ids = [UrlId(2), UrlId(1), UrlId(2), UrlId(3)];
    let indexed = export_detail(
        &file,
        DetailKind::Inlinks,
        &ids,
        &dir.path().join("indexed.csv"),
        Format::Csv,
    )
    .unwrap();
    exec(&file, "DROP INDEX links_target");
    let reader = os_store::CrawlFile::open(file.path(), os_store::OpenMode::ReadOnly).unwrap();
    assert!(!os_store::link_index::present(reader.conn()).unwrap());
    let unindexed = export_detail(
        &reader,
        DetailKind::Inlinks,
        &ids,
        &dir.path().join("unindexed.csv"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(unindexed.rows, indexed.rows);
    assert_eq!(
        std::fs::read(unindexed.path).unwrap(),
        std::fs::read(indexed.path).unwrap()
    );
    assert!(!os_store::link_index::present(reader.conn()).unwrap());
}
