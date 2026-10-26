use calamine::{Reader, Xlsx};
use os_engine::{list_input, serp_input, start_crawl, testing, CrawlRequest, CrawlSource};
use os_export::{types::ExportCtx, Format};
use os_model::{CancelToken, CrawlConfig};
use os_store::{CrawlFile, OpenMode};
use std::path::Path;

async fn import(
    site: &os_testsite::TestSite,
    cfg: CrawlConfig,
    source: CrawlSource,
    path: &Path,
) -> CrawlFile {
    let deps = testing::test_deps();
    start_crawl(
        testing::site_fetcher(site, &cfg, &deps),
        CrawlRequest {
            config: cfg,
            source,
            file: path.into(),
            project: "e06".into(),
            name: "export acceptance".into(),
        },
        deps,
    )
    .await
    .unwrap()
    .wait()
    .await
    .unwrap();
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

fn table(path: &Path) -> Vec<Vec<String>> {
    if path.extension().unwrap() == "xlsx" {
        let mut workbook: Xlsx<_> = calamine::open_workbook(path).unwrap();
        workbook
            .worksheet_range_at(0)
            .unwrap()
            .unwrap()
            .rows()
            .map(|r| r.iter().map(ToString::to_string).collect())
            .collect()
    } else {
        csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(path)
            .unwrap()
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect()
    }
}

// spec: CRAWL-029
#[tokio::test]
async fn upload_export_csv_and_xlsx_preserve_all_ten_rows() {
    let site = os_testsite::TestSite::start("crawl-m2-list").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let inputs: Vec<String> = include_str!("dupes.txt")
        .lines()
        .map(str::to_string)
        .collect();
    let mut cfg = list_input::list_config(CrawlConfig::default());
    cfg.scope.exclude = vec![".*/privat/.*".into()];
    let file = import(
        &site,
        cfg.clone(),
        CrawlSource::List {
            items: inputs.clone(),
            label: "dupes.txt".into(),
        },
        &dir.path().join("list.ospider"),
    )
    .await;
    let mut exported = Vec::new();
    for format in [Format::Csv, Format::Xlsx] {
        let cancel = CancelToken::new();
        let mut ctx = ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: false,
            workbook: None,
            skipped: Vec::new(),
        };
        let info = os_export::tab_export::export_tab_with(
            &file,
            &cfg,
            &os_analyze::views::ViewQuery::new("Internal", "All"),
            dir.path(),
            format,
            true,
            &mut ctx,
        )
        .unwrap()
        .unwrap();
        assert_eq!(info.rows, 10);
        let rows = table(&info.path);
        assert_eq!(rows.len(), 11);
        let column = |name| rows[0].iter().position(|s| s == name).unwrap();
        assert_eq!(rows[0][0], "Original Url");
        assert_eq!(
            rows[1..].iter().map(|r| r[0].clone()).collect::<Vec<_>>(),
            inputs
        );
        assert_eq!(rows[4][column("Address")], "http://site.test/a");
        assert_eq!(
            rows[4][column("Status Code")],
            rows[1][column("Status Code")]
        );
        assert_eq!(rows[3][column("Status Code")], "");
        assert_eq!(rows[3][column("Status")], "Excluded");
        // XLSX stores dates and numbers as typed cells, unlike CSV. Compare
        // the contractual upload fields without conflating their display formats.
        let fields = [
            "Original Url",
            "Address",
            "Status Code",
            "Status",
            "Title 1",
        ];
        exported.push(
            rows[1..]
                .iter()
                .map(|row| {
                    fields
                        .iter()
                        .map(|field| row[column(field)].clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(exported[0], exported[1]);
}

// spec: CRAWL-031
// spec: OUT-098 [M2]
#[tokio::test]
async fn serp_summary_roundtrips_twice_through_csv_and_xlsx_and_matches_tabs() {
    let site = os_testsite::TestSite::start("crawl-m2-list").await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let source = serp_input::parse(include_str!("serp.csv")).unwrap();
    for (kind, format) in [("csv", Format::Csv), ("xlsx", Format::Xlsx)] {
        let cfg = CrawlConfig::default();
        let mut file = import(
            &site,
            cfg.clone(),
            CrawlSource::Serp {
                rows: source.clone(),
                label: "serp.csv".into(),
            },
            &dir.path().join(format!("{kind}-0.ospider")),
        )
        .await;
        let mut original = None;
        for generation in 1..=2 {
            let out = dir.path().join(format!("{kind}-{generation}"));
            let result = os_export::save_report(&file, &cfg, "SERP Summary", &out, format).unwrap();
            let data = table(&result.files[0].path);
            assert_eq!(data.len(), 6);
            assert_eq!(
                data[0],
                [
                    "Address",
                    "Title 1",
                    "Title 1 Length",
                    "Title 1 Pixel Width",
                    "Meta Description 1",
                    "Meta Description 1 Length",
                    "Meta Description 1 Pixel Width"
                ]
            );
            if let Some(ref first) = original {
                assert_eq!(&data, first);
            } else {
                original = Some(data.clone());
            }
            for (row, input) in data[1..].iter().zip(&source) {
                assert_eq!(
                    row,
                    &vec![
                        input.url.clone(),
                        input.title.clone(),
                        os_parse::text::char_len(&input.title).to_string(),
                        os_parse::pixel::width_px(&input.title, 20.0, false).to_string(),
                        input.description.clone(),
                        os_parse::text::char_len(&input.description).to_string(),
                        os_parse::pixel::width_px(&input.description, 14.0, false).to_string()
                    ]
                );
            }
            for (tab, fields, offset) in [
                (
                    "Page Titles",
                    ["Title 1", "Title 1 Length", "Title 1 Pixel Width"],
                    1,
                ),
                (
                    "Meta Description",
                    [
                        "Meta Description 1",
                        "Meta Description 1 Length",
                        "Meta Description 1 Pixel Width",
                    ],
                    4,
                ),
            ] {
                let view = os_analyze::views::query_view(
                    &file,
                    &cfg,
                    &os_analyze::views::ViewQuery::new(tab, "All"),
                )
                .unwrap();
                assert_eq!(view.rows.len(), source.len());
                for (record, row) in data[1..].iter().zip(&view.rows) {
                    for (i, field) in fields.iter().enumerate() {
                        let col = view.columns.iter().position(|c| c == field).unwrap();
                        let value = match &row[col] {
                            os_model::Value::Text(v) => v.clone(),
                            os_model::Value::Int(v) => v.to_string(),
                            v => panic!("unexpected {v:?}"),
                        };
                        assert_eq!(record[offset + i], value);
                    }
                }
                let duplicates = os_analyze::views::query_view(
                    &file,
                    &cfg,
                    &os_analyze::views::ViewQuery::new(tab, "Duplicate"),
                )
                .unwrap();
                assert_eq!(
                    duplicates.rows.len(),
                    if tab == "Page Titles" { 0 } else { 2 }
                );
            }
            let imported = serp_input::read(&result.files[0].path).unwrap();
            assert_eq!(imported, source);
            file = import(
                &site,
                cfg.clone(),
                CrawlSource::Serp {
                    rows: imported,
                    label: "roundtrip".into(),
                },
                &dir.path().join(format!("{kind}-{generation}.ospider")),
            )
            .await;
        }
    }
    assert!(site.requests().is_empty());
}
