#[path = "sidebar_support.rs"]
mod support;
use os_export::{
    catalog::TableJob,
    reports::{self, sidebar},
    Format,
};
use os_model::{CrawlConfig, Value};

// spec: OUT-025
#[test]
fn structure_csv_and_xlsx_have_directories_and_complete_depth_classes() {
    use calamine::{open_workbook, Data, Reader, Xlsx};
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let job = TableJob {
        entry_name: "Site Structure",
        tabs: &[],
        file: &file,
        cfg: &cfg,
    };
    let result =
        sidebar::export_site_structure(&job, &dir.path().join("csv"), Format::Csv).unwrap();
    assert_eq!(result.files.len(), 2);
    support::golden(&result.files[0].path, "site_structure_directories.csv");
    support::golden(&result.files[1].path, "site_structure_depth.csv");
    let (_, depth) = sidebar::site_structure(&job).unwrap();
    assert_eq!(depth.rows.len(), 12);
    assert_eq!(depth.rows[1][1], Value::Int(1));
    assert_eq!(depth.rows[1][3], Value::Int(1));
    assert_eq!(depth.rows[11][1], Value::Int(11));
    assert_eq!(
        depth
            .rows
            .iter()
            .map(|r| if let Value::Int(n) = r[7] { n } else { 0 })
            .sum::<i64>(),
        14
    );
    let path = dir.path().join("structure.xlsx");
    sidebar::export_site_structure(&job, &path, Format::Xlsx).unwrap();
    let mut book: Xlsx<_> = open_workbook(path).unwrap();
    assert_eq!(book.sheet_names(), ["Directories", "Crawl Depth"]);
    let depth = book.worksheet_range("Crawl Depth").unwrap();
    assert_eq!(depth.get_value((12, 1)), Some(&Data::Float(11.)));
}

// spec: OUT-025
#[test]
fn response_time_intervals_are_half_open_and_percentages_use_known_responses() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let result = reports::save_report_in(
        &[],
        &file,
        &cfg,
        "Sidebar:Response Times",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    support::golden(&result.files[0].path, "response_times.csv");
    let csv = support::csv(&result.files[0].path);
    assert_eq!(csv[1][1], "1");
    assert_eq!(csv[2][1], "1");
    assert_eq!(csv[3][1], "1");
    assert_eq!(csv[11][1], "1");
    assert_eq!(csv[1][2], "25.00");
}

// spec: OUT-025
#[test]
fn spelling_aggregates_instances_distinct_urls_and_top_hundred() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let result = reports::save_report_in(
        &[],
        &file,
        &cfg,
        "Sidebar:Spelling & Grammar",
        &dir.path().join("first"),
        Format::Csv,
    )
    .unwrap();
    support::golden(&result.files[0].path, "spelling.csv");
    for n in 0..110 {
        file.conn().execute("INSERT INTO spelling_findings(url_id,kind,language,section,text) VALUES(1,'grammar','de','body',?1)",[format!("Error {n:03}")]).unwrap();
    }
    let result = reports::save_report_in(
        &[],
        &file,
        &cfg,
        "Sidebar:Spelling & Grammar",
        &dir.path().join("top"),
        Format::Csv,
    )
    .unwrap();
    let rows = support::csv(&result.files[0].path);
    assert_eq!(rows.len(), 101);
    assert_eq!(rows[1], vec!["Beispeil", "Spelling", "3", "5"]);
    assert_eq!(rows[2][0], "Error 000");
    assert_eq!(rows[100][0], "Error 098");
}

// spec: OUT-025
#[test]
fn overview_reuses_report_and_api_and_segments_export_visible_counts() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let mut cfg = CrawlConfig::default();
    cfg.segments.list.push(os_model::config::SegmentConfig {
        name: "First".into(),
        ..Default::default()
    });
    cfg.segments.list.push(os_model::config::SegmentConfig {
        name: "Second".into(),
        ..Default::default()
    });
    let one = reports::save_report_in(
        &[],
        &file,
        &cfg,
        "Sidebar:Overview",
        &dir.path().join("panel"),
        Format::Csv,
    )
    .unwrap();
    let two = reports::save_report_in(
        &[],
        &file,
        &cfg,
        "Crawl Overview",
        &dir.path().join("report"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(
        support::csv(&one.files[0].path),
        support::csv(&two.files[0].path)
    );
    for (name, golden) in [
        ("Sidebar:API", "api.csv"),
        ("Sidebar:Segments", "segments.csv"),
    ] {
        let result =
            reports::save_report_in(&[], &file, &cfg, name, dir.path(), Format::Csv).unwrap();
        support::golden(&result.files[0].path, golden);
    }
}

// spec: OUT-025
#[test]
fn issues_use_the_shared_counts_and_bilingual_catalog_texts() {
    use os_analyze::{issues, views};
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let expected =
        issues::issues_from_counts(&views::filter_counts_in(&tabs, &file, &cfg).unwrap());
    let result = reports::save_report_in(
        &tabs,
        &file,
        &cfg,
        "Sidebar:Issues",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    let rows = support::csv(&result.files[0].path);
    assert_eq!(rows.len(), expected.len() + 1);
    for (row, issue) in rows.iter().skip(1).zip(&expected) {
        assert_eq!(row[3], issue.count.to_string());
        assert!(!row[5].is_empty());
        assert!(!row[6].is_empty());
        assert!(!row[7].is_empty());
    }
    for entry in issues::catalog() {
        for lang in issues::Lang::ALL {
            let texts = issues::raw_texts(&entry.doc_key, *lang).unwrap();
            assert!(!texts.what.is_empty() && !texts.why.is_empty() && !texts.fix.is_empty());
        }
    }
}

// spec: OUT-025
#[test]
fn segment_issues_are_intersected_with_membership_using_the_shared_plan() {
    use os_analyze::{defs::placeholder_tab, FilterDef, FilterKind};
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let mut cfg = CrawlConfig::default();
    for name in ["First", "Second"] {
        cfg.segments.list.push(os_model::config::SegmentConfig {
            name: name.into(),
            ..Default::default()
        });
    }
    let mut missing = FilterDef::placeholder("Missing", FilterKind::Live, &[]);
    missing.predicate = "u.word_count > :minimum";
    let mut tab = placeholder_tab("Page Titles", vec![missing]);
    tab.population = "u.id=1";
    tab.params = |_| vec![("minimum", Value::Int(50))];
    let result = reports::save_report_in(
        &[tab],
        &file,
        &cfg,
        "Sidebar:Segments",
        dir.path(),
        Format::Csv,
    )
    .unwrap();
    let rows = support::csv(&result.files[0].path);
    assert_eq!(rows[1][4], "1");
    assert_eq!(rows[2][4], "0");
}

// spec: OUT-025
#[test]
fn depth_counts_hidden_internal_urls_and_blocked_and_no_response_separately() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    file.conn().execute_batch("UPDATE urls SET in_tables=0,robots_blocked=1 WHERE id=2; UPDATE urls SET status_code=NULL WHERE id=3;").unwrap();
    let (_, depth) = sidebar::site_structure(&TableJob {
        entry_name: "Site Structure",
        tabs: &[],
        file: &file,
        cfg: &cfg,
    })
    .unwrap();
    assert_eq!(depth.rows[1][5], Value::Int(1));
    assert_eq!(depth.rows[1][6], Value::Int(1));
    assert_eq!(depth.rows[1][7], Value::Int(2));
}
