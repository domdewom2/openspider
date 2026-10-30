#[path = "reports_common.rs"]
mod common;

use common::Table;
use os_export::{save_report, Format};
use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

fn file(dir: &std::path::Path, mode: CrawlMode) -> CrawlFile {
    let now = Timestamp::from_db("2026-10-13T12:00:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.join("crawl.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(11).new_crawl_id(now),
            mode,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: String::new(),
            name: "N11".into(),
            config: CrawlConfig::default(),
            created_at: now,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!("../../../tests/fixtures/n11-exports/list.sql"))
        .unwrap();
    file
}

// spec: OUT-085 [M2]
// spec: OUT-081 [M2]
// spec: OUT-083
// spec: OUT-084
#[test]
fn list_chains_include_zero_and_one_hop_starts_in_upload_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), CrawlMode::List);
    for name in [
        "Redirects:Redirect Chains",
        "Redirects:Redirect & Canonical Chains",
    ] {
        let out = tempfile::tempdir().unwrap();
        let result = save_report(
            &file,
            &CrawlConfig::default(),
            name,
            out.path(),
            Format::Csv,
        )
        .unwrap();
        let table = Table::read(&result.files[0].path);
        assert_eq!(
            table.column("Address"),
            [
                "https://site.test/end",
                "https://site.test/start",
                "https://site.test/middle"
            ]
        );
        assert_eq!(table.column("Number of Redirects"), ["0", "2", "1"]);
        assert_eq!(table.get(&table.rows[0], "Chain Type"), "");
        assert_eq!(
            table.get(&table.rows[0], "Final Address"),
            "https://site.test/end"
        );
        assert_eq!(
            table.get(&table.rows[1], "Redirect URL 1"),
            "https://site.test/middle"
        );
    }
    // OUT-084 still requires a redirect: a directly uploaded error is not a redirect to an error.
    file.conn()
        .execute("UPDATE urls SET status_code=404 WHERE id=3", [])
        .unwrap();
    let out = tempfile::tempdir().unwrap();
    let result = save_report(
        &file,
        &CrawlConfig::default(),
        "Redirects:Redirects to Errors",
        out.path(),
        Format::Csv,
    )
    .unwrap();
    let table = Table::read(&result.files[0].path);
    assert_eq!(
        table.column("Address"),
        ["https://site.test/start", "https://site.test/middle"]
    );
}

// spec: OUT-081 [M2]
// spec: OUT-085 [M2]
#[test]
fn all_redirects_excludes_zero_hop_list_urls() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), CrawlMode::List);
    let out = tempfile::tempdir().unwrap();
    let result = save_report(
        &file,
        &CrawlConfig::default(),
        "Redirects:All Redirects",
        out.path(),
        Format::Csv,
    )
    .unwrap();
    let table = Table::read(&result.files[0].path);
    assert_eq!(
        table.column("Address"),
        ["https://site.test/start", "https://site.test/middle"]
    );
    assert_eq!(table.column("Number of Redirects"), ["2", "1"]);
}

// spec: OUT-080
// spec: ONPAGE-057
#[test]
fn overview_external_javascript_row_is_present_and_empty_without_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), CrawlMode::Spider);
    let cfg = CrawlConfig::default();
    let tabs = os_analyze::tabs::configured(&cfg);
    let overview = os_export::reports::overview::build(&file, &cfg, &tabs).unwrap();
    let rows: Vec<_> = overview
        .rows
        .iter()
        .filter(|r| r.section == "Response Codes" && r.item == "External Redirection (JavaScript)")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].urls, os_export::reports::overview::Cell::Empty);
    assert_eq!(rows[0].pct, None);
}

// spec: OUT-123
// spec: OUT-125
#[test]
fn sitemap_dates_and_frequency_use_fixed_url_crawl_time() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), CrawlMode::Spider);
    // A malformed and a missing header both fall back to the URL's crawl date.
    file.conn().execute_batch("UPDATE urls SET redirect_type=NULL,redirect_url=NULL,redirect_target_id=NULL,status_code=200,indexability='Indexable'; UPDATE urls SET last_modified='invalid' WHERE id=1; UPDATE urls SET last_modified='2026-10-12T12:00:00.000Z' WHERE id=2;").unwrap();
    let out = tempfile::tempdir().unwrap();
    let clock = os_model::FixedClock::new(
        Timestamp::from_db("2040-01-01T00:00:00.000Z").unwrap(),
        std::time::Duration::ZERO,
    );
    os_export::sitemap_gen::generate_sitemap_with(
        &file,
        &os_export::SitemapOptions::default(),
        None,
        &clock,
        out.path(),
    )
    .unwrap();
    let xml = std::fs::read_to_string(out.path().join("sitemap.xml")).unwrap();
    assert_eq!(xml.matches("<lastmod>2026-10-13</lastmod>").count(), 2);
    assert_eq!(xml.matches("<lastmod>2026-10-12</lastmod>").count(), 1);
    assert_eq!(xml.matches("<changefreq>daily</changefreq>").count(), 2);
    assert_eq!(xml.matches("<changefreq>monthly</changefreq>").count(), 1);
    assert!(!xml.contains("2040-01-01"));
}
