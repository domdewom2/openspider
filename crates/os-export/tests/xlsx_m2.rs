use calamine::{open_workbook, Reader, Xlsx};
use os_analyze::ColumnKind;
use os_export::{types::WorkbookTarget, xlsx::XlsxWriter, TableWriter, WriterOptions};
use os_model::Value;

// spec: OUT-007
// spec: PERF-008 [M2]
#[test]
fn combined_workbook_splits_full_sheets_and_resolves_collisions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("combined.xlsx");
    let opts = WriterOptions {
        max_rows_per_sheet: 2,
        ..Default::default()
    };
    let mut workbook = XlsxWriter::create(&path, "Export", &opts).unwrap();
    for (name, count, header) in [
        ("A/B", 3, vec!["Address".into()]),
        ("A:B", 1, vec!["Address".into(), "Count".into()]),
    ] {
        let writer = workbook.sheet(name).unwrap();
        writer.header(&header).unwrap();
        for n in 0..count {
            let values = if header.len() == 1 {
                vec![Value::Text(format!("url {n}"))]
            } else {
                vec![Value::Text("url".into()), Value::Int(n)]
            };
            writer
                .row(&[ColumnKind::Url, ColumnKind::Int], &values)
                .unwrap();
        }
    }
    let info = Box::new(workbook).finish().unwrap();
    assert_eq!(info.rows, 4);
    assert_eq!(info.warnings.len(), 1);
    let mut wb: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(wb.sheet_names(), ["A B", "A B (2)", "A B (3)"]);
    assert_eq!(wb.worksheet_range("A B").unwrap().height(), 3);
    assert_eq!(wb.worksheet_range("A B (2)").unwrap().height(), 2);
    assert_eq!(wb.worksheet_range("A B (3)").unwrap().width(), 2);
}
