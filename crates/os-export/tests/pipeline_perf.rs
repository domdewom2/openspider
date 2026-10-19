#[path = "links_support.rs"]
mod support;
use os_analyze::views::{ColumnSelection, ViewQuery};
use os_export::{pipeline::export_view, Format};
use os_model::CrawlConfig;
use support::*;

// spec: PERF-008 [M2]
#[test]
#[ignore = "Benchmark: one million CSV rows in less than 60 seconds with bounded RSS"]
fn million_csv_rows_stream_in_under_sixty_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "million.ospider");
    exec(&file, "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<1000000)
        INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,title_1)
        SELECT i, 'https://site.test/'||i, 'https://site.test/'||i, 'https://site.test',1,'done','link',0,30,200,'Title '||i FROM n");
    let cfg = CrawlConfig::default();
    let tabs = os_analyze::tabs::configured(&cfg);
    let mut q = ViewQuery::new("Internal", "All");
    q.columns = ColumnSelection::Named(vec![
        "Address".into(),
        "Title 1".into(),
        "Status Code".into(),
    ]);
    let baseline = rss_mib();
    let start = std::time::Instant::now();
    let (info, peak) = peak_rss_during(|| {
        export_view(
            &tabs,
            &file,
            &cfg,
            &q,
            &dir.path().join("million.csv"),
            Format::Csv,
            "million",
        )
        .unwrap()
    });
    let elapsed = start.elapsed().as_secs_f64();
    assert_eq!(info.rows, 1_000_000);
    assert!(elapsed < 60.0, "CSV export took {elapsed:.2}s");
    if let (Some(baseline), Some(peak)) = (baseline, peak) {
        assert!(
            peak - baseline < 128.0,
            "RSS increase: {} MiB",
            peak - baseline
        );
        eprintln!(
            "1,000,000 rows: {elapsed:.2}s, peak RSS increase {:.1} MiB",
            peak - baseline
        );
    }
}

// spec: PERF-008 [M2]
#[test]
#[ignore = "Benchmark: actual Excel row limit, 1,048,576 data rows"]
fn actual_excel_limit_creates_a_continuation_sheet_and_warning() {
    use calamine::{open_workbook, Reader, Xlsx};
    use os_export::{xlsx::XlsxWriter, TableWriter};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.xlsx");
    let mut writer = XlsxWriter::create(&path, "Rows", &Default::default()).unwrap();
    writer.header(&["Number".into()]).unwrap();
    for i in 0..1_048_576 {
        writer
            .row(&[os_analyze::ColumnKind::Int], &[os_model::Value::Int(i)])
            .unwrap();
    }
    let info = Box::new(writer).finish().unwrap();
    assert_eq!(info.rows, 1_048_576);
    assert_eq!(info.warnings.len(), 1);
    let workbook: Xlsx<_> = open_workbook(path).unwrap();
    assert_eq!(workbook.sheet_names(), ["Rows", "Rows (2)"]);
}
