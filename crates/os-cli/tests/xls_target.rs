//! BIFF8 fixture coverage for the CLI's legacy target.
use calamine::{Reader, Xls};
use std::path::{Path, PathBuf};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/out-m2-cli")
        .join(name)
}

// spec: OUT-195 [M2]
#[test]
fn legacy_strings_handle_unicode_continuations_and_never_become_formulas() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("edge.xls");
    os_cli::xls::convert(&fixture("legacy.csv"), &path).unwrap();
    let mut book: Xls<_> = calamine::open_workbook(&path).unwrap();
    let range = book.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!(range.get_value((1, 1)).unwrap().to_string(), "日本語 🕷");
    assert!(range
        .get_value((2, 1))
        .unwrap()
        .to_string()
        .starts_with("=SUM(A1)"));
    assert!(book
        .worksheet_formula("Export")
        .unwrap()
        .cells()
        .all(|(_, _, v)| v.is_empty()));
    let csv = dir.path().join("large.csv");
    let mut writer = csv::Writer::from_path(&csv).unwrap();
    writer.write_record(["Title"]).unwrap();
    writer.write_record(["🕷日本".repeat(9000)]).unwrap();
    for _ in 0..65535 {
        writer.write_record(["last"]).unwrap();
    }
    writer.flush().unwrap();
    let warnings = os_cli::xls::convert(&csv, &path).unwrap();
    assert_eq!(warnings.len(), 2);
    let mut book: Xls<_> = calamine::open_workbook(&path).unwrap();
    assert_eq!(book.sheet_names().len(), 2);
    assert_eq!(
        book.worksheet_range_at(0).unwrap().unwrap().rows().count(),
        65536
    );
    assert_eq!(
        book.worksheet_range_at(1)
            .unwrap()
            .unwrap()
            .get_value((1, 0))
            .unwrap()
            .to_string(),
        "last"
    );
}
