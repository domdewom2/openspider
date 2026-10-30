#![cfg(windows)]
use os_analyze::ColumnKind;
use os_export::{output::open_writer, Format, WriterOptions};
use os_model::Value;

// spec: OUT-262
#[test]
fn csv_export_roundtrips_beyond_max_path() {
    let root = tempfile::tempdir().unwrap();
    let mut dir = root.path().to_path_buf();
    while dir.as_os_str().len() < 280 {
        dir.push("long-export-fixture");
    }
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.csv");
    let mut writer =
        open_writer(&path, Format::Csv, "Internal", &WriterOptions::default()).unwrap();
    writer.header(&["Title".into()]).unwrap();
    writer
        .row(&[ColumnKind::Text], &[Value::Text("Grüße".into())])
        .unwrap();
    assert_eq!(writer.finish().unwrap().rows, 1);
    assert!(String::from_utf8(std::fs::read(path).unwrap())
        .unwrap()
        .contains("Grüße"));
}
