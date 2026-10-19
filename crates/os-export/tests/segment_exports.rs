#[path = "links_support.rs"]
mod fixture;
use os_analyze::segments::{self, SegmentFilter};
use os_export::{segment, types::ExportSelection, Format};
use os_model::{CrawlConfig, SegmentCondition, SegmentConfig, SegmentGroup, SegmentOp};
fn cfg() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.segments.list = vec![SegmentConfig {
        name: "B".into(),
        color: "#123456".into(),
        groups: vec![SegmentGroup {
            conditions: vec![SegmentCondition {
                field: "Address".into(),
                op: SegmentOp::EndsWith,
                value: "/b".into(),
            }],
        }],
    }];
    cfg
}
// spec: OUT-011
#[test]
fn all_exports_keep_unfiltered_output_and_each_saved_segment() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "segments.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    // Changed rule inputs must not cause the export to recalculate membership.
    let mut changed = cfg.clone();
    changed.segments.list[0].groups[0].conditions[0].value = "/a".into();
    let files = segment::export_all(
        &file,
        &changed,
        &ExportSelection {
            kind: "tab".into(),
            name: "Internal:All".into(),
        },
        Some("all"),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].rows, 3);
    assert_eq!(files[1].rows, 1);
    assert!(files[0].path.exists());
    assert!(files[1].path.to_string_lossy().contains("_segment_b"));
    let text = fixture::read_csv(&files[1].path);
    assert!(text.contains("https://site.test/b"));
    assert!(!text.contains("https://site.test/a"));
}
// spec: OUT-011
#[test]
fn inlinks_select_targets_and_outlinks_select_sources() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "links.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let scope = SegmentFilter::new("B");
    let files = segment::export_bulk(
        &file,
        &cfg,
        "Links:All Inlinks",
        Some(&scope),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files[0].rows, 3);
    let text = fixture::read_csv(&files[0].path);
    let rows = fixture::parse_csv(&text);
    assert!(rows[1..].iter().all(|r| r[2] == "https://site.test/b"));
    assert!(rows[1..].iter().any(|r| r[1] == "https://site.test/c"));
    let files = segment::export_bulk(
        &file,
        &cfg,
        "Links:All Outlinks",
        Some(&scope),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files[0].rows, 2);
    let text = fixture::read_csv(&files[0].path);
    let rows = fixture::parse_csv(&text);
    assert!(rows[1..].iter().all(|r| r[1] == "https://site.test/b"));
}
// spec: OUT-011
#[test]
fn filtered_bulk_and_overview_reports_use_the_same_saved_population() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "reports.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let scope = SegmentFilter::new("B");
    let files = segment::export_bulk(
        &file,
        &cfg,
        "Response Codes:Internal:All Inlinks",
        Some(&scope),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files[0].rows, 3);
    let report = segment::export_report(
        &file,
        &cfg,
        "Crawl Overview",
        Some(&scope),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert!(report.files[0]
        .path
        .to_string_lossy()
        .contains("_segment_b"));
    assert!(!fixture::read_csv(&report.files[0].path).is_empty());
    assert_eq!(
        file.conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        3
    );
    assert!(segment::selections(&file, Some("missing")).is_err());
}

// spec: OUT-011
#[test]
fn all_bulk_outputs_coexist_and_segment_reports_keep_external_chain_context() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "coexist.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let cfg = cfg();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let files = segment::export_all(
        &file,
        &cfg,
        &ExportSelection {
            kind: "bulk".into(),
            name: "Response Codes:Internal:All Inlinks".into(),
        },
        Some("all"),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].rows, 6);
    assert_eq!(files[1].rows, 3);
    assert!(files.iter().all(|f| f.path.exists()));
    assert_ne!(files[0].path, files[1].path);
    assert_eq!(
        fixture::parse_csv(&fixture::read_csv(&files[0].path)).len(),
        7
    );
    file.conn().execute_batch("UPDATE urls SET status_code=301,status='Moved Permanently',redirect_type='http',redirect_target_id=2 WHERE id=1").unwrap();
    let report = segment::export_report(
        &file,
        &cfg,
        "Redirects:All Redirects",
        Some(&SegmentFilter::new("B")),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    let rows = fixture::parse_csv(&fixture::read_csv(&report.files[0].path));
    let final_address = rows[0].iter().position(|v| v == "Final Address").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1][final_address], "https://site.test/a");
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='os_segment_report_addresses'",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
}
