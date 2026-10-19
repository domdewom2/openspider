//! Exporte im Vergleichsmodus (M2-C01): Crawl Overview mit Vergleichsspalten (OUT-147), Tab- und Link-Exporte
//! (OUT-151, OUT-149), Reports im Vergleichs- und Segmentkontext (OUT-113), Pause/Abbruch (CRAWL-129).
//! Fixtures: `out-m2-compare/v1`, `v2`, `mapping`, `segments`.

#[path = "../../os-analyze/tests/compare_common.rs"]
mod common;

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use common::*;
use os_analyze::compare::{ChangeFilter, CompareClass, LinkDirection, LinkSource, Side};
use os_analyze::views::ColumnSelection;
use os_export::compare::{self as cx, LinkSelection, OverviewRow, TabSelection};
use os_export::reports::overview::Cell;
use os_export::types::{ExportCtx, Format, SegmentScope};
use os_model::CancelToken;

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn read(path: &Path) -> Table {
        let bytes = std::fs::read(path).unwrap();
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(bytes);
        let mut all: Vec<Vec<String>> = rdr
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect();
        let header = all.remove(0);
        Table { header, rows: all }
    }
    fn col(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("Spalte {name} fehlt in {:?}", self.header))
    }
    fn get<'a>(&self, row: &'a [String], name: &str) -> &'a str {
        &row[self.col(name)]
    }
    fn find(&self, column: &str, value: &str) -> Option<&Vec<String>> {
        let c = self.col(column);
        self.rows.iter().find(|r| r[c] == value)
    }
}

fn ctx(cancel: &CancelToken) -> ExportCtx<'_> {
    ExportCtx {
        cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    }
}

fn csv_writer(path: &Path) -> Box<dyn os_export::types::TableWriter> {
    os_export::open_writer(path, Format::Csv, "Sheet", &Default::default()).unwrap()
}

fn num(v: Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

// spec: OUT-147
#[tokio::test(flavor = "multi_thread")]
async fn crawl_overview_matches_the_comparison_panel() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let dir = tempfile::tempdir().unwrap();
    let result = cx::save_report(&mut cmp, "Crawl Overview", dir.path(), Format::Csv, &CancelToken::new()).unwrap();
    let table = Table::read(&result.files[0].path);
    assert_eq!(table.header, cx::OVERVIEW_HEADER);
    // Kopfblock mit Name und Startzeit beider Crawls.
    let start = table
        .rows
        .iter()
        .find(|r| r[0] == cx::HEADER_SECTION && r[1] == "Crawl Start")
        .expect("Kopfblock");
    assert!(start[2].starts_with("2026-02-01"), "{start:?}");
    assert!(start[3].starts_with("2026-01-01"), "{start:?}");
    assert!(table.rows.iter().any(|r| r[0] == cx::HEADER_SECTION && r[1] == "Name"));
    // Jede Filterzeile hat genau die Werte des Panels.
    let panel = cmp.overview(&CancelToken::new()).unwrap();
    let mut checked = 0;
    for f in &panel {
        let Some(row) = table
            .rows
            .iter()
            .find(|r| r[0] == f.tab && r[1] == f.label)
        else {
            continue;
        };
        assert_eq!(table.get(row, "Current"), num(f.current), "{}:{}", f.tab, f.label);
        assert_eq!(table.get(row, "Previous"), num(f.previous), "{}:{}", f.tab, f.label);
        assert_eq!(
            table.get(row, "Change"),
            f.change().map(|c| c.to_string()).unwrap_or_default()
        );
        for class in CompareClass::ALL {
            assert_eq!(table.get(row, class.as_str()), num(f.class(class)), "{}:{}", f.tab, f.label);
        }
        checked += 1;
    }
    assert!(checked > 100, "{checked}");
    let html = table
        .rows
        .iter()
        .find(|r| r[0] == "Internal" && r[1] == "HTML")
        .unwrap();
    assert_eq!(table.get(html, "New"), "4");
    assert_eq!(table.get(html, "Missing"), "2");
}

// spec: OUT-147
// Change-Detection-Elemente (M2-C02) erscheinen als eigener Abschnitt hinter dem Report.
#[tokio::test(flavor = "multi_thread")]
async fn change_detection_rows_form_their_own_section() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("overview.csv");
    let mut writer = csv_writer(&path);
    let extra = [OverviewRow {
        section: "Change Detection".into(),
        item: "Page Titles".into(),
        current: Cell::Int(1),
        previous: Cell::Int(0),
        classes: None,
    }];
    let cancel = CancelToken::new();
    cx::write_crawl_overview(&mut cmp, &extra, writer.as_mut(), &mut ctx(&cancel)).unwrap();
    writer.finish().unwrap();
    let table = Table::read(&path);
    let last = table.rows.last().unwrap();
    assert_eq!(last[0], "Change Detection");
    assert_eq!(table.get(last, "Change"), "1");
    assert_eq!(
        table.rows.iter().filter(|r| r[0] == "Change Detection").count(),
        1
    );
}

// spec: OUT-151
#[tokio::test(flavor = "multi_thread")]
async fn removed_subfilter_export_carries_the_fixed_404() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("4xx.csv");
    let info = cx::export_tab(
        &mut cmp,
        &TabSelection {
            tab: "Response Codes".into(),
            filter: "Client Error (4XX)".into(),
            class: Some(CompareClass::Removed),
            columns: ColumnSelection::Default,
        },
        &path,
        Format::Csv,
    )
    .unwrap();
    assert_eq!(info.rows, 1);
    let table = Table::read(&path);
    let row = table.find("Address", &www("/missing.html")).expect("behobene 404");
    assert_eq!(table.get(row, cx::STATUS_COLUMN), "Removed");
    // Ohne Mapping keine Spalte Previous Address.
    assert!(!table.header.iter().any(|h| h == cx::PREVIOUS_ADDRESS_COLUMN));
}

// spec: OUT-151
#[tokio::test(flavor = "multi_thread")]
async fn filter_export_lists_statuses_and_missing_rows_come_from_previous() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let dir = tempfile::tempdir().unwrap();
    let all = dir.path().join("html.csv");
    let selection = |class| TabSelection {
        tab: "Internal".into(),
        filter: "HTML".into(),
        class,
        columns: ColumnSelection::Default,
    };
    cx::export_tab(&mut cmp, &selection(None), &all, Format::Csv).unwrap();
    let table = Table::read(&all);
    assert_eq!(table.get(table.find("Address", &www("/new1.html")).unwrap(), cx::STATUS_COLUMN), "New");
    assert_eq!(table.get(table.find("Address", &www("/missing.html")).unwrap(), cx::STATUS_COLUMN), "Added");
    assert_eq!(table.get(table.find("Address", &www("/a.html")).unwrap(), cx::STATUS_COLUMN), "Unchanged");
    assert!(table.find("Address", &www("/gone1.html")).is_none());

    let missing = dir.path().join("html-missing.csv");
    let info = cx::export_tab(&mut cmp, &selection(Some(CompareClass::Missing)), &missing, Format::Csv).unwrap();
    assert_eq!(info.rows, 2);
    let table = Table::read(&missing);
    let addresses: Vec<&str> = table.rows.iter().map(|r| table.get(r, "Address")).collect();
    assert_eq!(addresses, [www("/gone1.html"), www("/gone2.html")]);
    assert!(table.rows.iter().all(|r| table.get(r, cx::STATUS_COLUMN) == "Missing"));
}

// spec: OUT-151
// spec: TECH-306
#[tokio::test(flavor = "multi_thread")]
async fn mapping_adds_the_previous_address() {
    let (old, new) = (staging().await, live().await);
    let rules = vec![
        rule(r"^http://staging\.site\.test", "http://www.site.test"),
        rule(r"(\.test/.+)/$", "$1"),
    ];
    let mut cmp = compare(&old, &new, &rules);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mapped.csv");
    cx::export_tab(
        &mut cmp,
        &TabSelection {
            tab: "Internal".into(),
            filter: "HTML".into(),
            class: None,
            columns: ColumnSelection::Default,
        },
        &path,
        Format::Csv,
    )
    .unwrap();
    let table = Table::read(&path);
    let row = table.find("Address", &www("/a")).unwrap();
    assert_eq!(table.get(row, cx::PREVIOUS_ADDRESS_COLUMN), "http://staging.site.test/a/");
    assert_eq!(table.get(row, cx::STATUS_COLUMN), "Unchanged");
}

// spec: OUT-151
// spec: OUT-149
#[tokio::test(flavor = "multi_thread")]
async fn link_detail_export_carries_the_change_columns() {
    let (old, new) = (v1().await, v2().await);
    let cmp = compare(&old, &new, &[]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inlinks.csv");
    let mut writer = csv_writer(&path);
    let cancel = CancelToken::new();
    let n = cx::write_links(
        &cmp,
        &LinkSelection {
            side: Side::Current,
            url_id: id_of(&new.file, &www("/c.html")),
            direction: LinkDirection::Inlinks,
            source: LinkSource::Auto,
            change: ChangeFilter::All,
        },
        writer.as_mut(),
        &mut ctx(&cancel),
    )
    .unwrap();
    writer.finish().unwrap();
    assert_eq!(n, 1);
    let table = Table::read(&path);
    for column in ["Link Change", "Crawl", "Previous Anchor"] {
        assert!(table.header.iter().any(|h| h == column), "{column}");
    }
    let row = &table.rows[0];
    assert_eq!(table.get(row, "Link Change"), "Anchor Changed");
    assert_eq!(table.get(row, "Crawl"), "Current");
    assert_eq!(table.get(row, "Anchor"), "Neu");
    assert_eq!(table.get(row, "Previous Anchor"), "Alt");
}

// spec: OUT-151
// spec: OUT-113
// Reports außer dem Crawl Overview beziehen sich auf den Current-Crawl.
#[tokio::test(flavor = "multi_thread")]
async fn reports_in_comparison_refer_to_the_current_crawl() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let in_compare = cx::save_report(&mut cmp, "Redirects:All Redirects", a.path(), Format::Csv, &CancelToken::new()).unwrap();
    let plain = os_export::save_report(&new.file, &new.file.config().unwrap(), "Redirects:All Redirects", b.path(), Format::Csv).unwrap();
    assert_eq!(
        std::fs::read(&in_compare.files[0].path).unwrap(),
        std::fs::read(&plain.files[0].path).unwrap()
    );
}

// spec: OUT-113
// Im Vergleichsmodus ist das Reports-Menü bedienbar: jeder Report verhält sich wie für den Current-Crawl allein
// (fehlende Voraussetzungen nach OUT-112 melden beide gleich), der Crawl Overview trägt die Vergleichsspalten.
#[tokio::test(flavor = "multi_thread")]
async fn every_report_can_be_saved_in_comparison_mode() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let cfg = new.file.config().unwrap();
    let names = cx::report_names();
    assert!(names.len() > 3);
    let mut saved = 0;
    for name in names {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let in_compare = cx::save_report(&mut cmp, &name, a.path(), Format::Csv, &CancelToken::new());
        let plain = os_export::save_report(&new.file, &cfg, &name, b.path(), Format::Csv);
        match (&in_compare, &plain) {
            (Ok(r), Ok(_)) => {
                assert!(!r.files.is_empty(), "{name}");
                saved += 1;
            }
            (Err(x), Err(y)) => assert_eq!(x.to_string(), y.to_string(), "{name}"),
            _ => panic!("{name}: {:?} vs {:?}", in_compare.err(), plain.err()),
        }
    }
    assert!(saved > 3);
}

struct Shop;

impl SegmentScope for Shop {
    fn population_sql(&self) -> String {
        "u.address LIKE '%/shop/%'".into()
    }
}

// spec: OUT-113
#[tokio::test(flavor = "multi_thread")]
async fn redirect_chains_with_segment_keep_only_chains_starting_in_it() {
    let fx = crawl_at(
        "out-m2-compare/segments",
        "http://www.site.test/",
        cfg(),
        "2026-01-01T00:00:00.000Z",
        61,
        os_store::StorageProfile::Balanced,
    )
    .await;
    let cfg = fx.file.config().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    let all_path = dir.path().join("all.csv");
    let mut writer = csv_writer(&all_path);
    let job = os_export::catalog::TableJob {
        entry_name: "Redirects:Redirect Chains",
        tabs: &os_analyze::tabs::configured_for_file(&cfg, &fx.file),
        file: &fx.file,
        cfg: &cfg,
    };
    os_export::reports::save_report_to(&job, writer.as_mut(), &mut ctx(&cancel)).unwrap();
    writer.finish().unwrap();
    let all = Table::read(&all_path);
    assert_eq!(all.rows.len(), 2, "beide Ketten ohne Segment");

    let shop_path = dir.path().join("shop.csv");
    let mut writer = csv_writer(&shop_path);
    cx::write_report_in_segment(&fx.file, &cfg, "Redirects:Redirect Chains", &Shop, writer.as_mut(), &mut ctx(&cancel)).unwrap();
    writer.finish().unwrap();
    let shop = Table::read(&shop_path);
    assert_eq!(shop.header, all.header);
    let starts: Vec<&str> = shop.rows.iter().map(|r| shop.get(r, "Address")).collect();
    assert_eq!(starts, [www("/shop/old1")]);
}

// spec: CRAWL-129
// Der Export im Vergleichsmodus pausiert über das Token des Ressourcenwächters und läuft danach weiter.
#[tokio::test(flavor = "multi_thread")]
async fn a_paused_comparison_export_waits_and_resumes() {
    let (old, new) = (v1().await, v2().await);
    let (a, b) = (side(&old), side(&new));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("overview.csv");
    let token = CancelToken::new();
    let worker = token.clone();
    token.pause();
    let (tx, rx) = mpsc::channel();
    let out = path.clone();
    std::thread::spawn(move || {
        let result = (|| {
            let mut cmp = os_analyze::compare::Comparison::open(a, b, &[], &CancelToken::new())
                .map_err(cx::compare_error)?;
            let mut writer = os_export::open_writer(&out, Format::Csv, "Sheet", &Default::default())?;
            cx::write_crawl_overview(&mut cmp, &[], writer.as_mut(), &mut ctx(&worker))?;
            writer.finish()
        })();
        tx.send(result.map(|i| i.rows)).unwrap();
    });
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    token.resume();
    let rows = rx.recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
    assert!(rows > 100);

    let cancelled = CancelToken::new();
    cancelled.cancel();
    let mut cmp = compare(&old, &new, &[]);
    let mut writer = csv_writer(&dir.path().join("x.csv"));
    assert!(cx::write_crawl_overview(&mut cmp, &[], writer.as_mut(), &mut ctx(&cancelled)).is_err());
}
