#[path = "sidebar_support.rs"]
mod support;
use os_export::{
    catalog::TableJob,
    timeseries::{self, CellValue},
};
use os_model::{CrawlConfig, Timestamp, Value};
use time::UtcOffset;

// spec: OUT-210
#[test]
fn three_crawls_make_one_numeric_row_each_with_typed_local_dates() {
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let job = TableJob {
        entry_name: "summary",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    };
    let selected = vec!["Internal:HTML".into(), "Response Codes:All".into()];
    let mut header = vec![];
    let mut rows = vec![];
    for day in 18..21 {
        file.set_meta("started_at", &format!("2026-10-{day}T08:30:00.000Z"))
            .unwrap();
        let measurement = timeseries::measure(&job).unwrap();
        let row = timeseries::align(
            &header,
            &selected,
            &measurement,
            UtcOffset::from_hms(2, 0, 0).unwrap(),
        )
        .unwrap();
        header = row.header.clone();
        rows.push(row.cells);
    }
    assert_eq!(
        header,
        ["Date", "Start URL", "Internal:HTML", "Response Codes:All"]
    );
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert!(matches!(row[0], CellValue::Date(_)));
        assert_eq!(row[1], CellValue::Text("https://site.test/".into()));
        assert_eq!(row[2], CellValue::Number(14.));
        assert_eq!(row[3], CellValue::Number(14.));
    }
    let date = |row: &Vec<CellValue>| {
        if let CellValue::Date(v) = row[0] {
            v
        } else {
            panic!()
        }
    };
    assert_eq!(date(&rows[1]) - date(&rows[0]), 1.);
    assert_eq!(date(&rows[2]) - date(&rows[1]), 1.);
    assert_eq!(
        timeseries::date_serial(Timestamp::UNIX_EPOCH, UtcOffset::UTC),
        25569.
    );
    file.set_meta("mode", "list").unwrap();
    file.set_meta("list_source", "input.txt").unwrap();
    assert_eq!(timeseries::measure(&job).unwrap().start_url, "input.txt");
}

// spec: OUT-211
#[test]
fn numeric_catalog_tracks_registry_and_unpopulated_values_remain_empty() {
    use os_analyze::{defs::placeholder_tab, FilterDef, FilterKind};
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let mut tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let mut tab = placeholder_tab(
        "Fixture",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Unavailable", FilterKind::Live, &[]),
        ],
    );
    tab.population = "u.is_internal=1";
    tabs.push(tab);
    let catalog = timeseries::catalog(&tabs);
    let names = catalog.iter().map(|m| m.name.as_str()).collect::<Vec<_>>();
    assert!(names.contains(&"Fixture:All"));
    assert!(names.contains(&"Fixture:Unavailable"));
    assert!(!names.contains(&"Mode"));
    assert!(!names.contains(&"Crawl Start"));
    assert!(!names.contains(&"OpenSpider Version"));
    assert!(names.contains(&"Average Word Count"));
    assert!(names.contains(&"Issues (High)"));
    assert_eq!(
        names.len(),
        names.iter().collect::<std::collections::HashSet<_>>().len()
    );
    let m = timeseries::measure(&TableJob {
        entry_name: "summary",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    })
    .unwrap();
    assert_eq!(m.values["Fixture:All"], Value::Int(14));
    assert_eq!(m.values["Fixture:Unavailable"], Value::Null);
    assert_eq!(m.values["Average Word Count"], Value::Float(100.));
    let selected =
        timeseries::parse_selection("Fixture:All\r\nFixture:Unavailable\nFixture:All\n\n");
    assert_eq!(selected, ["Fixture:All", "Fixture:Unavailable"]);
    assert!(timeseries::catalog_text(&tabs).contains("Fixture:Unavailable"));
    let row = timeseries::align(&[], &selected, &m, UtcOffset::UTC).unwrap();
    assert_eq!(row.cells[3], CellValue::Empty);
}

// spec: OUT-212
#[test]
fn header_identity_survives_add_remove_reorder_and_missing_metrics() {
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            support::root().join("tests/fixtures/out-m2-reports/timeseries.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let strings = |key: &str| {
        fixture[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let dir = tempfile::tempdir().unwrap();
    let file = support::file(dir.path(), "panels.sql");
    let cfg = CrawlConfig::default();
    let tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
    let m = timeseries::measure(&TableJob {
        entry_name: "summary",
        tabs: &tabs,
        file: &file,
        cfg: &cfg,
    })
    .unwrap();
    let row = timeseries::align(
        &strings("existing"),
        &strings("selected"),
        &m,
        UtcOffset::UTC,
    )
    .unwrap();
    assert_eq!(row.header, strings("expected"));
    assert_eq!(row.cells[2], CellValue::Number(14.));
    assert_eq!(row.cells[3], CellValue::Empty);
    assert_eq!(row.cells[4], CellValue::Number(14.));
    let mut selection = strings("selected");
    selection.reverse();
    let next = timeseries::align(&row.header, &selection, &m, UtcOffset::UTC).unwrap();
    assert_eq!(row, next);
    selection.push("Gone".into());
    let next = timeseries::align(&row.header, &selection, &m, UtcOffset::UTC).unwrap();
    assert_eq!(next.cells.last(), Some(&CellValue::Empty));
    assert!(next
        .warnings
        .contains(&"metric 'Gone' is no longer populated".into()));
    assert!(timeseries::align(
        &["Date".into(), "Start URL".into(), "Date".into()],
        &selection,
        &m,
        UtcOffset::UTC
    )
    .is_err());
}
