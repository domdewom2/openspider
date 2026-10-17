//! Tests des Reports „Crawl Overview“ (OUT-080) gegen die Fixtures `out-issues` und `out-redirects`, gegen
//! synthetisch befüllte Crawl-Dateien und strukturell gegen die Platzhalter-Tabs (`tabs::all()`).
//!
//! Erwartungen stammen aus dem Fixture-Entwurf und den Akzeptanzkriterien der Spec; echte Tab-Zähler der
//! Platzhalter-Tabs liefern die Tab-Pakete (P20–P25); der Golden mit ihnen liegt unter
//! `tests/golden/out-issues/crawl_overview.csv` (M1-P26).

#[path = "reports_common.rs"]
mod common;

use std::path::Path;

use common::{base_config, issue_tabs, Table};
use os_analyze::{
    tabs, ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, TabDef,
};
use os_engine::testing::{crawl_fixture, crawl_fixture_from, FixtureCrawl};
use os_export::reports::save_report_in;
use os_export::{save_report, Format};
use os_model::time::{IdSource, SeededIds};
use os_model::{AnalysisStep, CrawlConfig, CrawlMode, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::golden::{assert_m1_overview_golden as assert_golden, normalize};

const OVERVIEW: &str = "Crawl Overview";

fn out_dir(tmp: &tempfile::TempDir) -> &Path {
    tmp.path()
}

async fn issues_crawl(cfg: &CrawlConfig) -> FixtureCrawl {
    crawl_fixture("out-issues", cfg.clone()).await
}

fn overview(
    fx: &FixtureCrawl,
    cfg: &CrawlConfig,
    tab_defs: &[TabDef],
) -> (tempfile::TempDir, Table, os_export::ReportResult) {
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report_in(
        tab_defs,
        &fx.file,
        cfg,
        OVERVIEW,
        out_dir(&tmp),
        Format::Csv,
    )
    .unwrap();
    let table = Table::read(&result.files[0].path);
    (tmp, table, result)
}

/// Zeile `(Section, Item)`.
fn item<'a>(t: &'a Table, section: &str, item: &str) -> &'a Vec<String> {
    t.rows
        .iter()
        .find(|r| t.get(r, "Section") == section && t.get(r, "Item") == item)
        .unwrap_or_else(|| panic!("keine Zeile {section:?} / {item:?}"))
}

fn urls(t: &Table, section: &str, it: &str) -> String {
    t.get(item(t, section, it), "URLs").to_string()
}

fn pct(t: &Table, section: &str, it: &str) -> String {
    t.get(item(t, section, it), "% of Total").to_string()
}

fn section_items<'a>(t: &'a Table, section: &str) -> Vec<&'a str> {
    t.rows
        .iter()
        .filter(|r| t.get(r, "Section") == section)
        .map(|r| t.get(r, "Item"))
        .collect()
}

/// Abschnitte in der Reihenfolge ihres ersten Auftretens.
fn sections(t: &Table) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in &t.rows {
        let s = t.get(r, "Section").to_string();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
    }
    seen
}

fn empty_crawl(dir: &Path, name: &str) -> CrawlFile {
    let created = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    CrawlFile::create(
        &dir.join(name),
        &CrawlInit {
            crawl_id: SeededIds::new(3).new_crawl_id(created),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".to_string()),
            list_source: None,
            project: "Projekt".to_string(),
            name: "Overview".to_string(),
            config: CrawlConfig::default(),
            created_at: created,
        },
        StorageProfile::Balanced,
    )
    .unwrap()
}

// spec: OUT-080
#[tokio::test]
async fn long_format_with_all_sections_in_order() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, result) = overview(&fx, &cfg, &issue_tabs());
    assert_eq!(t.header, ["Section", "Item", "URLs", "% of Total"]);
    assert_eq!(
        sections(&t),
        [
            "Summary",
            "Crawl Data \u{2013} Internal",
            "Crawl Data \u{2013} External",
            "Response Codes \u{2013} Internal",
            "Response Codes \u{2013} External",
            "Page Titles",
            "H1",
            "Response Codes",
            "Crawl Depth",
            "Issues",
        ]
    );
    assert_eq!(
        result.files[0].path.file_name().unwrap(),
        "crawl_overview.csv"
    );
}

// spec: OUT-080
#[tokio::test]
async fn summary_section_has_the_specified_items_in_order() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    let items = section_items(&t, "Summary");
    assert_eq!(
        &items[..13],
        [
            "Start URL",
            "Mode",
            "Crawl Start",
            "Crawl End",
            "Elapsed Time",
            "Total URLs Encountered",
            "Total URLs Crawled",
            "Total Internal Blocked by robots.txt",
            "Total External Blocked by robots.txt",
            "URLs Remaining In Queue",
            "Average Response Time (ms)",
            "Average URLs/s",
            "OpenSpider Version",
        ]
    );
    // [OS-Erweiterung] hinter den Angaben der Spec: Zähler der übersprungenen URLs.
    assert_eq!(
        &items[13..],
        [
            "Skipped By Exclude",
            "Skipped Not In Include",
            "Skipped By Depth Limit",
            "Skipped By Redirect Limit",
            "Skipped By Folder Depth",
            "Skipped By Query Parameters",
            "Skipped By URL Length"
        ]
    );
    assert_eq!(
        urls(&t, "Summary", "Start URL"),
        format!("{}/", fx.site.base_url())
    );
    assert_eq!(urls(&t, "Summary", "Mode"), "Spider");
    assert_eq!(
        urls(&t, "Summary", "OpenSpider Version"),
        env!("CARGO_PKG_VERSION")
    );
    // Zeitangaben: ISO 8601 mit Offset; die Dauer als HH:MM:SS.
    for it in ["Crawl Start", "Crawl End"] {
        let v = urls(&t, "Summary", it);
        assert!(v.len() >= 25 && v.as_bytes()[10] == b'T', "{it}: {v}");
    }
    let elapsed = urls(&t, "Summary", "Elapsed Time");
    assert_eq!(elapsed.len(), 8);
    assert_eq!(elapsed.matches(':').count(), 2);
    // Zusammenfassungszeilen tragen keinen Prozentwert.
    assert_eq!(pct(&t, "Summary", "Total URLs Encountered"), "");
}

// spec: OUT-080
#[tokio::test]
async fn summary_counts_and_the_encountered_crawled_invariant() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    // Fixture-Entwurf: 11 interne URLs in den Tabellen (Startseite, 7 Seiten, 404, 404, 500, gesperrte Seite;
    // die robots.txt zählt nicht, K-54) und 2 externe; eine interne URL ist durch robots.txt gesperrt.
    let n = |it: &str| -> u64 { urls(&t, "Summary", it).parse().unwrap() };
    assert_eq!(n("Total URLs Encountered"), 13);
    assert_eq!(n("Total URLs Crawled"), 12);
    assert_eq!(n("Total Internal Blocked by robots.txt"), 1);
    assert_eq!(n("Total External Blocked by robots.txt"), 0);
    assert_eq!(n("URLs Remaining In Queue"), 0);
    assert!(n("Total URLs Encountered") >= n("Total URLs Crawled"));
    // Encountered − Crawled = blockiert + nicht gecrawlt + Queue.
    let not_crawled: u64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE in_tables = 1 AND state = 'discovered'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        n("Total URLs Encountered") - n("Total URLs Crawled"),
        n("Total Internal Blocked by robots.txt")
            + n("Total External Blocked by robots.txt")
            + not_crawled
            + n("URLs Remaining In Queue")
    );
    // Der Durchschnitt der Antwortzeit ist eine Zahl mit zwei Nachkommastellen.
    let avg = urls(&t, "Summary", "Average Response Time (ms)");
    assert!(
        avg.parse::<f64>().is_ok() && avg.split('.').nth(1).map(str::len) == Some(2),
        "{avg}"
    );
}

// spec: OUT-080
#[tokio::test]
async fn crawl_data_counts_per_content_class_with_two_decimal_percentages() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    let internal = "Crawl Data \u{2013} Internal";
    let external = "Crawl Data \u{2013} External";
    assert_eq!(
        section_items(&t, internal),
        [
            "All",
            "HTML",
            "JavaScript",
            "CSS",
            "Images",
            "PDF",
            "Plugins",
            "Media",
            "Fonts",
            "XML",
            "Other",
            "Unknown"
        ]
    );
    assert_eq!(section_items(&t, external), section_items(&t, internal));
    // 11 interne URLs: 10 HTML (inkl. der Fehlerseiten), die gesperrte Seite Unknown; die robots.txt steht in
    // keiner Zeile (K-54).
    assert_eq!(urls(&t, internal, "All"), "11");
    assert_eq!(pct(&t, internal, "All"), "100.00");
    assert_eq!(urls(&t, internal, "HTML"), "10");
    assert_eq!(pct(&t, internal, "HTML"), "90.91");
    assert_eq!(urls(&t, internal, "Other"), "0");
    assert_eq!(pct(&t, internal, "Other"), "0.00");
    assert_eq!(urls(&t, internal, "Unknown"), "1");
    assert_eq!(urls(&t, internal, "CSS"), "0");
    assert_eq!(pct(&t, internal, "CSS"), "0.00");
    // Summe der Klassen = All.
    let sum: u64 = section_items(&t, internal)
        .iter()
        .skip(1)
        .map(|i| urls(&t, internal, i).parse::<u64>().unwrap())
        .sum();
    assert_eq!(sum, 11);
    // Extern: die beiden Seiten auf localhost.
    assert_eq!(urls(&t, external, "All"), "2");
    assert_eq!(urls(&t, external, "HTML"), "2");
    assert_eq!(pct(&t, external, "HTML"), "100.00");
}

// spec: OUT-080
#[tokio::test]
async fn response_code_sections_split_internal_and_external_and_all_errors_adds_up() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    let internal = "Response Codes \u{2013} Internal";
    let external = "Response Codes \u{2013} External";
    let expected_items = [
        "Blocked by Robots.txt",
        "Blocked Resource",
        "No Response",
        "Success (2xx)",
        "Redirection (3xx)",
        "Redirection (JavaScript)",
        "Redirection (Meta Refresh)",
        "Redirection (HTTP Refresh)",
        "Redirect Chain",
        "Redirect Loop",
        "Client Error (4xx)",
        "Server Error (5xx)",
        "All Errors",
    ];
    assert_eq!(section_items(&t, internal), expected_items);
    assert_eq!(section_items(&t, external), expected_items);

    // Intern (Grundgesamtheit 11 URLs mit Antwort oder Sperre): 1 gesperrt, 7 × 2xx, 2 × 404, 1 × 500.
    assert_eq!(urls(&t, internal, "Blocked by Robots.txt"), "1");
    assert_eq!(urls(&t, internal, "No Response"), "0");
    assert_eq!(urls(&t, internal, "Success (2xx)"), "7");
    assert_eq!(pct(&t, internal, "Success (2xx)"), "63.64");
    assert_eq!(urls(&t, internal, "Client Error (4xx)"), "2");
    assert_eq!(urls(&t, internal, "Server Error (5xx)"), "1");
    assert_eq!(urls(&t, internal, "All Errors"), "3");
    assert_eq!(pct(&t, internal, "All Errors"), "27.27");
    // Extern: 1 × 200, 1 × 404.
    assert_eq!(urls(&t, external, "Success (2xx)"), "1");
    assert_eq!(urls(&t, external, "Client Error (4xx)"), "1");
    assert_eq!(urls(&t, external, "All Errors"), "1");
    assert_eq!(pct(&t, external, "Client Error (4xx)"), "50.00");

    // Nicht befüllbare Zeilen haben ein leeres Feld statt 0: Blocked Resource und JavaScript-Weiterleitung
    // brauchen Rendering.
    for section in [internal, external] {
        for it in ["Blocked Resource", "Redirection (JavaScript)"] {
            assert_eq!(urls(&t, section, it), "", "{section} {it}");
            assert_eq!(pct(&t, section, it), "");
        }
    }
}

// spec: OUT-080
#[tokio::test]
async fn all_errors_is_no_response_plus_4xx_plus_5xx_per_scope() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    // Zwei Antworten künstlich ohne Antwort stellen (keine Fixture liefert deterministisch „No Response“).
    fx.file
        .conn()
        .execute_batch(
            "UPDATE urls SET status_code = 0, status = 'Connection Timeout', no_response_reason = 'connection_timeout' \
             WHERE address LIKE '%/ok.html' OR address LIKE '%/ext-ok.html';",
        )
        .unwrap();
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    for (section, no_resp, client, server) in [
        ("Response Codes \u{2013} Internal", 1, 2, 1),
        ("Response Codes \u{2013} External", 1, 1, 0),
    ] {
        let n = |it: &str| -> u64 { urls(&t, section, it).parse().unwrap() };
        assert_eq!(n("No Response"), no_resp, "{section}");
        assert_eq!(n("Client Error (4xx)"), client);
        assert_eq!(n("Server Error (5xx)"), server);
        assert_eq!(
            n("All Errors"),
            n("No Response") + n("Client Error (4xx)") + n("Server Error (5xx)"),
            "{section}"
        );
    }
}

// spec: OUT-080
#[tokio::test]
async fn crawl_depth_rows_zero_to_ten_and_ten_plus() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    let rows: Vec<&str> = section_items(&t, "Crawl Depth");
    assert_eq!(
        rows,
        ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "10+"]
    );
    // Zeile 0 = die Start-URL; die übrigen internen HTML-Seiten mit 2xx liegen in Tiefe 1 (K-58: die 404-/500-
    // Seiten und die gesperrte Seite zählen nicht).
    assert_eq!(urls(&t, "Crawl Depth", "0"), "1");
    assert_eq!(urls(&t, "Crawl Depth", "1"), "6");
    assert_eq!(urls(&t, "Crawl Depth", "2"), "0");
    assert_eq!(pct(&t, "Crawl Depth", "0"), "14.29");
    assert_eq!(pct(&t, "Crawl Depth", "1"), "85.71");
    // Summe über alle Zeilen = Zahl interner HTML-Seiten mit 2xx und bekannter Tiefe.
    let sum: u64 = rows
        .iter()
        .map(|d| urls(&t, "Crawl Depth", d).parse::<u64>().unwrap())
        .sum();
    assert_eq!(sum, 7);

    // Synthetisch: eine Kette bis Tiefe 12 → „10“ und „10+“ getrennt.
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "depth.ospider");
    file.conn()
        .execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < 12) \
             INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                crawl_depth, folder_depth, url_length, status_code, content_class) \
             SELECT i + 1, 'https://site.test/d' || i, 'https://site.test/d' || i, 'https://site.test', \
                1, 1, 'done', 'link', i, 0, 30, 200, 'HTML' FROM n;",
        )
        .unwrap();
    let out = tempfile::tempdir().unwrap();
    let result = save_report_in(
        &issue_tabs(),
        &file,
        &cfg,
        OVERVIEW,
        out.path(),
        Format::Csv,
    )
    .unwrap();
    let t = Table::read(&result.files[0].path);
    assert_eq!(urls(&t, "Crawl Depth", "9"), "1");
    assert_eq!(urls(&t, "Crawl Depth", "10"), "1");
    assert_eq!(urls(&t, "Crawl Depth", "10+"), "2");
    assert_eq!(pct(&t, "Crawl Depth", "10+"), "15.38");
}

// spec: OUT-080
#[test]
fn empty_denominators_leave_percentages_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "empty.ospider");
    let fx_cfg = base_config();
    let out = tempfile::tempdir().unwrap();
    let result = save_report_in(
        &issue_tabs(),
        &file,
        &fx_cfg,
        OVERVIEW,
        out.path(),
        Format::Csv,
    )
    .unwrap();
    let t = Table::read(&result.files[0].path);
    // Division durch 0 → leer, nie „NaN“ oder „0.00“ aus 0/0.
    for section in [
        "Crawl Data \u{2013} Internal",
        "Response Codes \u{2013} Internal",
        "Page Titles",
        "Crawl Depth",
    ] {
        for it in section_items(&t, section) {
            assert_eq!(pct(&t, section, it), "", "{section} / {it}");
        }
    }
    // Die Zähler selbst sind 0, nicht leer.
    assert_eq!(urls(&t, "Crawl Data \u{2013} Internal", "All"), "0");
    assert_eq!(urls(&t, "Summary", "Total URLs Encountered"), "0");
    // Ohne Dauer und ohne Antworten bleiben Durchschnitte leer.
    assert_eq!(urls(&t, "Summary", "Average Response Time (ms)"), "");
    assert_eq!(urls(&t, "Summary", "Average URLs/s"), "");
}

// spec: OUT-080
#[tokio::test]
async fn every_tab_filter_appears_with_count_and_share_of_all() {
    let mut cfg = base_config();
    cfg.title.max_chars = 60;
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    assert_eq!(
        section_items(&t, "Page Titles"),
        ["All", "Missing", "Duplicate", "Over 60 Characters"]
    );
    // Grundmenge „All“ des Tabs: 7 interne 2xx-HTML-Seiten.
    assert_eq!(urls(&t, "Page Titles", "All"), "7");
    assert_eq!(pct(&t, "Page Titles", "All"), "100.00");
    assert_eq!(urls(&t, "Page Titles", "Missing"), "1");
    assert_eq!(pct(&t, "Page Titles", "Missing"), "14.29");
    assert_eq!(urls(&t, "Page Titles", "Duplicate"), "2");
    assert_eq!(pct(&t, "Page Titles", "Duplicate"), "28.57");
    assert_eq!(urls(&t, "Page Titles", "Over 60 Characters"), "1");
    assert_eq!(urls(&t, "H1", "Missing"), "1");

    // Schwellen stehen im Namen und folgen der Konfiguration.
    // Titellängen: 29, 40, 31, 31, 37, 92 Zeichen → über 35 sind drei.
    let mut cfg35 = cfg.clone();
    cfg35.title.max_chars = 35;
    let (_tmp2, t2, _) = overview(&fx, &cfg35, &issue_tabs());
    assert_eq!(
        section_items(&t2, "Page Titles"),
        ["All", "Missing", "Duplicate", "Over 35 Characters"]
    );
    assert_eq!(urls(&t2, "Page Titles", "Over 35 Characters"), "3");
}

fn probe_col() -> ColumnDef {
    ColumnDef {
        key: "Address",
        sql: ColumnSql::Expr("u.address"),
        kind: ColumnKind::Url,
        default_visible: true,
        extension: false,
        ..ColumnDef::META
    }
}

fn probe_filter(id: &'static str, kind: FilterKind, requires: Vec<Requirement>) -> FilterDef {
    FilterDef {
        id,
        predicate: "1",
        kind,
        requires,
        ignore_non_indexable: false,
        default_columns: &[],
    }
}

// spec: OUT-080
#[tokio::test]
async fn filters_that_cannot_be_filled_have_an_empty_urls_cell_not_zero() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tab = TabDef {
        id: "Probe",
        population: "u.is_internal = 1",
        columns: vec![probe_col()],
        filters: vec![
            probe_filter("All", FilterKind::Live, vec![]),
            probe_filter(
                "Needs Config",
                FilterKind::Live,
                vec![Requirement::Config("duplicates.near_enabled")],
            ),
            probe_filter(
                "Needs Analysis",
                FilterKind::PostCrawl(AnalysisStep::Canonicals),
                vec![],
            ),
            probe_filter(
                "Needs Rendering",
                FilterKind::Live,
                vec![Requirement::Rendering],
            ),
        ],
        default_sort: vec![],
        params: |_| Vec::new(),
        hidden_population: None,
    };
    let (_tmp, t, result) = overview(&fx, &cfg, &[tab]);
    assert_eq!(urls(&t, "Probe", "All"), "12");
    for it in ["Needs Config", "Needs Analysis", "Needs Rendering"] {
        assert_eq!(urls(&t, "Probe", it), "", "{it}");
        assert_eq!(pct(&t, "Probe", it), "", "{it}");
    }
    // OUT-112: die Analyse-Abhängigkeit wird als Hinweis genannt, der Report entsteht trotzdem.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("crawl analysis") && w.contains("Probe:Needs Analysis")),
        "{:?}",
        result.warnings
    );
}

// spec: OUT-080
#[tokio::test]
async fn issues_section_counts_issues_by_type_and_priority() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    assert_eq!(
        section_items(&t, "Issues"),
        [
            "Issues (High)",
            "Issues (Medium)",
            "Issues (Low)",
            "Warnings (High)",
            "Warnings (Medium)",
            "Warnings (Low)",
            "Opportunities (High)",
            "Opportunities (Medium)",
            "Opportunities (Low)",
        ]
    );
    // Katalog: Titel fehlt (Issue/High), 4xx intern (Issue/High), H1 fehlt (Issue/Medium), Titel doppelt und
    // zu lang (Opportunity/Medium).
    let n = |it: &str| urls(&t, "Issues", it);
    assert_eq!(n("Issues (High)"), "2");
    assert_eq!(n("Issues (Medium)"), "1");
    assert_eq!(n("Issues (Low)"), "0");
    assert_eq!(n("Warnings (High)"), "0");
    assert_eq!(n("Opportunities (Medium)"), "2");
    assert_eq!(n("Opportunities (High)"), "0");
}

// spec: OUT-080
#[tokio::test]
async fn redirect_rows_count_chains_loops_and_refresh_pages() {
    let mut cfg = base_config();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from("out-redirects", "http://site.test/", cfg.clone()).await;
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    let internal = "Response Codes \u{2013} Internal";
    // Meta Refresh: /meta, /meta-delay, /mixmeta; HTTP Refresh: /hr (Header mit Ziel).
    assert_eq!(urls(&t, internal, "Redirection (Meta Refresh)"), "3");
    assert_eq!(urls(&t, internal, "Redirection (HTTP Refresh)"), "1");
    // Schleifen: /f, /g und /s (alle Mitglieder).
    assert_eq!(urls(&t, internal, "Redirect Loop"), "3");
    // Ketten (≥ 2 Weiterleitungen, keine Schleife): /c, /l1 … /l4, /mix, /hw1, /x1, /long1 … /long5.
    assert_eq!(urls(&t, internal, "Redirect Chain"), "13");
    // 3xx: alle URLs mit Status 3xx.
    let three_xx: u64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE in_tables = 1 AND is_internal = 1 AND state = 'done' \
             AND status_code BETWEEN 300 AND 399",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        urls(&t, internal, "Redirection (3xx)"),
        three_xx.to_string()
    );
    assert!(three_xx > 20);
}

// spec: OUT-080, CRAWL-110
#[tokio::test]
async fn skipped_url_counters_come_from_the_crawl_file() {
    // Die Namen entsprechen den Konstanten der Engine.
    assert_eq!(
        os_export::reports::overview::COUNTER_EXCLUDED,
        os_engine::frontier::COUNTER_EXCLUDED
    );
    assert_eq!(
        os_export::reports::overview::COUNTER_NOT_INCLUDED,
        os_engine::frontier::COUNTER_NOT_INCLUDED
    );
    assert_eq!(
        os_export::reports::overview::COUNTER_DEPTH_LIMIT,
        os_engine::frontier::COUNTER_DEPTH_LIMIT
    );
    assert_eq!(
        os_export::reports::overview::COUNTER_REDIRECT_LIMIT,
        os_engine::frontier::COUNTER_REDIRECT_LIMIT
    );

    let mut cfg = base_config();
    cfg.scope.exclude = vec![".*/ok\\.html".to_string()];
    let fx = issues_crawl(&cfg).await;
    let stored: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COALESCE((SELECT value FROM crawl_counters WHERE key = 'excluded_skipped'), 0)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(stored >= 1, "die Exclude-Regel muss getroffen haben");
    let (_tmp, t, _) = overview(&fx, &cfg, &issue_tabs());
    assert_eq!(
        urls(&t, "Summary", "Skipped By Exclude"),
        stored.to_string()
    );
    assert_eq!(urls(&t, "Summary", "Skipped By Depth Limit"), "0");

    // Tiefenlimit 0: alle verlinkten Seiten werden übersprungen und gezählt.
    let mut cfg0 = base_config();
    cfg0.limits.max_depth = Some(0);
    let fx0 = issues_crawl(&cfg0).await;
    let (_t2, t0, _) = overview(&fx0, &cfg0, &issue_tabs());
    let skipped: u64 = urls(&t0, "Summary", "Skipped By Depth Limit")
        .parse()
        .unwrap();
    assert!(skipped >= 10, "{skipped}");
}

// spec: OUT-080
#[tokio::test]
async fn placeholder_tabs_give_every_tab_with_all_filters_in_ui_order() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(&fx.file, &cfg, OVERVIEW, tmp.path(), Format::Csv).unwrap();
    let t = Table::read(&result.files[0].path);

    // Abschnitt 5 = die Tabs aus tabs::all() in UI-Reihenfolge, je Tab alle Filter in Anzeigereihenfolge. Stub-Tabs und
    // Stub-Filter von M2 (M2-F02) tragen noch keine Zähler und erscheinen nicht.
    let all: Vec<_> = tabs::all().into_iter().filter(|t| !t.is_stub()).collect();
    let tab_ids: Vec<&str> = all.iter().map(|t| t.id).collect();
    let secs = sections(&t);
    let first = secs.iter().position(|s| s == tab_ids[0]).unwrap();
    assert_eq!(
        secs[first..first + tab_ids.len()],
        tab_ids,
        "Tab-Abschnitte"
    );
    assert_eq!(secs[first - 1], "Response Codes \u{2013} External");
    assert_eq!(secs[first + tab_ids.len()], "Crawl Depth");
    for tab in &all {
        let want: Vec<String> = tab
            .filters
            .iter()
            .filter(|f| !f.is_stub())
            .map(|f| os_analyze::names::label(f.id, &cfg))
            .collect();
        let got: Vec<&str> = section_items(&t, tab.id);
        assert_eq!(
            got,
            want.iter().map(String::as_str).collect::<Vec<_>>(),
            "{}",
            tab.id
        );
    }
    // Nachlauf-Filter ohne Analyse haben keine Zahl.
    assert_eq!(urls(&t, "Canonicals", "Non-Indexable Canonical"), "");
}

/// Tab- und Filterkennungen aus `docs/plan/M1-filters.toml` (Zeilen `id = "…"` unter `[[tab]]` und
/// `{ id = "…", …}` in den Filterlisten).
fn filters_toml() -> Vec<(String, Vec<String>)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plan/M1-filters.toml");
    let text = std::fs::read_to_string(path).unwrap();
    let quoted = |s: &str| -> String {
        let start = s.find('"').unwrap() + 1;
        let end = start + s[start..].find('"').unwrap();
        s[start..end].to_string()
    };
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut expect_id = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[tab]]" {
            expect_id = true;
        } else if expect_id && line.starts_with("id = ") {
            out.push((quoted(line), Vec::new()));
            expect_id = false;
        } else if line.starts_with("{ id = ") {
            if let Some(last) = out.last_mut() {
                last.1.push(quoted(line));
            }
        }
    }
    out
}

// spec: OUT-080
#[tokio::test]
async fn overview_names_come_from_the_canonical_filter_list() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(&fx.file, &cfg, OVERVIEW, tmp.path(), Format::Csv).unwrap();
    let t = Table::read(&result.files[0].path);
    let canonical = filters_toml();
    assert!(canonical.len() >= 10);
    for (tab, filters) in canonical {
        let want: Vec<String> = filters
            .iter()
            .map(|f| os_analyze::names::label(f, &cfg))
            .collect();
        let got: Vec<String> = section_items(&t, &tab)
            .iter()
            .map(|s| (*s).to_string())
            .filter(|label| want.contains(label))
            .collect();
        assert_eq!(got, want, "Tab {tab}");
    }
}

// spec: OUT-080, OUT-112
#[tokio::test]
async fn missing_crawl_analysis_is_a_warning_and_never_aborts() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(&fx.file, &cfg, OVERVIEW, tmp.path(), Format::Csv).unwrap();
    assert_eq!(result.files.len(), 1);
    let w: Vec<&String> = result
        .warnings
        .iter()
        .filter(|w| w.starts_with("report 'Crawl Overview' requires the crawl analysis"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", result.warnings);
}

// spec: OUT-080
#[tokio::test]
async fn xlsx_holds_numbers_as_numbers_and_the_same_rows() {
    use calamine::{open_workbook, Data, Reader, Xlsx};
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report_in(
        &issue_tabs(),
        &fx.file,
        &cfg,
        OVERVIEW,
        tmp.path(),
        Format::Xlsx,
    )
    .unwrap();
    assert_eq!(
        result.files[0].path.file_name().unwrap(),
        "crawl_overview.xlsx"
    );
    let (_t, csv, _) = overview(&fx, &cfg, &issue_tabs());
    let mut wb: Xlsx<_> = open_workbook(&result.files[0].path).unwrap();
    let range = wb.worksheet_range("Crawl Overview").unwrap();
    assert_eq!(range.height(), csv.rows.len() + 1);
    // Zeile „Crawl Data – Internal / HTML“: Zahl und Prozentwert als Zahlenzellen.
    let row_idx = csv
        .rows
        .iter()
        .position(|r| {
            csv.get(r, "Section") == "Crawl Data \u{2013} Internal" && csv.get(r, "Item") == "HTML"
        })
        .unwrap()
        + 1;
    assert!(matches!(
        range.get((row_idx, 2)).unwrap(),
        Data::Float(_) | Data::Int(_)
    ));
    assert!(matches!(range.get((row_idx, 3)).unwrap(), Data::Float(_)));
}

/// Die Durchschnitte hängen an der Laufzeit des Crawls und werden maskiert.
fn mask_averages(normalized: &str) -> String {
    normalized
        .lines()
        .map(|l| {
            if l.starts_with("Summary,Average Response Time (ms),") {
                "Summary,Average Response Time (ms),<N>,".to_string()
            } else if l.starts_with("Summary,Average URLs/s,") {
                "Summary,Average URLs/s,<N>,".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

// spec: OUT-080
#[tokio::test]
async fn report_matches_the_regression_golden_with_test_tabs() {
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let (_tmp, _t, result) = overview(&fx, &cfg, &issue_tabs());
    let text = std::fs::read_to_string(&result.files[0].path).unwrap();
    let bases = fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let masked = mask_averages(&normalize(&text, &bases));
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-issues/test-tabs/crawl_overview.csv"
        ),
        &masked,
    );
}

// spec: OUT-080
#[tokio::test]
async fn crawl_overview_golden_with_the_real_tab_counters() {
    // Golden `tests/golden/out-issues/crawl_overview.csv` (M1-P26) mit den echten Zählern der Tabs P20–P25.
    let cfg = base_config();
    let fx = issues_crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(&fx.file, &cfg, OVERVIEW, tmp.path(), Format::Csv).unwrap();
    let text = std::fs::read_to_string(&result.files[0].path).unwrap();
    let mut bases = fx.site.golden_bases();
    for port in [fx.site.port(), fx.site.port2()] {
        bases.push(format!("http://localhost:{port}"));
    }
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-issues/crawl_overview.csv"
        ),
        &mask_averages(&normalize(&text, &bases)),
    );
}

// spec: OUT-080
#[test]
fn rows_convert_to_typed_cells() {
    use os_export::reports::overview::{Cell, OverviewRow};
    let row = OverviewRow {
        section: "S".into(),
        item: "I".into(),
        urls: Cell::Int(3),
        pct: Some(12.5),
    };
    let (kinds, values) = row.to_values();
    assert_eq!(kinds[2], ColumnKind::Int);
    assert_eq!(values[2], Value::Int(3));
    assert_eq!(values[3], Value::Float(12.5));
    let empty = OverviewRow {
        urls: Cell::Empty,
        pct: None,
        ..row
    };
    let (_, values) = empty.to_values();
    assert_eq!(values[2], Value::Null);
    assert_eq!(values[3], Value::Null);
}
