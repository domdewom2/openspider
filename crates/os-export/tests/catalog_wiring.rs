//! Verdrahtung des Katalogs (OUT-008, OUT-063, OUT-080 ff.): `catalog::resolve/list` kennen Tabs, Bulk-Exporte
//! (Links und `Issues:All Issues`) und Reports; `pipeline` führt Issues-Bulk und Reports aus.

#[path = "reports_common.rs"]
mod common;

use std::path::Path;

use common::{base_config, Table};
use os_analyze::NameError;
use os_engine::testing::crawl_fixture;
use os_export::catalog::reports::{self, REPORTS};
use os_export::catalog::{bulk_issues, list, resolve};
use os_export::pipeline::{export_bulk, export_report};
use os_export::{naming, ExportKind, ExportRef, Format};

fn suggestions(err: NameError) -> Vec<String> {
    match err {
        NameError::Unknown { suggestions, .. } => suggestions,
    }
}

// spec: OUT-008
#[test]
fn every_listed_name_resolves() {
    for kind in [ExportKind::Tab, ExportKind::Bulk, ExportKind::Report] {
        let names = list(kind);
        assert!(!names.is_empty(), "{kind:?}: keine Namen");
        for name in &names {
            let resolved = resolve(kind, name)
                .unwrap_or_else(|e| panic!("{kind:?}: {name:?} löst sich nicht auf: {e}"));
            match (kind, resolved) {
                (ExportKind::Tab, ExportRef::Tab { .. }) => {}
                (ExportKind::Bulk, ExportRef::Bulk(canonical)) => {
                    assert_eq!(&canonical, name, "Bulk-Name kanonisch");
                }
                (ExportKind::Report, ExportRef::Report(canonical)) => {
                    assert_eq!(&canonical, name, "Report-Name kanonisch");
                }
                (k, r) => panic!("{k:?}: {name:?} → {r:?}"),
            }
        }
    }
}

// spec: OUT-008
#[test]
fn issues_bulk_and_all_reports_are_listed() {
    let bulk = list(ExportKind::Bulk);
    assert!(bulk.iter().any(|n| n == "Issues:All Issues"), "{bulk:?}");
    assert!(bulk.iter().any(|n| n == "Links:All Inlinks"), "{bulk:?}");
    let report_names = list(ExportKind::Report);
    // Die Reports von M1 stehen vorn, dahinter die Tabellen-Einträge von M2 (Stubs, M2-F02).
    let m1 = reports::names();
    assert_eq!(report_names[..m1.len()], m1[..]);
    assert_eq!(m1.len(), REPORTS.len());
    assert_eq!(
        report_names.len(),
        REPORTS.len() + os_export::reports::report_tables().len()
    );
    for expected in [
        "Crawl Overview",
        "Redirects:All Redirects",
        "Redirects:Redirect Chains",
    ] {
        assert!(report_names.iter().any(|n| n == expected), "{expected}");
    }
}

// spec: OUT-008
#[test]
fn names_resolve_case_insensitively() {
    assert_eq!(
        resolve(ExportKind::Bulk, " issues : ALL   issues ").unwrap(),
        ExportRef::Bulk("Issues:All Issues".to_string())
    );
    assert_eq!(
        resolve(ExportKind::Report, "redirects:redirect chains").unwrap(),
        ExportRef::Report("Redirects:Redirect Chains".to_string())
    );
    assert_eq!(
        resolve(ExportKind::Report, "Reports:Crawl Overview").unwrap(),
        ExportRef::Report("Crawl Overview".to_string())
    );
    assert_eq!(
        resolve(ExportKind::Bulk, "links:all inlinks").unwrap(),
        ExportRef::Bulk("Links:All Inlinks".to_string())
    );
}

// spec: OUT-008
#[test]
fn no_two_exports_share_a_file_name() {
    // Tab-, Bulk- und Report-Namen laufen gemeinsam durch den Kollisionstest; Reports tragen ihren
    // Dateinamen-Stamm (`def.file_stem`), nicht den kanonischen Namen.
    let mut all: Vec<String> = list(ExportKind::Tab);
    all.extend(list(ExportKind::Bulk));
    all.extend(reports::file_stems().into_iter().map(str::to_string));
    let collisions = naming::collisions(all.iter().map(String::as_str));
    assert!(collisions.is_empty(), "Kollisionen: {collisions:?}");
    // Der Issues-Bulk legt zusätzlich einen eigenen Ordner an und kollidiert deshalb auch nicht mit
    // Dateien im Zielordner.
    assert_ne!(bulk_issues::DIR_NAME, "");
}

// spec: OUT-008
#[test]
fn typos_get_suggestions_from_all_names_of_the_kind() {
    let s = suggestions(resolve(ExportKind::Bulk, "Issues:All Issuse").unwrap_err());
    assert_eq!(s.first().map(String::as_str), Some("Issues:All Issues"));
    let s = suggestions(resolve(ExportKind::Bulk, "Links:All Inlnks").unwrap_err());
    assert_eq!(s.first().map(String::as_str), Some("Links:All Inlinks"));
    let s = suggestions(resolve(ExportKind::Report, "Crawl Overvew").unwrap_err());
    assert_eq!(s.first().map(String::as_str), Some("Crawl Overview"));
    let s = suggestions(resolve(ExportKind::Report, "Redirects:Redirect Chain").unwrap_err());
    assert_eq!(
        s.first().map(String::as_str),
        Some("Redirects:Redirect Chains")
    );
    // Weit entfernte Eingaben liefern keine Vorschläge, aber einen Fehler.
    let s = suggestions(resolve(ExportKind::Report, "zzzzzzzzzzzzzzzzzzzz").unwrap_err());
    assert!(s.is_empty(), "{s:?}");
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

// spec: OUT-063
#[tokio::test]
async fn bulk_export_runs_the_issues_export_with_inlinks_files() {
    let cfg = base_config();
    let fx = crawl_fixture("out-issues", cfg.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    let files = export_bulk(&fx.file, &cfg, "issues:all issues", tmp.path(), Format::Csv).unwrap();

    let dir = tmp.path().join("issues");
    assert!(dir.is_dir());
    let names = names_in(&dir);
    assert!(
        names.iter().any(|n| n == "issues_overview.csv"),
        "{names:?}"
    );
    assert_eq!(
        files.last().and_then(|f| f.path.file_name()).unwrap(),
        "issues_overview.csv"
    );
    // Linkbezogene Issues (Response Codes) schreiben ihre Inlinks-Datei über `export_detail`, ohne Hinweis
    // „inlinks file skipped“.
    let inlinks: Vec<_> = files
        .iter()
        .filter(|f| {
            f.path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with("_inlinks.csv"))
        })
        .collect();
    assert!(!inlinks.is_empty(), "keine Inlinks-Datei: {names:?}");
    for info in &inlinks {
        assert!(info.path.is_file());
        assert!(info.rows > 0, "{} ohne Zeilen", info.path.display());
        let t = Table::read(&info.path);
        assert_eq!(t.header.first().map(String::as_str), Some("Type"));
        assert_eq!(t.header.len(), 15, "Standard-Linkformat");
    }
    let warnings: Vec<&String> = files.iter().flat_map(|f| &f.warnings).collect();
    assert!(
        warnings.iter().all(|w| !w.contains("inlinks file skipped")),
        "{warnings:?}"
    );
}

// spec: OUT-080
#[tokio::test]
async fn report_runs_through_the_pipeline() {
    let cfg = base_config();
    let fx = crawl_fixture("out-issues", cfg.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    let result = export_report(
        &fx.file,
        &cfg,
        "redirects:redirect chains",
        tmp.path(),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(names_in(tmp.path()), vec!["redirect_chains.csv"]);
    assert_eq!(result.files.len(), 1);
    assert_eq!(result.files[0].path, tmp.path().join("redirect_chains.csv"));

    let result = export_report(&fx.file, &cfg, "Crawl Overview", tmp.path(), Format::Xlsx).unwrap();
    assert!(tmp.path().join("crawl_overview.xlsx").is_file());
    assert_eq!(result.files.len(), 1);

    let err = export_report(&fx.file, &cfg, "Crawl Overvew", tmp.path(), Format::Csv).unwrap_err();
    match err {
        os_export::ExportError::Name(e) => {
            assert_eq!(
                suggestions(e).first().map(String::as_str),
                Some("Crawl Overview")
            );
        }
        other => panic!("falscher Fehler: {other}"),
    }
}

// spec: OUT-008
#[tokio::test]
async fn unknown_bulk_name_fails_with_suggestions() {
    let cfg = base_config();
    let fx = crawl_fixture("out-issues", cfg.clone()).await;
    let tmp = tempfile::tempdir().unwrap();
    let err =
        export_bulk(&fx.file, &cfg, "Issues:All Issuse", tmp.path(), Format::Csv).unwrap_err();
    match err {
        os_export::ExportError::Name(e) => {
            assert_eq!(
                suggestions(e).first().map(String::as_str),
                Some("Issues:All Issues")
            );
        }
        other => panic!("falscher Fehler: {other}"),
    }
    assert!(!tmp.path().join("issues").exists(), "nichts geschrieben");
}
