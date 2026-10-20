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

// spec: OUT-011
#[test]
fn segment_filenames_resolve_configured_thresholds_like_regular_tab_exports() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "names.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    let mut cfg = cfg();
    cfg.title.max_chars = 17;
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    let query =
        os_analyze::views::ViewQuery::new("Page Titles", "Over {title.max_chars} Characters");
    let info = segment::export_view(
        &file,
        &cfg,
        &query,
        Some(&SegmentFilter::new("B")),
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(
        info.path.file_name().unwrap(),
        "page_titles_over_17_characters_segment_b.csv"
    );
}

// spec: OUT-011
#[test]
fn colliding_segment_names_keep_all_tab_bulk_and_report_files_and_contents() {
    let dir = tempfile::tempdir().unwrap();
    let file = fixture::new_file(dir.path(), "collisions.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    fixture::exec(&file, "UPDATE urls SET status_code=301,redirect_type='http',redirect_target_id=2 WHERE id=1; UPDATE urls SET status_code=302,redirect_type='http',redirect_target_id=3 WHERE id=2;");
    let mut cfg = cfg();
    cfg.segments.list = ["Blog News", "Blog_News", "Blog/News", "日本", "中文"]
        .iter()
        .zip(["/a", "/b", "/c", "/missing1", "/missing2"])
        .map(|(name, value)| {
            let mut segment = cfg.segments.list[0].clone();
            segment.name = (*name).into();
            segment.groups[0].conditions[0].value = value.into();
            segment
        })
        .collect();
    segments::reassign(&file, &cfg, &Default::default()).unwrap();
    for (kind, name) in [
        ("tab", "Internal:All"),
        ("bulk", "Links:All Outlinks"),
        ("bulk", "Response Codes:Internal:All Inlinks"),
        ("report", "Redirects:All Redirects"),
        ("report", "Crawl Overview"),
    ] {
        let selection = ExportSelection {
            kind: kind.into(),
            name: name.into(),
        };
        let files = segment::export_all(
            &file,
            &cfg,
            &selection,
            Some("all"),
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        assert_eq!(files.len(), cfg.segments.list.len() + 1, "{name}");
        let paths = files
            .iter()
            .map(|info| info.path.to_string_lossy().to_lowercase())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(paths.len(), files.len(), "{name}");
        for info in &files {
            let rows = fixture::parse_csv(&fixture::read_csv(&info.path));
            assert_eq!(rows.len() as u64, info.rows + 1, "{}", info.path.display());
        }
        let contents = files
            .iter()
            .map(|info| std::fs::read(&info.path).unwrap())
            .collect::<Vec<_>>();
        // Repeated and single-segment exports must select the same stable filenames.
        for (index, definition) in cfg.segments.list.iter().enumerate() {
            let single_dir = tempfile::tempdir().unwrap();
            let single = segment::export_all(
                &file,
                &cfg,
                &selection,
                Some(&definition.name),
                single_dir.path(),
                Format::Csv,
            )
            .unwrap();
            assert_eq!(
                single[0].path.file_name(),
                files[index + 1].path.file_name()
            );
            assert_eq!(std::fs::read(&single[0].path).unwrap(), contents[index + 1]);
        }
        let repeated = segment::export_all(
            &file,
            &cfg,
            &selection,
            Some("all"),
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        for (index, info) in repeated.iter().enumerate() {
            assert_eq!(info.path, files[index].path);
            assert_eq!(std::fs::read(&info.path).unwrap(), contents[index]);
        }
        if kind == "tab" {
            assert_eq!(
                files.iter().map(|info| info.rows).collect::<Vec<_>>(),
                [3, 1, 1, 1, 0, 0]
            );
            for (index, address) in [
                "https://site.test/a",
                "https://site.test/b",
                "https://site.test/c",
            ]
            .iter()
            .enumerate()
            {
                let rows =
                    fixture::parse_csv(&String::from_utf8(contents[index + 1].clone()).unwrap());
                let column = rows[0].iter().position(|value| value == "Address").unwrap();
                assert_eq!(&rows[1][column], address);
            }
        }
    }
}
