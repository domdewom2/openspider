use calamine::{open_workbook, Data, Reader, Xlsx};
use os_analyze::ColumnKind;
use os_export::{
    open_writer, types::WorkbookTarget, xlsx::XlsxWriter, Format, TableWriter, WriterOptions,
};
use os_model::Value;

fn options(label: &str) -> WriterOptions {
    WriterOptions {
        header_labels: [("Address".into(), label.into())].into(),
        ..Default::default()
    }
}

// spec: OUT-001 [M3]
#[test]
fn interleaved_writers_keep_their_own_headers_and_default_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let mut de = open_writer(
        &dir.path().join("de.csv"),
        Format::Csv,
        "T",
        &options("Adresse"),
    )
    .unwrap();
    let mut en = open_writer(
        &dir.path().join("en.csv"),
        Format::Csv,
        "T",
        &options("URL"),
    )
    .unwrap();
    let mut plain = open_writer(
        &dir.path().join("plain.csv"),
        Format::Csv,
        "T",
        &WriterOptions::default(),
    )
    .unwrap();
    // A concrete writer can move to another thread before its header is written.
    let mut thread_writer = os_export::csv::CsvWriter::create(
        &dir.path().join("thread.csv"),
        &options("Andere Adresse"),
    )
    .unwrap();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            thread_writer.header(&["Address".into()]).unwrap();
            Box::new(thread_writer).finish().unwrap();
        });
        for writer in [&mut de, &mut plain, &mut en] {
            writer.header(&["Address".into(), "Count".into()]).unwrap();
            writer
                .row(
                    &[ColumnKind::Url, ColumnKind::Int],
                    &[Value::Text("https://example.test/".into()), Value::Int(7)],
                )
                .unwrap();
        }
    });
    for (writer, name, header) in [
        (de, "de.csv", "Adresse"),
        (en, "en.csv", "URL"),
        (plain, "plain.csv", "Address"),
    ] {
        writer.finish().unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(name)).unwrap(),
            format!("\u{feff}{header},Count\r\nhttps://example.test/,7\r\n").as_bytes()
        );
    }
    assert_eq!(
        std::fs::read(dir.path().join("thread.csv")).unwrap(),
        "\u{feff}Andere Adresse\r\n".as_bytes()
    );
}

// spec: OUT-001 [M3]
#[test]
fn workbook_headers_keep_labels_across_sheets_and_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("labels.xlsx");
    let mut opts = options("Adresse");
    opts.max_rows_per_sheet = 1;
    let mut workbook = XlsxWriter::create(&path, "Export", &opts).unwrap();
    // A later mutation of caller-owned options cannot affect this writer.
    opts.header_labels
        .insert("Address".into(), "Changed".into());
    for name in ["First", "Second"] {
        let writer = workbook.sheet(name).unwrap();
        writer.header(&["Address".into(), "Count".into()]).unwrap();
        for _ in 0..2 {
            writer
                .row(
                    &[ColumnKind::Url, ColumnKind::Int],
                    &[Value::Text("https://example.test/".into()), Value::Int(7)],
                )
                .unwrap();
        }
    }
    Box::new(workbook).finish().unwrap();
    let mut reader: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(reader.sheet_names().len(), 4);
    for name in reader.sheet_names() {
        let range = reader.worksheet_range(&name).unwrap();
        assert_eq!(range.get((0, 0)), Some(&Data::String("Adresse".into())));
        assert_eq!(range.get((0, 1)), Some(&Data::String("Count".into())));
    }
}
