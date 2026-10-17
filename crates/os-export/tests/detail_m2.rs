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
