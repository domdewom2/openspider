#[path = "../../os-integrations/tests/drive_support.rs"]
mod support;
use os_export::{
    target_gsheet::{self, Destination, Table},
    timeseries::{Measurement, DATE_FORMAT},
    TableWriter,
};
use os_model::{CancelToken, Timestamp, Value};
use serde_json::json;
use std::collections::BTreeMap;

fn table(
    name: &str,
    n: u64,
) -> Table<impl Iterator<Item = Result<Vec<Value>, os_export::ExportError>>> {
    Table {
        name: name.into(),
        header: vec!["Address".into(), "Value".into()],
        row_count: n,
        rows: (0..n).map(|i| {
            Ok(vec![
                Value::Text(format!("https://site.test/{i}")),
                Value::Text("=1+1".into()),
            ])
        }),
    }
}
// spec: OUT-005
#[tokio::test]
async fn streamed_raw_blocks_consolidation_replacement_and_cell_budget() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let cancel = CancelToken::new();
    let target = Destination::new("Project", "Audit", "Export");
    let first = target_gsheet::export(
        &client,
        &target,
        vec![table("Internal", 10_002), table("Other", 1)],
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(first.rows, 10_003);
    let puts: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert!(puts.iter().all(|r| r.query["valueInputOption"] == "RAW"
        && r.body["values"].as_array().unwrap().len() <= 10_000));
    assert_eq!(puts[1].body["values"][0][1], "=1+1");
    let file = mock.files().into_iter().find(|f| f.id == first.id).unwrap();
    assert_eq!(file.sheets.len(), 2);
    let sheet_id = file.sheets[0].id;
    let mut limited = target.clone();
    limited.cell_limit = 8;
    let next = target_gsheet::export(&client, &limited, vec![table("Internal", 8)], &cancel)
        .await
        .unwrap();
    assert_eq!(first.id, next.id);
    assert_eq!(next.rows, 3);
    assert!(next.warnings[0].contains("5 Zeilen abgeschnitten"));
    assert!(next.warnings[0].contains("CSV/XLSX"));
    let file = mock.files().into_iter().find(|f| f.id == next.id).unwrap();
    assert_eq!(file.sheets.len(), 1);
    assert_eq!(file.sheets[0].id, sheet_id);
    assert_eq!(file.sheets[0].rows.len(), 4);
    mock.trash(&next.id);
    let new = target_gsheet::export(&client, &target, vec![table("Internal", 0)], &cancel)
        .await
        .unwrap();
    assert_ne!(new.id, next.id);
    let count = mock.requests().len();
    let mut impossible = target.clone();
    impossible.cell_limit = 1;
    assert!(
        target_gsheet::export(&client, &impossible, vec![table("Internal", 1)], &cancel)
            .await
            .is_err()
    );
    assert_eq!(mock.requests().len(), count);
}

// spec: OUT-210
// spec: OUT-212
#[tokio::test]
async fn three_runs_have_numeric_dates_and_metrics_with_stable_headers_and_renaming() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let cancel = CancelToken::new();
    let target = Destination::new("Project", "Audit", "Task");
    let mut measurement = Measurement {
        started_at: Timestamp::from_db("2026-10-13T07:35:00Z").unwrap(),
        start_url: "https://site.test/".into(),
        values: BTreeMap::from([
            ("Internal:HTML".into(), Value::Int(7)),
            ("Response Codes:Client Error (4xx)".into(), Value::Int(2)),
            ("Average Word Count".into(), Value::Float(42.5)),
        ]),
    };
    let first = [
        "Internal:HTML".into(),
        "Response Codes:Client Error (4xx)".into(),
    ];
    let second = ["Average Word Count".into(), "Internal:HTML".into()];
    let third = ["Internal:HTML".into(), "Average Word Count".into()];
    let mut id = String::new();
    for selection in [&first[..], &second[..], &third[..]] {
        let result = target_gsheet::custom_summary(
            &client,
            &target,
            selection,
            &measurement,
            time::UtcOffset::from_hms(2, 0, 0).unwrap(),
            &cancel,
        )
        .await
        .unwrap();
        if !id.is_empty() {
            assert_eq!(result.id, id);
        }
        id = result.id;
        measurement.started_at = measurement
            .started_at
            .plus(std::time::Duration::from_secs(60));
    }
    let file = mock.files().into_iter().find(|f| f.id == id).unwrap();
    assert_eq!(file.name, "Task_custom_summary_report");
    let sheet = &file.sheets[0];
    assert_eq!(sheet.title, "data");
    assert_eq!(sheet.date_format.as_deref(), Some(DATE_FORMAT));
    assert_eq!(sheet.rows.len(), 4);
    assert_eq!(
        sheet.rows[0],
        vec![
            json!("Date"),
            json!("Start URL"),
            json!(first[0]),
            json!(first[1]),
            json!("Average Word Count")
        ]
    );
    assert_eq!(sheet.rows[1][2], json!(7.0));
    assert_eq!(sheet.rows[1][3], json!(2.0));
    assert_eq!(sheet.rows[2][3], "");
    assert_eq!(sheet.rows[2][4], 42.5);
    assert_eq!(sheet.rows[3][3], "");
    assert_eq!(sheet.rows[3][2], json!(7.0));
    assert!(sheet.rows[1][0].is_number());
    assert!(sheet.rows[1][0].as_f64() < sheet.rows[2][0].as_f64());
    let expected = os_export::timeseries::date_serial(
        Timestamp::from_db("2026-10-13T07:35:00Z").unwrap(),
        time::UtcOffset::from_hms(2, 0, 0).unwrap(),
    );
    assert_eq!(sheet.rows[1][0], expected);
    let mut renamed = target.clone();
    renamed.name = "Renamed".into();
    let other = target_gsheet::custom_summary(
        &client,
        &renamed,
        &third,
        &measurement,
        time::UtcOffset::UTC,
        &cancel,
    )
    .await
    .unwrap();
    assert_ne!(other.id, id);
    assert_eq!(
        mock.files()
            .into_iter()
            .find(|f| f.id == id)
            .unwrap()
            .sheets[0]
            .rows
            .len(),
        4
    );
}

// spec: OUT-005
#[tokio::test]
async fn existing_table_writer_pipeline_can_stage_and_upload_without_formula_escaping() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let mut writer = target_gsheet::StagingWriter::create(&dir.path().join("stage.jsonl")).unwrap();
    writer.header(&["Text".into()]).unwrap();
    writer
        .row(
            &[os_analyze::ColumnKind::Text],
            &[Value::Text("=1+1".into())],
        )
        .unwrap();
    let info = Box::new(writer).finish().unwrap();
    let staged = target_gsheet::staged_table("Values", &info).unwrap();
    let result = target_gsheet::export(
        &client,
        &Destination::new("Project", "Crawl", "Staged"),
        vec![staged],
        &CancelToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        mock.files()
            .into_iter()
            .find(|f| f.id == result.id)
            .unwrap()
            .sheets[0]
            .rows[1][0],
        "=1+1"
    );
}

// spec: OUT-005
#[tokio::test]
async fn revoked_google_target_leaves_other_local_exports_available_and_cancel_sends_nothing() {
    let (_server, mock, client, _, _) = support::fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local.csv");
    let mut writer = os_export::csv::CsvWriter::create(&path, &Default::default()).unwrap();
    writer.header(&["Address".into()]).unwrap();
    writer
        .row(
            &[os_analyze::ColumnKind::Text],
            &[Value::Text("https://site.test/".into())],
        )
        .unwrap();
    Box::new(writer).finish().unwrap();
    mock.fail_next("GET", "/drive/v3/files", 401);
    let target = Destination::new("Project", "Crawl", "Export");
    let error = target_gsheet::export(
        &client,
        &target,
        vec![table("Internal", 1)],
        &CancelToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("Google-Konto erneut verbinden"));
    assert!(path.is_file());
    let count = mock.requests().len();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        target_gsheet::export(&client, &target, vec![table("Internal", 1)], &cancel)
            .await
            .is_err()
    );
    assert_eq!(count, mock.requests().len());
}

// spec: OUT-183 [M2]
#[tokio::test]
async fn sheets_source_preserves_ten_upload_positions_while_engine_plans_eight_urls() {
    use os_integrations::drive;
    use os_testsite::mocks::drive::Sheet;
    let (_server, mock, client, _, _) = support::fixture().await;
    let urls: Vec<_> = ["b", "a", "c", "d", "e", "f", "g", "h", "b", "a"]
        .iter()
        .map(|name| json!(format!("https://site.test/{name}")))
        .collect();
    let id = mock.seed(
        "Upload",
        "root",
        drive::SPREADSHEET,
        vec![
            Sheet {
                id: 0,
                title: "First".into(),
                rows: vec![urls[..5].to_vec()],
                date_format: None,
            },
            Sheet {
                id: 1,
                title: "Second".into(),
                rows: vec![urls[5..].to_vec()],
                date_format: None,
            },
        ],
    );
    let items = client.list_items(&drive::link(&id)).await.unwrap();
    let plan = os_engine::list_input::plan(&items, &Default::default()).unwrap();
    assert_eq!(plan.counts.read, 10);
    assert_eq!(plan.counts.duplicates, 2);
    assert_eq!(plan.counts.to_crawl, 8);
    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| entry.position)
            .collect::<Vec<_>>(),
        (1..=10).collect::<Vec<_>>()
    );
    assert_eq!(items[0], "https://site.test/b");
    assert_eq!(items[8], items[0]);
}

// spec: OUT-005
#[tokio::test]
async fn shared_catalog_stages_regular_tables_for_consolidated_sheets() {
    use os_export::types::{ExportCtx, ExportSelection};
    use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds};
    use os_store::{CrawlFile, CrawlInit, StorageProfile};
    let (_server, mock, client, _, _) = support::fixture().await;
    let directory = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let file = CrawlFile::create(
        &directory.path().join("crawl.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(73).new_crawl_id(Timestamp::UNIX_EPOCH),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "Project".into(),
            name: "Crawl".into(),
            config: cfg.clone(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/out-m2-export/crawl.sql"
        ))
        .unwrap();
    let cancel = CancelToken::new();
    let mut control = ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: vec![],
    };
    let staged = target_gsheet::stage_batch(
        &file,
        &cfg,
        &[
            ExportSelection {
                kind: "tab".into(),
                name: "Internal:All".into(),
            },
            ExportSelection {
                kind: "tab".into(),
                name: "Page Titles:All".into(),
            },
        ],
        directory.path(),
        &mut control,
    )
    .unwrap();
    assert!(staged.warnings.is_empty(), "{:?}", staged.warnings);
    assert_eq!(staged.tables.len(), 2);
    let tables = staged
        .tables
        .iter()
        .map(|(name, info)| target_gsheet::staged_table(name, info).unwrap())
        .collect();
    let result = target_gsheet::export(
        &client,
        &Destination::new("Project", "Crawl", "Combined"),
        tables,
        &cancel,
    )
    .await
    .unwrap();
    let files = mock.files();
    let sheet = files.iter().find(|f| f.id == result.id).unwrap();
    assert_eq!(sheet.sheets.len(), 2);
    assert_eq!(sheet.sheets[0].title, "Internal All");
    assert_eq!(sheet.sheets[1].title, "Page Titles All");
    assert_eq!(sheet.sheets[0].rows[0][0], "Address");
    assert!(sheet.sheets[0].rows.len() > 1);
}
