#[path = "bulk_web_support.rs"]
mod support;
use os_export::{save_report, Format};
use support::*;

// spec: OUT-110
#[test]
fn header_summary_normalizes_names_counts_urls_and_distinct_values() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let cfg = config();
    let result = save_report(&file, &cfg, "HTTP Header Summary", dir.path(), Format::Csv).unwrap();
    let text = std::fs::read_to_string(&result.files[0].path).unwrap();
    golden("header-summary.csv", &text);
    let rows = records(&text);
    assert_eq!(
        rows.iter().find(|r| r[0] == "cache-control").unwrap(),
        &["cache-control", "10", "83.33", "2", "max-age=60"]
    );
    assert_eq!(
        rows.iter().find(|r| r[0] == "content-type").unwrap(),
        &["content-type", "12", "100.00", "1", "text/html"]
    );
    assert_eq!(
        rows.iter().find(|r| r[0] == "set-cookie").unwrap(),
        &["set-cookie", "1", "8.33", "2", "a=1"]
    );
    assert!(!text.contains("authorization"));
    let result = save_report(&file, &cfg, "HTTP Header Summary", dir.path(), Format::Xlsx).unwrap();
    use calamine::{Reader, Xlsx};
    let mut book: Xlsx<_> = calamine::open_workbook(&result.files[0].path).unwrap();
    let range = book.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!(range.height(), 5);
}

// spec: OUT-110
// spec: OUT-112
#[test]
fn disabled_extraction_reports_a_warning_and_no_data_even_if_old_rows_exist() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.extraction.http_headers = false;
    let result = save_report(&file, &cfg, "HTTP Header Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(result.files[0].rows, 0);
    assert_eq!(
        result.warnings,
        vec!["report 'HTTP Header Summary' requires HTTP header extraction"]
    );
    cfg.extraction.http_headers = true;
    file.conn().execute("DELETE FROM http_headers", []).unwrap();
    file.conn().execute("DELETE FROM cookies", []).unwrap();
    file.conn().execute("DELETE FROM urls", []).unwrap();
    let result = save_report(&file, &cfg, "HTTP Header Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(result.files[0].rows, 0);
    assert!(result.warnings.is_empty());
}
