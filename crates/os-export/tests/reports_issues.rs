//! Tests des Issues-Exports (OUT-063): Ordner `issues/`, eine Datei je Issue, `issues_overview`.
//!
//! Die Tab-Pakete (P20–P25) liefern die echten Filter erst später; die Tests laufen gegen Test-Tabs mit den
//! Katalog-Namen (`reports_common::issue_tabs`) und strukturell gegen die Platzhalter-Tabs. Goldens mit den
//! echten Zählern stehen als `#[ignore = "M1-P26"]` bereit.

#[path = "reports_common.rs"]
mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{base_config, issue_tabs, Table};
use os_analyze::issues::{self, Lang};
use os_analyze::views::{filter_counts_in, query_view_in, ViewQuery};
use os_analyze::TabDef;
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_export::catalog::bulk_issues::{self, export_in, IssuesExport};
use os_export::{ExportFileInfo, Format};
use os_model::CrawlConfig;
use os_testsite::golden::{assert_golden, normalize};

async fn crawl(cfg: &CrawlConfig) -> FixtureCrawl {
    crawl_fixture("out-issues", cfg.clone()).await
}

fn names(files: &[ExportFileInfo]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

/// Adressen aus der Spalte `Address` einer Issue-Datei, sortiert.
fn addresses(path: &Path) -> BTreeSet<String> {
    let t = Table::read(path);
    t.column("Address")
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn export(
    fx: &FixtureCrawl,
    cfg: &CrawlConfig,
    tabs: &[TabDef],
    format: Format,
) -> (tempfile::TempDir, IssuesExport) {
    let tmp = tempfile::tempdir().unwrap();
    let out = export_in(tabs, &fx.file, cfg, tmp.path(), format).unwrap();
    (tmp, out)
}

// spec: OUT-063
#[tokio::test]
async fn one_file_per_issue_row_named_after_type_tab_and_filter() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, out) = export(&fx, &cfg, &tabs, Format::Csv);

    let dir = tmp.path().join("issues");
    assert!(dir.is_dir());
    let file_names = names(&out.files);
    // Issues-Panel: Titel fehlt, 4xx intern (Issues), H1 fehlt (Issue/Medium), Titel doppelt und zu lang.
    for expected in [
        "issue_page_titles_missing.csv",
        "issue_h1_missing.csv",
        "issue_response_codes_internal_client_error_4xx.csv",
        "opportunity_page_titles_duplicate.csv",
        "opportunity_page_titles_over_60_characters.csv",
        "issues_overview.csv",
    ] {
        assert!(
            file_names.iter().any(|n| n == expected),
            "{expected} fehlt in {file_names:?}"
        );
        assert!(dir.join(expected).is_file(), "{expected}");
    }
    // Anzahl Dateien (ohne Inlinks-Varianten und Übersicht) = Zeilen im Issues-Panel.
    let counts = filter_counts_in(&tabs, &fx.file, &cfg).unwrap();
    let panel = issues::issues_from_counts(&counts);
    let issue_files = file_names
        .iter()
        .filter(|n| *n != "issues_overview.csv" && !n.ends_with("_inlinks.csv"))
        .count();
    assert_eq!(panel.len(), 5);
    assert_eq!(issue_files, panel.len());
    // Die Übersicht ist die letzte Datei.
    assert_eq!(
        file_names.last().map(String::as_str),
        Some("issues_overview.csv")
    );
}

// spec: OUT-063
#[tokio::test]
async fn every_file_holds_exactly_the_urls_of_its_issue_counter() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, _out) = export(&fx, &cfg, &tabs, Format::Csv);
    let dir = tmp.path().join("issues");
    let base = fx.site.base_url();
    let set = |paths: &[&str]| -> BTreeSet<String> {
        paths.iter().map(|p| format!("{base}{p}")).collect()
    };
    assert_eq!(
        addresses(&dir.join("issue_page_titles_missing.csv")),
        set(&["/no-title.html"])
    );
    assert_eq!(
        addresses(&dir.join("opportunity_page_titles_duplicate.csv")),
        set(&["/dup-a.html", "/dup-b.html"])
    );
    assert_eq!(
        addresses(&dir.join("opportunity_page_titles_over_60_characters.csv")),
        set(&["/long-title.html"])
    );
    assert_eq!(
        addresses(&dir.join("issue_h1_missing.csv")),
        set(&["/no-h1.html"])
    );
    assert_eq!(
        addresses(&dir.join("issue_response_codes_internal_client_error_4xx.csv")),
        set(&["/missing.html", "/gone"])
    );

    // Allgemein: Zeilenzahl jeder Datei = Zähler des Panels (Ansicht und Export teilen den Plan).
    let counts = filter_counts_in(&tabs, &fx.file, &cfg).unwrap();
    let overview = Table::read(&dir.join("issues_overview.csv"));
    for row in &overview.rows {
        let name = overview.get(row, "Issue Name");
        let (tab, label) = name.split_once(": ").unwrap();
        let c = counts
            .iter()
            .find(|c| c.tab == tab && c.label == label)
            .unwrap();
        assert_eq!(
            overview.get(row, "URLs"),
            c.count.unwrap().to_string(),
            "{name}"
        );
    }
}

// spec: OUT-063
#[tokio::test]
async fn issues_overview_columns_follow_the_panel_export_with_why_it_matters_last() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, _out) = export(&fx, &cfg, &tabs, Format::Csv);
    let t = Table::read(&tmp.path().join("issues/issues_overview.csv"));
    assert_eq!(
        t.header,
        [
            "Issue Name",
            "Issue Type",
            "Issue Priority",
            "URLs",
            "% of Total",
            "Description",
            "How To Fix",
            "Why It Matters"
        ]
    );
    // Reihenfolge des Panels: Typ → Priorität → Anzahl absteigend.
    let names: Vec<&str> = t.column("Issue Name");
    assert_eq!(
        names,
        [
            "Response Codes: Internal Client Error (4XX)",
            "Page Titles: Missing",
            "H1: Missing",
            "Page Titles: Duplicate",
            "Page Titles: Over 60 Characters",
        ]
    );
    let r = t.row("Issue Name", "Page Titles: Missing");
    assert_eq!(t.get(r, "Issue Type"), "Issue");
    assert_eq!(t.get(r, "Issue Priority"), "High");
    assert_eq!(t.get(r, "URLs"), "1");
    // 1 von 7 Seiten in der Grundmenge des Tabs, zwei Nachkommastellen.
    assert_eq!(t.get(r, "% of Total"), "14.29");
    // Description = what, How To Fix = fix, Why It Matters = why (Englisch, mit den Schwellen der Konfiguration).
    let texts = issues::texts("page-titles-missing", Lang::En, &[]).unwrap();
    assert_eq!(t.get(r, "Description"), texts.what);
    assert_eq!(t.get(r, "How To Fix"), texts.fix);
    assert_eq!(t.get(r, "Why It Matters"), texts.why);
    for row in &t.rows {
        for col in ["Description", "How To Fix", "Why It Matters"] {
            assert!(!t.get(row, col).trim().is_empty(), "{col} leer in {row:?}");
        }
    }
    // Prozent des 4xx-Issues: 2 von 14 Zeilen der Grundmenge dieses Test-Tabs.
    assert_eq!(
        t.get(
            t.row("Issue Name", "Response Codes: Internal Client Error (4XX)"),
            "% of Total"
        ),
        "14.29"
    );
}

// spec: OUT-063
#[tokio::test]
async fn texts_use_the_thresholds_of_the_crawl_configuration() {
    let mut cfg = base_config();
    cfg.title.max_chars = 35;
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, out) = export(&fx, &cfg, &tabs, Format::Csv);
    // Name der Datei und des Issues nennen den aktuellen Schwellenwert.
    let file_names = names(&out.files);
    assert!(
        file_names
            .iter()
            .any(|n| n == "opportunity_page_titles_over_35_characters.csv"),
        "{file_names:?}"
    );
    let t = Table::read(&tmp.path().join("issues/issues_overview.csv"));
    let r = t.row("Issue Name", "Page Titles: Over 35 Characters");
    let params = (tabs[0].params)(&cfg);
    let texts = issues::texts("page-titles-over-max-chars", Lang::En, &params).unwrap();
    assert_eq!(t.get(r, "Description"), texts.what);
    assert!(
        texts.what.contains("35") || texts.fix.contains("35") || texts.why.contains("35"),
        "die Schwelle 35 muss in den Texten stehen: {texts:?}"
    );
    assert!(t.get(r, "Description").contains("35") || t.get(r, "How To Fix").contains("35"));
    // Dateien nennen den Schwellenwert in der Zeile „Over 35 Characters“ und enthalten drei URLs.
    assert_eq!(
        addresses(
            &tmp.path()
                .join("issues/opportunity_page_titles_over_35_characters.csv")
        )
        .len(),
        3
    );
}

// spec: OUT-063
#[tokio::test]
async fn ignore_non_indexable_acts_on_panel_and_export_alike() {
    let mut tab_with_flag = issue_tabs();
    // „Missing“ der Titel ignoriert nicht indexierbare Seiten (CRAWL-089).
    for tab in &mut tab_with_flag {
        if tab.id == "Page Titles" {
            for f in &mut tab.filters {
                if f.id == "Missing" {
                    f.ignore_non_indexable = true;
                }
            }
        }
    }
    let mut cfg = base_config();
    let fx = crawl(&cfg).await;

    cfg.issues.ignore_non_indexable = true;
    let (tmp_on, out_on) = export(&fx, &cfg, &tab_with_flag, Format::Csv);
    assert!(names(&out_on.files)
        .iter()
        .any(|n| n == "issue_page_titles_missing.csv"));
    drop(tmp_on);

    // Die Seite ohne Title ist nicht indexierbar: mit dem Schalter verschwindet das Issue in beiden Sichten.
    fx.file
        .conn()
        .execute_batch(
            "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = 'noindex' \
             WHERE address LIKE '%/no-title.html';",
        )
        .unwrap();
    let (tmp, out) = export(&fx, &cfg, &tab_with_flag, Format::Csv);
    assert!(!names(&out.files)
        .iter()
        .any(|n| n == "issue_page_titles_missing.csv"));
    let counts = filter_counts_in(&tab_with_flag, &fx.file, &cfg).unwrap();
    let missing = counts
        .iter()
        .find(|c| c.tab == "Page Titles" && c.filter == "Missing")
        .unwrap();
    assert_eq!(missing.count, Some(0));
    let t = Table::read(&tmp.path().join("issues/issues_overview.csv"));
    assert!(t.find("Issue Name", "Page Titles: Missing").is_none());

    // Schalter aus: das Issue kehrt zurück, mit identischem Zähler in Panel und Datei.
    cfg.issues.ignore_non_indexable = false;
    let (tmp2, out2) = export(&fx, &cfg, &tab_with_flag, Format::Csv);
    assert!(names(&out2.files)
        .iter()
        .any(|n| n == "issue_page_titles_missing.csv"));
    let q = ViewQuery::new("Page Titles", "Missing");
    let page = query_view_in(&tab_with_flag, &fx.file, &cfg, &q).unwrap();
    assert_eq!(
        addresses(&tmp2.path().join("issues/issue_page_titles_missing.csv")).len() as u64,
        page.total
    );
}

// spec: OUT-063
#[tokio::test]
async fn link_related_issues_get_an_inlinks_variant_or_a_warning() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, out) = export(&fx, &cfg, &tabs, Format::Csv);
    // Nur die Issues des Tabs „Response Codes“ sind linkbezogen.
    let inlinks: Vec<String> = names(&out.files)
        .into_iter()
        .filter(|n| n.ends_with("_inlinks.csv"))
        .collect();
    let skipped = out
        .warnings
        .iter()
        .any(|w| w.contains("Response Codes") && w.contains("inlinks file skipped"));
    // Die Inlinks-Datei schreibt `export_detail` (M1-P16). Bis dahin meldet der Export den Verzicht.
    if inlinks.is_empty() {
        assert!(
            skipped,
            "weder Inlinks-Datei noch Hinweis: {:?}",
            out.warnings
        );
    } else {
        assert_eq!(
            inlinks,
            vec!["issue_response_codes_internal_client_error_4xx_inlinks.csv"]
        );
        assert!(tmp.path().join("issues").join(&inlinks[0]).is_file());
        assert!(!skipped);
    }
}

// spec: OUT-063
#[test]
fn the_link_related_rule_covers_response_codes_only() {
    let entries = issues::catalog();
    let related: Vec<&str> = entries
        .iter()
        .filter(|e| bulk_issues::is_link_related(e))
        .map(|e| e.tab.as_str())
        .collect();
    assert!(!related.is_empty());
    assert!(related.iter().all(|t| *t == "Response Codes"));
    // Seiten-Issues (Titel, H1 …) sind es nie.
    for e in entries
        .iter()
        .filter(|e| e.tab == "Page Titles" || e.tab == "H1")
    {
        assert!(!bulk_issues::is_link_related(e));
    }
}

// spec: OUT-063
#[tokio::test]
async fn xlsx_writes_one_workbook_per_issue_and_an_overview_workbook() {
    use calamine::{open_workbook, Reader, Xlsx};
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, out) = export(&fx, &cfg, &tabs, Format::Xlsx);
    let file_names = names(&out.files);
    assert!(file_names.contains(&"issue_page_titles_missing.xlsx".to_string()));
    assert_eq!(
        file_names.last().map(String::as_str),
        Some("issues_overview.xlsx")
    );
    let mut wb: Xlsx<_> = open_workbook(tmp.path().join("issues/issues_overview.xlsx")).unwrap();
    let sheet = wb.worksheet_range("Issues Overview").unwrap();
    assert_eq!(sheet.height(), 6);
    assert_eq!(sheet.width(), 8);
    let mut wb2: Xlsx<_> =
        open_workbook(tmp.path().join("issues/issue_page_titles_missing.xlsx")).unwrap();
    // Blattname = Filtername, Excel-tauglich.
    let sheets = wb2.sheet_names();
    assert_eq!(sheets, vec!["Missing".to_string()]);
    assert_eq!(wb2.worksheet_range("Missing").unwrap().height(), 2);
}

// spec: OUT-063
#[tokio::test]
async fn the_overview_is_written_last_with_its_header_for_the_real_tabs() {
    // Mit den umgesetzten Tabs (P20–P25) hat der Fixture-Crawl echte Issues; die Übersicht entsteht zuletzt, mit
    // Kopfzeile und je Issue-Zeile eine Datei (Inlinks-Dateien zählen nicht als eigene Zeile).
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    let files =
        os_export::catalog::bulk_issues::export(&fx.file, &cfg, tmp.path(), Format::Csv).unwrap();
    let file_names = names(&files);
    assert_eq!(
        file_names.last().map(String::as_str),
        Some("issues_overview.csv")
    );
    let t = Table::read(&tmp.path().join("issues/issues_overview.csv"));
    assert_eq!(t.header.len(), 8);
    assert!(!t.rows.is_empty());
    let issue_files = file_names
        .iter()
        .filter(|n| n.as_str() != "issues_overview.csv" && !n.ends_with("_inlinks.csv"))
        .count();
    assert_eq!(t.rows.len(), issue_files);
    assert_eq!(files.last().map(|f| f.rows), Some(t.rows.len() as u64));
}

// spec: OUT-063, OUT-112
#[tokio::test]
async fn crawl_analysis_dependencies_are_named_in_the_warnings() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    // Platzhalter-Tabs enthalten Nachlauf-Filter, die ohne Crawl-Analyse leer bleiben.
    let tmp = tempfile::tempdir().unwrap();
    let out = export_in(
        &os_analyze::tabs::all(),
        &fx.file,
        &cfg,
        tmp.path(),
        Format::Csv,
    )
    .unwrap();
    assert!(
        out.warnings
            .iter()
            .any(|w| w.starts_with("report 'Issues:All Issues' requires the crawl analysis")),
        "{:?}",
        out.warnings
    );
    // Der Hinweis steht auch an der Übersichtsdatei.
    let overview = out.files.last().unwrap();
    assert!(overview
        .warnings
        .iter()
        .any(|w| w.contains("crawl analysis")));
}

// spec: OUT-063
#[test]
fn the_bulk_name_is_registered_and_unique_among_file_names() {
    assert_eq!(bulk_issues::names(), vec!["Issues:All Issues"]);
    assert_eq!(
        bulk_issues::resolve(" issues : all issues ").unwrap(),
        "Issues:All Issues"
    );
    assert!(bulk_issues::resolve("Issues:All Issue").is_err());
    // Kein Dateiname der Issue-Dateien kollidiert mit einem anderen: alle Katalogeinträge ergeben
    // verschiedene Namen (mit und ohne Inlinks-Variante).
    let mut seen = BTreeSet::new();
    for e in issues::catalog() {
        let row = issues::IssueRow {
            id: e.id.clone(),
            tab: e.tab.clone(),
            filter: e.filter.clone(),
            issue_type: e.issue_type,
            priority: e.priority,
            count: 1,
            pct: None,
        };
        let label = issues::filter_label(e, &[]);
        for name in [
            bulk_issues::issue_file_name(&row, &label, "csv"),
            bulk_issues::inlinks_file_name(&row, &label, "csv"),
        ] {
            assert!(name.len() <= 124, "{name}");
            assert!(seen.insert(name.clone()), "doppelter Dateiname {name}");
        }
    }
}

// spec: OUT-063
#[tokio::test]
async fn issue_files_match_the_regression_golden_with_test_tabs() {
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tabs = issue_tabs();
    let (tmp, _out) = export(&fx, &cfg, &tabs, Format::Csv);
    let bases = fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let golden_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/out-issues/test-tabs");
    let mut produced: Vec<String> = std::fs::read_dir(tmp.path().join("issues"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    produced.sort();
    for name in produced {
        let text = std::fs::read_to_string(tmp.path().join("issues").join(&name)).unwrap();
        let text = if name == "issues_overview.csv" {
            mask_texts(&text)
        } else {
            text
        };
        assert_golden(
            golden_dir.join("issues").join(&name),
            &normalize(&text, &bases),
        );
    }
}

/// Ersetzt die drei Textspalten der Übersicht durch einen Platzhalter: die Texte pflegt der Katalog (P12),
/// ihr Wortlaut gehört nicht in dieses Regressions-Golden (der Test oben prüft sie gegen `issues::texts`).
fn mask_texts(text: &str) -> String {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(body.as_bytes());
    let mut wtr = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.unwrap();
        let fields: Vec<String> = rec
            .iter()
            .enumerate()
            .map(|(c, f)| {
                if i > 0 && c >= 5 {
                    "<TEXT>".to_string()
                } else {
                    f.to_string()
                }
            })
            .collect();
        wtr.write_record(&fields).unwrap();
    }
    String::from_utf8(wtr.into_inner().unwrap()).unwrap()
}

// spec: OUT-063
#[tokio::test]
#[ignore = "M1-P26"]
async fn issues_golden_with_the_real_tab_counters() {
    // P26 schaltet den Test scharf und erzeugt `tests/golden/out-issues/issues/*.csv`, sobald die Tabs P20–P25
    // echte Zähler und Filter liefern.
    let cfg = base_config();
    let fx = crawl(&cfg).await;
    let tmp = tempfile::tempdir().unwrap();
    bulk_issues::export(&fx.file, &cfg, tmp.path(), Format::Csv).unwrap();
    let bases = fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let golden_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/out-issues/issues");
    for entry in std::fs::read_dir(tmp.path().join("issues")).unwrap() {
        let entry = entry.unwrap();
        let text = std::fs::read_to_string(entry.path()).unwrap();
        assert_golden(
            golden_dir.join(entry.file_name()),
            &normalize(&text, &bases),
        );
    }
}

// spec: OUT-063
#[tokio::test]
async fn text_thresholds_come_from_the_configuration_even_if_a_tab_names_its_params_differently() {
    let mut cfg = base_config();
    cfg.title.max_chars = 35;
    let fx = crawl(&cfg).await;
    // Ein Tab, dessen SQL-Prädikat die Schwelle fest enthält und der keinen Parameter unter dem Konfigurationspfad
    // liefert: die Texte müssen trotzdem 35 nennen und nicht den Standardwert 60.
    let mut tabs = issue_tabs();
    let titles = tabs.iter_mut().find(|t| t.id == "Page Titles").unwrap();
    titles.params = |_| Vec::new();
    for f in &mut titles.filters {
        if f.id.starts_with("Over") {
            f.predicate = "u.title_1_len > 35";
        }
    }
    let (tmp, _out) = export(&fx, &cfg, &tabs, Format::Csv);
    let t = Table::read(&tmp.path().join("issues/issues_overview.csv"));
    let r = t.row("Issue Name", "Page Titles: Over 35 Characters");
    assert!(
        t.get(r, "Description").contains("35"),
        "{}",
        t.get(r, "Description")
    );
    assert!(!t.get(r, "Description").contains("60"));
}
