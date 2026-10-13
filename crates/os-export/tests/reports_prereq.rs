//! Tests der Report-Voraussetzungen (OUT-112) und des Report-Registers (Namen, Dateinamen, Auflösung).

#[path = "reports_common.rs"]
mod common;

use common::{base_config, Table};
use os_analyze::NameError;
use os_engine::testing::crawl_fixture_from;
use os_export::catalog::bulk_issues;
use os_export::catalog::reports::{self, ReportDef, ReportKind, ReportRequirement, REPORTS};
use os_export::naming;
use os_export::reports::prereq::requirement_warnings;
use os_export::{save_report, Format};

/// Ein Report mit Voraussetzung, wie ihn spätere Pakete anlegen (OUT-112: Cookie Summary).
const COOKIE_SUMMARY: ReportDef = ReportDef {
    name: "Cookie Summary",
    file_stem: "Cookie Summary",
    requires: &[ReportRequirement::Config {
        key: "extraction.cookies",
        what: "cookie extraction",
    }],
    kind: ReportKind::CrawlOverview,
};

// spec: OUT-112
#[test]
fn missing_extraction_names_the_report_and_the_requirement() {
    let cfg = base_config();
    let w = requirement_warnings(COOKIE_SUMMARY.name, COOKIE_SUMMARY.requires, &cfg, &[]);
    assert_eq!(
        w,
        vec!["report 'Cookie Summary' requires cookie extraction"]
    );
}

// spec: OUT-112
#[test]
fn a_satisfied_config_requirement_gives_no_warning() {
    let mut cfg = base_config();
    // `export.utf8_bom` ist standardmäßig an: dieselbe Prüfung, anderes Ergebnis.
    const NEEDS_BOM: &[ReportRequirement] = &[ReportRequirement::Config {
        key: "export.utf8_bom",
        what: "the BOM option",
    }];
    assert!(requirement_warnings("X", NEEDS_BOM, &cfg, &[]).is_empty());
    cfg.export.utf8_bom = false;
    assert_eq!(
        requirement_warnings("X", NEEDS_BOM, &cfg, &[]),
        vec!["report 'X' requires the BOM option"]
    );
}

// spec: OUT-112
#[test]
fn analysis_dependent_reports_name_the_empty_filters() {
    let cfg = base_config();
    const NEEDS_ANALYSIS: &[ReportRequirement] = &[ReportRequirement::CrawlAnalysis];
    // Nichts offen: kein Hinweis.
    assert!(requirement_warnings("Hreflang", NEEDS_ANALYSIS, &cfg, &[]).is_empty());
    let pending: Vec<String> = (1..=7).map(|i| format!("Hreflang:Filter {i}")).collect();
    let w = requirement_warnings("Hreflang", NEEDS_ANALYSIS, &cfg, &pending);
    assert_eq!(w.len(), 1);
    assert!(w[0].starts_with("report 'Hreflang' requires the crawl analysis: 7 filter(s)"));
    assert!(w[0].contains("Hreflang:Filter 1"));
    assert!(w[0].contains("and 2 more"));
    assert!(w[0].ends_with("run the crawl analysis first"));
}

// spec: OUT-112
#[test]
fn rendering_requirement_follows_the_rendering_mode() {
    let mut cfg = base_config();
    const NEEDS_RENDERING: &[ReportRequirement] = &[ReportRequirement::Rendering];
    assert_eq!(
        requirement_warnings("JavaScript", NEEDS_RENDERING, &cfg, &[]),
        vec!["report 'JavaScript' requires JavaScript rendering"]
    );
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    assert!(requirement_warnings("JavaScript", NEEDS_RENDERING, &cfg, &[]).is_empty());
}

// spec: OUT-112
#[tokio::test]
async fn warnings_never_stop_the_export() {
    let mut cfg = base_config();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from("out-redirects", "http://site.test/", cfg.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    // Der Crawl Overview braucht die Crawl-Analyse (Platzhalter-Tabs haben Nachlauf-Filter): Warnung, Datei da.
    let r = save_report(&fx.file, &cfg, "Crawl Overview", tmp.path(), Format::Csv).unwrap();
    assert!(!r.warnings.is_empty());
    let t = Table::read(&r.files[0].path);
    assert!(t.rows.len() > 100);
    // Reports ohne Voraussetzung melden nichts.
    let r = save_report(
        &fx.file,
        &cfg,
        "Redirects:All Redirects",
        tmp.path(),
        Format::Csv,
    )
    .unwrap();
    assert!(r.warnings.is_empty());
}

// spec: OUT-080, OUT-081, OUT-082
#[test]
fn the_register_lists_the_reports_with_canonical_names() {
    assert_eq!(
        reports::names(),
        [
            "Crawl Overview",
            "Redirects:All Redirects",
            "Redirects:Redirect Chains"
        ]
    );
    assert_eq!(REPORTS.len(), 3);
    // Dateinamen nach OUT-008 / OUT-194.
    let file_of = |n: &str| reports::resolve(n).unwrap().file_name("csv");
    assert_eq!(file_of("Crawl Overview"), "crawl_overview.csv");
    assert_eq!(file_of("Redirects:Redirect Chains"), "redirect_chains.csv");
    assert_eq!(file_of("Redirects:All Redirects"), "all_redirects.csv");
    assert_eq!(
        reports::resolve("Crawl Overview")
            .unwrap()
            .file_name("xlsx"),
        "crawl_overview.xlsx"
    );
}

// spec: OUT-080, OUT-081, OUT-082
#[test]
fn names_resolve_case_insensitively_and_tolerate_spaces_around_the_colon() {
    for input in [
        "redirects:redirect chains",
        "Redirects : Redirect Chains",
        "  REDIRECTS:REDIRECT   CHAINS ",
        "Reports:Redirects:Redirect Chains",
    ] {
        assert_eq!(
            reports::resolve(input).unwrap().name,
            "Redirects:Redirect Chains",
            "{input}"
        );
    }
    assert_eq!(
        reports::resolve("Reports:Crawl Overview").unwrap().name,
        "Crawl Overview"
    );
    assert_eq!(
        reports::resolve("crawl overview").unwrap().kind,
        ReportKind::CrawlOverview
    );
}

// spec: OUT-082
#[test]
fn unknown_names_come_with_suggestions() {
    let Err(NameError::Unknown { input, suggestions }) =
        reports::resolve("Redirects:Redirect Chain")
    else {
        panic!("Name darf nicht auflösbar sein");
    };
    assert_eq!(input, "Redirects:Redirect Chain");
    assert!(
        suggestions.contains(&"Redirects:Redirect Chains".to_string()),
        "{suggestions:?}"
    );
    let Err(NameError::Unknown { suggestions, .. }) = reports::resolve("Gibtsnicht") else {
        panic!("Name darf nicht auflösbar sein");
    };
    assert!(suggestions.is_empty());
}

// spec: OUT-080
#[test]
fn no_two_exports_of_this_package_share_a_file_name() {
    // Dateinamen der Reports und des Issues-Exports (Ordner `issues/` liegt ohnehin getrennt).
    let mut names: Vec<&str> = reports::file_stems();
    names.push(bulk_issues::BULK_NAME);
    assert!(naming::collisions(names.iter().copied()).is_empty());
    // Alle Namen sind unter Windows gültig und kurz.
    for stem in reports::file_stems() {
        let f = naming::file_name(stem, None, "csv");
        assert!(f.len() < 60, "{f}");
        assert!(f
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.')));
    }
}

// spec: OUT-080, OUT-081, OUT-082
#[tokio::test]
async fn every_registered_report_can_be_saved() {
    let mut cfg = base_config();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from("out-redirects", "http://site.test/", cfg.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    for def in REPORTS {
        let r = save_report(&fx.file, &cfg, def.name, tmp.path(), Format::Csv).unwrap();
        assert_eq!(r.files.len(), 1, "{}", def.name);
        assert_eq!(r.files[0].path, tmp.path().join(def.file_name("csv")));
        assert!(r.files[0].bytes > 0);
    }
    let err = save_report(&fx.file, &cfg, "Nope", tmp.path(), Format::Csv).unwrap_err();
    assert!(matches!(err, os_export::ExportError::Name(_)));
}
