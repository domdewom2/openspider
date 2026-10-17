#[path = "links_support.rs"]
mod support;
use calamine::{open_workbook, Reader, Xlsx};
use os_analyze::{
    tabs,
    views::{ColumnSelection, ViewQuery},
};
use os_export::{
    pipeline::{export_batch, export_view, snapshot, BatchOptions},
    types::{ExportCtx, ExportSelection},
    Format,
};
use os_model::{CancelToken, CrawlConfig, CrawlMode};
use std::{
    cell::{Cell, RefCell},
    path::Path,
};
use support::*;

fn fixture(dir: &Path) -> os_store::CrawlFile {
    let file = new_file(dir, "fixture.ospider");
    exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    file
}
fn selections() -> Vec<ExportSelection> {
    vec![
        ExportSelection {
            kind: "tab".into(),
            name: "Internal:All".into(),
        },
        ExportSelection {
            kind: "tab".into(),
            name: "Response Codes:All".into(),
        },
        ExportSelection {
            kind: "bulk".into(),
            name: "Links:All Inlinks".into(),
        },
    ]
}
fn control<'a>(
    cancel: &'a CancelToken,
    progress: &'a dyn Fn(u64),
    skip_empty: bool,
) -> ExportCtx<'a> {
    ExportCtx {
        cancel,
        progress,
        skip_empty,
        workbook: None,
        skipped: Vec::new(),
    }
}

// spec: OUT-007
// spec: OUT-075
#[test]
fn workbook_preserves_selection_order_and_fixed_bulk_columns() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let cancel = CancelToken::new();
    let counts = RefCell::new(Vec::new());
    let progress = |n| counts.borrow_mut().push(n);
    let mut ctx = control(&cancel, &progress, false);
    let cfg = CrawlConfig::default();
    let result = export_batch(
        &file,
        &cfg,
        &selections(),
        &BatchOptions {
            format: Format::Xlsx,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: true,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(result.skipped.is_empty());
    assert_eq!(result.files.len(), 1);
    assert_eq!(
        result.files[0].path.file_name().unwrap(),
        "demo_export.xlsx"
    );
    let mut wb: Xlsx<_> = open_workbook(&result.files[0].path).unwrap();
    assert_eq!(
        wb.sheet_names(),
        ["Internal All", "Response Codes All", "Links All Inlinks"]
    );
    assert_eq!(wb.worksheet_range("Internal All").unwrap().height(), 4);
    assert_eq!(wb.worksheet_range("Links All Inlinks").unwrap().height(), 7);
    assert_eq!(wb.worksheet_range("Links All Inlinks").unwrap().width(), 15);
    assert!(counts.borrow().windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(counts.borrow().last(), Some(&12));
}

// spec: OUT-009
#[test]
fn configured_columns_and_explicit_override_keep_address_first_and_warn_on_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let mut cfg = CrawlConfig::default();
    cfg.ui.columns.insert(
        "Internal".into(),
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/out-m2-export/columns.json"
        ))
        .unwrap(),
    );
    let tabs = tabs::configured(&cfg);
    let mut q = ViewQuery::new("Internal", "All");
    let path = dir.path().join("columns.csv");
    let info = export_view(&tabs, &file, &cfg, &q, &path, Format::Csv, "columns").unwrap();
    assert_eq!(
        read_csv(&path).lines().next().unwrap(),
        "Address,Title 1,Status Code"
    );
    assert_eq!(info.warnings.len(), 1);
    assert!(info.warnings[0].contains("does not exist"));
    q.columns = ColumnSelection::Named(vec!["Status Code".into()]);
    export_view(&tabs, &file, &cfg, &q, &path, Format::Csv, "columns").unwrap();
    assert_eq!(
        read_csv(&path).lines().next().unwrap(),
        "Address,Status Code"
    );
}

// spec: OUT-012
// spec: OUT-007
#[test]
fn empty_tables_are_omitted_without_creating_a_file_or_sheet() {
    for format in [Format::Csv, Format::Xlsx] {
        let dir = tempfile::tempdir().unwrap();
        let file = fixture(dir.path());
        let cancel = CancelToken::new();
        let mut ctx = control(&cancel, &|_| {}, true);
        let mut items = selections();
        items.insert(
            0,
            ExportSelection {
                kind: "tab".into(),
                name: "Response Codes:Client Error (4xx)".into(),
            },
        );
        let result = export_batch(
            &file,
            &CrawlConfig::default(),
            &items,
            &BatchOptions {
                format,
                directory: dir.path(),
                crawl_name: "demo",
                single_workbook: true,
            },
            &mut ctx,
        )
        .unwrap();
        assert_eq!(result.skipped, ["Response Codes:Client Error (4XX)"]);
        assert!(!dir
            .path()
            .join("response_codes_client_error_4xx.csv")
            .exists());
        if format == Format::Csv {
            assert_eq!(result.files.len(), 3);
            assert!(result.warnings.iter().any(|w| w.contains("CSV")));
        } else {
            let wb: Xlsx<_> = open_workbook(&result.files[0].path).unwrap();
            assert_eq!(wb.sheet_names().len(), 3);
        }
        let only_empty = &items[..1];
        let result = export_batch(
            &file,
            &CrawlConfig::default(),
            only_empty,
            &BatchOptions {
                format,
                directory: dir.path(),
                crawl_name: "empty",
                single_workbook: true,
            },
            &mut ctx,
        )
        .unwrap();
        assert!(result.files.is_empty());
        assert!(!dir.path().join("empty_export.xlsx").exists());
        ctx.skip_empty = false;
        let result = export_batch(
            &file,
            &CrawlConfig::default(),
            only_empty,
            &BatchOptions {
                format,
                directory: dir.path(),
                crawl_name: "headers",
                single_workbook: false,
            },
            &mut ctx,
        )
        .unwrap();
        assert_eq!(result.files[0].rows, 0);
    }
}

// spec: OUT-010
#[test]
fn cancellation_removes_partial_files_and_preserves_previous_destination() {
    for format in [Format::Csv, Format::Xlsx] {
        let dir = tempfile::tempdir().unwrap();
        let file = fixture(dir.path());
        let cancel = CancelToken::new();
        let progress = |n| {
            if n >= 2 {
                cancel.cancel();
            }
        };
        let mut ctx = control(&cancel, &progress, false);
        let path = dir.path().join(if format == Format::Csv {
            "internal_all.csv"
        } else {
            "demo_export.xlsx"
        });
        std::fs::write(&path, b"previous complete export").unwrap();
        let start = std::time::Instant::now();
        assert!(export_batch(
            &file,
            &CrawlConfig::default(),
            &selections(),
            &BatchOptions {
                format,
                directory: dir.path(),
                crawl_name: "demo",
                single_workbook: format == Format::Xlsx,
            },
            &mut ctx
        )
        .is_err());
        assert!(start.elapsed().as_secs_f64() < 2.0);
        assert_eq!(std::fs::read(&path).unwrap(), b"previous complete export");
        assert!(!std::fs::read_dir(dir.path()).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "part")));
    }
}

// spec: OUT-010
#[test]
fn batch_uses_the_snapshot_from_before_concurrent_crawl_updates() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let other = os_store::CrawlFile::open(file.path(), os_store::OpenMode::ReadWrite).unwrap();
    let modified = Cell::new(false);
    let progress = |_| {
        if !modified.replace(true) {
            exec(
                &other,
                "UPDATE urls SET title_1 = 'changed after export start'",
            );
        }
    };
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &progress, false);
    let items = vec![
        selections().remove(0),
        ExportSelection {
            kind: "tab".into(),
            name: "Page Titles:All".into(),
        },
    ];
    export_batch(
        &file,
        &CrawlConfig::default(),
        &items,
        &BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: false,
        },
        &mut ctx,
    )
    .unwrap();
    let text = read_csv(&dir.path().join("page_titles_all.csv"));
    assert!(text.contains("Beta"));
    assert!(!text.contains("changed after"));
    assert!(file.conn().is_autocommit());
    snapshot(&file, || snapshot(&file, || Ok(()))).unwrap();
    assert!(file.conn().is_autocommit());
}

// spec: OUT-015 [M2]
#[test]
fn list_order_repeats_duplicates_and_reports_invalid_and_excluded_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let mut cfg = CrawlConfig {
        mode: CrawlMode::List,
        ..CrawlConfig::default()
    };
    cfg.ui
        .columns
        .insert("Internal".into(), vec!["Title 1".into(), "Status".into()]);
    let tabs = tabs::configured(&cfg);
    let path = dir.path().join("list.csv");
    export_view(
        &tabs,
        &file,
        &cfg,
        &ViewQuery::new("Internal", "All"),
        &path,
        Format::Csv,
        "list",
    )
    .unwrap();
    let rows = parse_csv(&read_csv(&path));
    assert_eq!(rows[0], ["Original Url", "Address", "Title 1", "Status"]);
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[1], rows[3]);
    assert_eq!(rows[1][1], "https://site.test/b");
    assert_eq!(rows[2][1], "https://site.test/a");
    assert_eq!(rows[4], ["https://[bad", "", "", "Invalid"]);
    assert_eq!(rows[5][3], "Excluded");
    // K2-48 / CRAWL-027: schemaless inputs are ignored.
    assert!(!rows.iter().flatten().any(|v| v == "example.test/c"));
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, false);
    let info = os_export::tab_export::export_tab_with(
        &file,
        &cfg,
        &ViewQuery::new("Internal", "All"),
        dir.path(),
        Format::Csv,
        false,
        &mut ctx,
    )
    .unwrap()
    .unwrap();
    let path = info.path.clone();
    assert_eq!(info.rows, 3);
    assert_eq!(
        read_csv(&path).lines().next().unwrap(),
        "Address,Title 1,Status"
    );
}

// spec: OUT-066
#[test]
fn generic_registry_covers_every_filter_and_missing_title_inlinks_keep_instances() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let expected: usize = tabs::all()
        .iter()
        .filter(|t| os_export::catalog::bulk_generic::TABS.contains(&t.id))
        .map(|t| t.filters.len())
        .sum();
    let count = os_export::catalog::bulk_tables()
        .iter()
        .filter(|entry| {
            entry
                .name
                .rsplit_once(':')
                .is_some_and(|(tab, _)| os_export::catalog::bulk_generic::TABS.contains(&tab))
        })
        .count();
    assert_eq!(count, expected);
    let files = os_export::export_bulk(
        &file,
        &CrawlConfig::default(),
        "Page Titles:Missing Inlinks",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    let rows = parse_csv(&read_csv(&files[0].path));
    assert_eq!(rows.len(), 4);
    assert!(rows[1..].iter().all(|r| r[2] == "https://site.test/a"));
    assert_eq!(rows[1][1], rows[2][1]);
    let mut cfg = CrawlConfig::default();
    cfg.export.inlinks_for_all_filters = false;
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, false);
    let result = export_batch(
        &file,
        &cfg,
        &[
            ExportSelection {
                kind: "bulk".into(),
                name: "Page Titles:Missing Inlinks".into(),
            },
            ExportSelection {
                kind: "bulk".into(),
                name: "Pagination:Contains Pagination Inlinks".into(),
            },
        ],
        &BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: false,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(result.files.is_empty());
    assert_eq!(result.skipped.len(), 2);
}

// spec: OUT-075
// spec: OUT-196
#[test]
fn portable_preset_roundtrips_and_unknown_names_warn_without_stopping_other_exports() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let mut preset = os_export::types::ExportPreset::parse(include_str!(
        "../../../tests/fixtures/out-m2-export/basic.ospreset"
    ))
    .unwrap();
    preset.items[2] = ExportSelection {
        kind: "tab".into(),
        name: "Future Tab:All".into(),
    };
    let path = dir.path().join("basic.ospreset");
    preset.save(&path).unwrap();
    assert_eq!(os_export::types::ExportPreset::load(&path).unwrap(), preset);
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, preset.skip_empty);
    let result = export_batch(
        &file,
        &CrawlConfig::default(),
        &preset.items,
        &BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: preset.single_workbook,
        },
        &mut ctx,
    )
    .unwrap();
    assert_eq!(result.files.len(), 2);
    assert_eq!(result.skipped, ["Future Tab:All"]);
    assert_eq!(result.warnings.len(), 1);
}

// spec: OUT-066
#[test]
fn configured_numeric_threshold_inlink_names_resolve_to_the_registered_filter() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    exec(&file, "UPDATE urls SET title_1_len = 80 WHERE id = 1");
    let mut cfg = CrawlConfig::default();
    cfg.title.max_chars = 70;
    let files = os_export::export_bulk(
        &file,
        &cfg,
        "page titles : over 70 characters INLINKS",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(files[0].rows, 3);
    let rows = parse_csv(&read_csv(&files[0].path));
    assert!(rows[1..].iter().all(|r| r[2] == "https://site.test/b"));
}

// spec: OUT-075
#[test]
fn missing_prerequisites_are_named_and_skipped_while_other_exports_run() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, false);
    let mut items = selections();
    items.insert(
        0,
        ExportSelection {
            kind: "report".into(),
            name: "Cookie Summary".into(),
        },
    );
    let result = export_batch(
        &file,
        &CrawlConfig::default(),
        &items,
        &BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: false,
        },
        &mut ctx,
    )
    .unwrap();
    assert_eq!(result.files.len(), 3);
    assert_eq!(result.skipped, ["Cookie Summary"]);
    assert_eq!(result.warnings.len(), 1);
    assert!(!dir.path().join("cookie_summary.csv").exists());
}

// spec: OUT-011
#[test]
fn segment_scope_extension_can_restrict_a_tab_and_generic_inlink_targets() {
    struct AssignedSegment;
    impl os_export::types::SegmentScope for AssignedSegment {
        fn population_sql(&self) -> String {
            "u.id = 2".into()
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let cfg = CrawlConfig::default();
    let tabs = os_export::pipeline::scoped_tabs(&file, &cfg, &AssignedSegment);
    let path = dir.path().join("scope.csv");
    let info = export_view(
        &tabs,
        &file,
        &cfg,
        &ViewQuery::new("Internal", "All"),
        &path,
        Format::Csv,
        "scope",
    )
    .unwrap();
    assert_eq!(info.rows, 1);
    let info = os_export::pipeline::export_bulk_in(
        &tabs,
        &file,
        &cfg,
        "Page Titles:All Inlinks",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info[0].rows, 3);
    assert!(parse_csv(&read_csv(&info[0].path))[1..]
        .iter()
        .all(|row| row[2] == "https://site.test/a"));
}

// spec: OUT-007
// spec: OUT-075
#[test]
fn reports_share_the_workbook_and_asset_manifests_stay_separate() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    exec(
        &file,
        "UPDATE urls SET status_code=301, redirect_target_id=2 WHERE id=3",
    );
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, false);
    let items = [
        ExportSelection {
            kind: "tab".into(),
            name: "Internal:All".into(),
        },
        ExportSelection {
            kind: "report".into(),
            name: "Redirects:All Redirects".into(),
        },
        ExportSelection {
            kind: "bulk".into(),
            name: "Web:All Page Source".into(),
        },
    ];
    let result = export_batch(
        &file,
        &cfg,
        &items,
        &BatchOptions {
            format: Format::Xlsx,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: true,
        },
        &mut ctx,
    )
    .unwrap();
    assert_eq!(result.files.len(), 2);
    assert!(result.skipped.is_empty());
    let mut workbook: Xlsx<_> = open_workbook(dir.path().join("demo_export.xlsx")).unwrap();
    assert_eq!(
        workbook.sheet_names(),
        ["Internal All", "Redirects All Redirects"]
    );
    assert_eq!(
        workbook
            .worksheet_range("Redirects All Redirects")
            .unwrap()
            .height(),
        2
    );
    assert!(dir.path().join("web_all_page_source.xlsx").is_file());
}

// spec: OUT-075
#[test]
fn disabled_api_filters_are_skipped_even_without_skip_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    let cfg = CrawlConfig::default();
    let cancel = CancelToken::new();
    let mut ctx = control(&cancel, &|_| {}, false);
    let items = [
        ExportSelection {
            kind: "tab".into(),
            name: "Search Console:Clicks Above 0".into(),
        },
        ExportSelection {
            kind: "bulk".into(),
            name: "Search Console:Clicks Above 0 Inlinks".into(),
        },
    ];
    let result = export_batch(
        &file,
        &cfg,
        &items,
        &BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "demo",
            single_workbook: false,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(result.files.is_empty());
    assert_eq!(result.skipped.len(), 2);
    assert_eq!(result.warnings.len(), 2);
}

// spec: OUT-015 [M2]
#[test]
fn sitemap_only_list_sources_fall_back_to_the_crawled_view() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture(dir.path());
    exec(&file, "DELETE FROM list_input; INSERT INTO list_input(position,original,outcome) VALUES(1,'https://site.test/sitemap.xml','sitemap')");
    let cfg = CrawlConfig {
        mode: CrawlMode::List,
        ..Default::default()
    };
    let tabs = tabs::configured(&cfg);
    let path = dir.path().join("sitemap-list.csv");
    let info = export_view(
        &tabs,
        &file,
        &cfg,
        &ViewQuery::new("Internal", "All"),
        &path,
        Format::Csv,
        "list",
    )
    .unwrap();
    assert_eq!(info.rows, 3);
    assert_eq!(
        os_export::pipeline::batch_row_count(&file, &cfg, &selections()[..1]).unwrap(),
        Some(3)
    );
    assert_eq!(parse_csv(&read_csv(&path))[0][0], "Address");
}
