//! B3-DIFF (Nutzerentscheidung 04.11.2026): Report „Crawl Changes“ als Datei (OUT-154) und Unterfilter-Exporte im
//! Vergleich (OUT-151) an der Fixture `out-b3-diff` (`v1` Previous → `v2` Current).
//! Golden: `tests/golden/out-b3-diff/crawl_changes.csv` (`UPDATE_GOLDEN=1` schreibt sie neu).

#[path = "../../os-analyze/tests/compare_common.rs"]
mod common;

use std::path::{Path, PathBuf};

use common::*;
use os_analyze::compare::CompareClass;
use os_analyze::views::ColumnSelection;
use os_export::compare::{self as cx, TabSelection};
use os_export::types::{ExportCtx, Format, SegmentScope};
use os_model::CancelToken;
use os_store::StorageProfile;

async fn b3(version: &str, start: &str, seed: u64) -> os_engine::testing::FixtureCrawl {
    crawl_at(
        &format!("out-b3-diff/{version}"),
        "http://www.site.test/",
        cfg(),
        start,
        seed,
        StorageProfile::Balanced,
    )
    .await
}

async fn pair() -> (
    os_engine::testing::FixtureCrawl,
    os_engine::testing::FixtureCrawl,
) {
    (
        b3("v1", "2026-01-01T00:00:00.000Z", 61).await,
        b3("v2", "2026-02-01T00:00:00.000Z", 62).await,
    )
}

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/out-b3-diff/crawl_changes.csv")
}

fn text(path: &Path) -> String {
    let raw = std::fs::read_to_string(path).unwrap();
    raw.strip_prefix('\u{feff}')
        .unwrap_or(&raw)
        .replace("\r\n", "\n")
}

fn rows(path: &Path) -> Vec<Vec<String>> {
    let body = text(path);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(body.as_bytes());
    rdr.records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

fn col(table: &[Vec<String>], name: &str) -> usize {
    table[0]
        .iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("Spalte {name} fehlt in {:?}", table[0]))
}

// spec: OUT-154
#[tokio::test(flavor = "multi_thread")]
async fn crawl_changes_report_matches_the_golden_file() {
    let (old, new) = pair().await;
    let mut c = compare(&new, &old, &[]);
    let dir = tempfile::tempdir().unwrap();
    let result = cx::save_report(
        &mut c,
        "Crawl Changes",
        dir.path(),
        Format::Csv,
        &CancelToken::new(),
    )
    .unwrap();
    let file = &result.files[0].path;
    assert_eq!(file.file_name().unwrap(), "crawl_changes.csv");
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let actual = text(file);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden().parent().unwrap()).unwrap();
        std::fs::write(golden(), &actual).unwrap();
    }
    assert_eq!(
        actual,
        text(&golden()),
        "UPDATE_GOLDEN=1 schreibt die Datei neu"
    );
    let table = rows(file);
    assert_eq!(table[0], os_analyze::compare::changes::HEADER);
    let change = col(&table, "Change");
    let address = col(&table, "Address");
    let gone: Vec<&str> = table[1..]
        .iter()
        .filter(|r| r[change] == "Page Gone")
        .map(|r| r[address].as_str())
        .collect();
    assert_eq!(
        gone,
        [www("/broken-now.html"), www("/gone.html")],
        "verschwundene Seiten"
    );
}

// spec: OUT-154
// spec: OUT-112
#[tokio::test(flavor = "multi_thread")]
async fn without_a_comparison_the_report_has_only_its_header_and_a_warning() {
    let new = b3("v2", "2026-02-01T00:00:00.000Z", 62).await;
    let cfg = new.file.config().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let result =
        os_export::reports::save_report(&new.file, &cfg, "Crawl Changes", dir.path(), Format::Csv)
            .unwrap();
    assert_eq!(rows(&result.files[0].path).len(), 1, "nur die Kopfzeile");
    assert_eq!(
        result.warnings,
        ["report 'Crawl Changes' requires a crawl comparison (--crawl-comparison or --compare-with-previous)"]
    );
    let def = os_export::catalog::reports::resolve("crawl changes").unwrap();
    let availability =
        os_export::reports::prereq::availability(def.name, def.requires, &new.file, &cfg);
    assert!(!availability.enabled);
}

struct OnlyRedirectsStartingWithR;

impl SegmentScope for OnlyRedirectsStartingWithR {
    fn population_sql(&self) -> String {
        "(u.address LIKE '%/r%' OR u.address LIKE '%/gone.html')".into()
    }
}

// spec: OUT-154
// spec: OUT-011
#[tokio::test(flavor = "multi_thread")]
async fn a_segment_keeps_rows_by_current_url_or_by_previous_url_without_partner() {
    let (old, new) = pair().await;
    let c = compare(&new, &old, &[]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("segment.csv");
    let mut writer =
        os_export::open_writer(&path, Format::Csv, "Sheet", &Default::default()).unwrap();
    let cancel = CancelToken::new();
    let mut ctx = ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    cx::write_crawl_changes(
        &c,
        Some(&OnlyRedirectsStartingWithR),
        writer.as_mut(),
        &mut ctx,
    )
    .unwrap();
    writer.finish().unwrap();
    let table = rows(&path);
    let address = col(&table, "Address");
    let mut got: Vec<&str> = table[1..].iter().map(|r| r[address].as_str()).collect();
    got.dedup();
    let mut sorted = got.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted,
        [www("/gone.html"), www("/r1"), www("/r1b"), www("/rt")]
    );
}

// spec: OUT-151
// spec: OUT-142
#[tokio::test(flavor = "multi_thread")]
async fn missing_rows_carry_previous_values_and_the_export_counts_satisfy_the_invariant() {
    let (old, new) = pair().await;
    let mut c = compare(&new, &old, &[]);
    let dir = tempfile::tempdir().unwrap();
    let export = |c: &mut os_analyze::compare::Comparison,
                  tab: &str,
                  filter: &str,
                  class: Option<CompareClass>|
     -> Vec<Vec<String>> {
        let name = format!("{tab}-{filter}-{class:?}").replace([' ', ':', '(', ')'], "_");
        let path = dir.path().join(format!("{name}.csv"));
        cx::export_tab(
            c,
            &TabSelection {
                tab: tab.into(),
                filter: filter.into(),
                class,
                columns: ColumnSelection::Default,
            },
            &path,
            Format::Csv,
        )
        .unwrap();
        rows(&path)
    };
    let missing = export(&mut c, "Internal", "HTML", Some(CompareClass::Missing));
    let (address, title, status) = (
        col(&missing, "Address"),
        col(&missing, "Title 1"),
        col(&missing, cx::STATUS_COLUMN),
    );
    assert_eq!(missing.len(), 2, "{missing:?}");
    assert_eq!(missing[1][address], www("/gone.html"));
    assert_eq!(missing[1][title], "Weg", "Titel aus dem Previous-Crawl");
    assert_eq!(missing[1][status], "Missing");

    let removed = export(
        &mut c,
        "Response Codes",
        "Client Error (4XX)",
        Some(CompareClass::Removed),
    );
    let address = col(&removed, "Address");
    assert_eq!(removed.len(), 2, "{removed:?}");
    assert_eq!(removed[1][address], www("/fixed.html"));
    assert_eq!(removed[1][col(&removed, cx::STATUS_COLUMN)], "Removed");

    // Invariante über die Exportdateien je Filter (OUT-151/142).
    for (tab, filter) in [
        ("Internal", "HTML"),
        ("Response Codes", "Client Error (4XX)"),
        ("Response Codes", "Redirection (3XX)"),
        ("Canonicals", "Canonicalised"),
    ] {
        let all = export(&mut c, tab, filter, None);
        let row = c.filter(tab, filter).unwrap();
        let (current, previous) = (row.current.unwrap() as i64, row.previous.unwrap() as i64);
        assert_eq!(all.len() as i64 - 1, current, "{tab}:{filter}");
        let status = col(&all, cx::STATUS_COLUMN);
        let unchanged = all[1..].iter().filter(|r| r[status] == "Unchanged").count() as i64;
        let mut n = |class| export(&mut c, tab, filter, Some(class)).len() as i64 - 1;
        let (added, new, removed, missing) = (
            n(CompareClass::Added),
            n(CompareClass::New),
            n(CompareClass::Removed),
            n(CompareClass::Missing),
        );
        assert_eq!(added + new + unchanged, current, "{tab}:{filter}");
        assert_eq!(
            current - previous,
            added + new - removed - missing,
            "{tab}:{filter}"
        );
    }
}
