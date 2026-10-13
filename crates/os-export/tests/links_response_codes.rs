//! Response-Codes-Inlinks-Exporte (OUT-050): 12 Einträge × {Internal & External, Internal, External}.
//! Fixture `out-response-codes`. Platzhalter-Modus: Die Auswertung läuft gegen den Test-Tab
//! `response_codes_tab()` (echte Prädikate, `links_support.rs`); die Goldens gegen die echten Filter von P21
//! stehen mit `#[ignore = "M1-P26"]` bereit.

#[path = "links_support.rs"]
mod support;

use std::collections::BTreeSet;

use os_analyze::tabs;
use os_export::catalog::bulk_links::{self, Selection};
use os_export::catalog::{self, ExportKind, ExportRef};
use os_export::pipeline::export_bulk_in;
use os_export::{export_bulk, naming, ExportFileInfo, Format};
use os_model::CrawlConfig;
use os_store::CrawlFile;
use support::*;

const SCOPES: [&str; 3] = ["Internal & External", "Internal", "External"];
const ENTRIES: [&str; 12] = [
    "All Inlinks",
    "Blocked by Robots.txt Inlinks",
    "Blocked Resource Inlinks",
    "No Response Inlinks",
    "Success (2xx) Inlinks",
    "Redirection (3xx) Inlinks",
    "Redirection (JavaScript) Inlinks",
    "Redirection (Meta Refresh) Inlinks",
    "Redirection (HTTP Refresh) Inlinks",
    "Client Error (4xx) Inlinks",
    "Server Error (5xx) Inlinks",
    "All Error Inlinks",
];

fn u(path: &str) -> String {
    format!("http://www.site.test{path}")
}

fn export(
    file: &CrawlFile,
    name: &str,
    dir: &std::path::Path,
) -> (ExportFileInfo, Vec<Vec<String>>) {
    let tabs = [response_codes_tab()];
    let infos =
        export_bulk_in(&tabs, file, &CrawlConfig::default(), name, dir, Format::Csv).unwrap();
    assert_eq!(infos.len(), 1);
    let info = infos.into_iter().next().unwrap();
    let csv = read_csv(&info.path);
    let mut rows = parse_csv(&csv);
    assert_eq!(rows[0].join(","), "Type,Source,Destination,Size (Bytes),Alt Text,Anchor,Status Code,Status,Follow,Target,Rel,Path Type,Link Path,Link Position,Link Origin");
    rows.remove(0);
    assert_eq!(info.rows as usize, rows.len());
    (info, rows)
}

fn names(prefix_scope: &str, entry: &str) -> String {
    format!("Response Codes:{prefix_scope}:{entry}")
}

fn dests(rows: &[Vec<String>]) -> BTreeSet<String> {
    rows.iter().map(|r| r[2].clone()).collect()
}

// spec: OUT-050
#[test]
fn all_36_entries_are_registered_with_distinct_file_names() {
    let all = bulk_links::names();
    for scope in SCOPES {
        for entry in ENTRIES {
            let n = names(scope, entry);
            assert!(all.contains(&n), "{n} fehlt");
            assert_eq!(
                catalog::resolve(ExportKind::Bulk, &n.to_lowercase()).unwrap(),
                ExportRef::Bulk(n.clone())
            );
        }
    }
    // der Eintrag `All` der Spec ist als Alias von `All Inlinks` ansprechbar
    assert_eq!(
        catalog::resolve(ExportKind::Bulk, "Response Codes:Internal:All").unwrap(),
        ExportRef::Bulk("Response Codes:Internal:All Inlinks".into())
    );
    assert_eq!(all.len(), 2 + 36);
    // OUT-008: keine Kollision mit den Tab-Exporten desselben Tabs
    let cfg = CrawlConfig::default();
    let tab_names: Vec<String> = tabs::all()
        .iter()
        .flat_map(|t| {
            t.filters
                .iter()
                .map(|f| format!("{}:{}", t.id, f.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let _ = cfg;
    let clashes = naming::collisions(tab_names.iter().chain(all.iter()).map(String::as_str));
    assert!(clashes.is_empty(), "{clashes:?}");
    assert_eq!(
        naming::file_name(
            "Response Codes:Internal:Redirection (HTTP Refresh) Inlinks",
            None,
            "csv"
        ),
        "response_codes_internal_redirection_http_refresh_inlinks.csv"
    );
}

// spec: OUT-050
#[test]
fn unknown_entry_names_get_suggestions() {
    let err = catalog::resolve(
        ExportKind::Bulk,
        "Response Codes:Internal:Client Error (4xx) Inlink",
    )
    .unwrap_err();
    match err {
        os_analyze::NameError::Unknown { suggestions, .. } => {
            assert_eq!(
                suggestions[0], "Response Codes:Internal:Client Error (4xx) Inlinks",
                "der nächste Name zuerst"
            );
            assert!(suggestions.len() <= 5);
        }
    }
}

// spec: OUT-050
#[tokio::test]
async fn internal_client_errors_list_every_link_to_an_internal_4xx_target() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let (info, rows) = export(
        &fx.file,
        &names("Internal", "Client Error (4xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(
        info.path.file_name().unwrap(),
        "response_codes_internal_client_error_4xx_inlinks.csv"
    );
    assert_eq!(
        dests(&rows),
        BTreeSet::from([u("/e403"), u("/e404"), u("/e410"), u("/gone404")])
    );
    assert!(rows
        .iter()
        .all(|r| r[6].starts_with('4') && r[6].len() == 3));
    // /e404 wird von zwei Seiten verlinkt, Ankertexte bleiben erhalten
    let e404: Vec<&Vec<String>> = rows.iter().filter(|r| r[2] == u("/e404")).collect();
    assert_eq!(e404.len(), 2);
    assert_eq!(e404[0][1], u("/"));
    assert_eq!(e404[0][5], "404");
    assert_eq!(e404[1][1], u("/second.html"));
    assert_eq!(e404[1][5], "404 von der zweiten Seite");
    // Anzahl der Ziele = Zähler des Filters
    let counts = os_analyze::views::filter_counts_in(
        &[response_codes_tab()],
        &fx.file,
        &CrawlConfig::default(),
    )
    .unwrap();
    let c = counts
        .iter()
        .find(|c| c.filter == "Internal Client Error (4XX)")
        .unwrap();
    assert_eq!(c.count, Some(dests(&rows).len() as u64));
}

// spec: OUT-050
#[tokio::test]
async fn the_source_can_itself_be_a_redirect() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let (_, rows) = export(
        &fx.file,
        &names("Internal", "Client Error (4xx) Inlinks"),
        dir.path(),
    );
    let via_redirect: Vec<&Vec<String>> = rows.iter().filter(|r| r[2] == u("/gone404")).collect();
    assert_eq!(via_redirect.len(), 1);
    assert_eq!(via_redirect[0][0], "HTTP Redirect");
    assert_eq!(via_redirect[0][1], u("/r301"));
    // die 301 selbst steht im Export „Redirection (3xx)“
    let (_, r3) = export(
        &fx.file,
        &names("Internal", "Redirection (3xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(dests(&r3), BTreeSet::from([u("/r301"), u("/r302")]));
}

// spec: OUT-050
#[tokio::test]
async fn scopes_split_internal_and_external_targets() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let (_, ext) = export(
        &fx.file,
        &names("External", "Client Error (4xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(
        dests(&ext),
        BTreeSet::from(["http://external.test/ext404".to_string()])
    );
    assert_eq!(ext.len(), 2, "von zwei Seiten verlinkt");
    let (_, both) = export(
        &fx.file,
        &names("Internal & External", "Client Error (4xx) Inlinks"),
        dir.path(),
    );
    let (_, int) = export(
        &fx.file,
        &names("Internal", "Client Error (4xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(both.len(), ext.len() + int.len());
    let (_, ext5) = export(
        &fx.file,
        &names("External", "Server Error (5xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(
        dests(&ext5),
        BTreeSet::from(["http://external.test/ext500".to_string()])
    );
    let (_, int5) = export(
        &fx.file,
        &names("Internal", "Server Error (5xx) Inlinks"),
        dir.path(),
    );
    assert_eq!(dests(&int5), BTreeSet::from([u("/e500"), u("/e503")]));
}

// spec: OUT-050
#[tokio::test]
async fn no_response_and_blocked_targets_have_no_status_code() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let (_, none) = export(
        &fx.file,
        &names("Internal", "No Response Inlinks"),
        dir.path(),
    );
    assert_eq!(dests(&none), BTreeSet::from([u("/drop")]));
    assert_eq!(none[0][6], "", "Status Code leer");
    assert_eq!(none[0][7], "Connection Error", "Status mit Fehlergrund");
    let (_, blocked) = export(
        &fx.file,
        &names("Internal", "Blocked by Robots.txt Inlinks"),
        dir.path(),
    );
    assert_eq!(
        dests(&blocked),
        BTreeSet::from([u("/private/blocked.html")])
    );
    assert_eq!(
        (blocked[0][6].as_str(), blocked[0][7].as_str()),
        ("", "Blocked by Robots.txt")
    );
}

// spec: OUT-050
#[tokio::test]
async fn refresh_targets_are_classified_by_their_refresh_and_not_as_success() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    // `Refresh: 0; url=…` im Antwortheader (Status 200)
    let (_, http) = export(
        &fx.file,
        &names("Internal", "Redirection (HTTP Refresh) Inlinks"),
        dir.path(),
    );
    assert_eq!(dests(&http), BTreeSet::from([u("/refresh-header")]));
    assert_eq!(http[0][0], "Hyperlink");
    // <meta http-equiv="refresh"> nur in Meta Refresh
    let (_, meta) = export(
        &fx.file,
        &names("Internal", "Redirection (Meta Refresh) Inlinks"),
        dir.path(),
    );
    assert_eq!(dests(&meta), BTreeSet::from([u("/meta.html")]));
    assert!(!dests(&http).contains(&u("/meta.html")));
    assert!(!dests(&meta).contains(&u("/refresh-header")));
    // Success enthält weder Meta- noch HTTP-Refresh-Ziele, wohl aber die JavaScript-Seite (kein Rendering)
    let (_, ok) = export(
        &fx.file,
        &names("Internal", "Success (2xx) Inlinks"),
        dir.path(),
    );
    let d = dests(&ok);
    assert!(!d.contains(&u("/refresh-header")) && !d.contains(&u("/meta.html")));
    assert!(d.contains(&u("/ok.html")) && d.contains(&u("/js.html")));
    // die Ziele der Weiterleitungen selbst sind 200 und stehen im Success-Export
    assert!(d.contains(&u("/refresh-target.html")));
}

// spec: OUT-050
#[tokio::test]
async fn all_error_inlinks_are_the_union_of_no_response_4xx_and_5xx() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let key = |rows: &[Vec<String>]| -> BTreeSet<(String, String, String, String)> {
        rows.iter()
            .map(|r| (r[2].clone(), r[1].clone(), r[0].clone(), r[12].clone()))
            .collect()
    };
    let (_, all) = export(
        &fx.file,
        &names("Internal & External", "All Error Inlinks"),
        dir.path(),
    );
    let mut union = BTreeSet::new();
    for entry in [
        "No Response Inlinks",
        "Client Error (4xx) Inlinks",
        "Server Error (5xx) Inlinks",
    ] {
        let (_, rows) = export(&fx.file, &names("Internal & External", entry), dir.path());
        assert!(!rows.is_empty(), "{entry}");
        union.extend(key(&rows));
    }
    assert_eq!(key(&all), union);
    assert_eq!(all.len(), union.len(), "keine Zeile doppelt");
    // sortiert nach Destination, Source, Dokumentreihenfolge (OUT-031)
    let order: Vec<(&str, &str)> = all.iter().map(|r| (r[2].as_str(), r[1].as_str())).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
}

// spec: OUT-050
#[tokio::test]
async fn entries_without_an_m1_filter_deliver_the_header_only() {
    // Blocked Resource und Redirection (JavaScript) setzen JavaScript-Rendering voraus (K-14).
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    for scope in SCOPES {
        for entry in [
            "Blocked Resource Inlinks",
            "Redirection (JavaScript) Inlinks",
        ] {
            let (info, rows) = export(&fx.file, &names(scope, entry), dir.path());
            assert!(rows.is_empty(), "{scope}:{entry}");
            assert_eq!(info.rows, 0);
            assert_eq!(info.warnings.len(), 1);
            assert!(
                info.warnings[0].contains("Rendering"),
                "{:?}",
                info.warnings
            );
        }
    }
    let e = bulk_links::resolve(&names("Internal", "Blocked Resource Inlinks")).unwrap();
    assert!(e.requires_rendering && e.selection == Selection::Empty);
}

// spec: OUT-050
#[tokio::test]
async fn every_entry_of_every_scope_exports_a_file_with_the_link_header() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let mut files = BTreeSet::new();
    for scope in SCOPES {
        for entry in ENTRIES {
            let (info, _) = export(&fx.file, &names(scope, entry), dir.path());
            files.insert(
                info.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    assert_eq!(files.len(), 36);
}

// spec: OUT-050
#[tokio::test]
async fn every_entry_exports_against_the_registered_tabs() {
    // Gegen die registrierten Tabs (heute P00b-Platzhalter, später die Filter von P21): kein Fehler, immer
    // eine Datei mit Kopfzeile, die Ausgabedateien sind verschieden.
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let mut files = BTreeSet::new();
    for name in bulk_links::names() {
        let infos = export_bulk(&fx.file, &cfg, &name, dir.path(), Format::Csv)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = read_csv(&infos[0].path);
        assert!(text.starts_with("Type,Source,Destination,"), "{name}");
        files.insert(infos[0].path.clone());
    }
    assert_eq!(files.len(), bulk_links::names().len());
}

// spec: OUT-050
#[tokio::test]
#[ignore = "M1-P26"]
async fn golden_inlinks_exports_with_the_real_response_codes_filters() {
    // P26: nach dem Merge von P21 Ignore entfernen und die Goldens mit UPDATE_GOLDEN=1 erzeugen.
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    for name in bulk_links::names()
        .into_iter()
        .filter(|n| n.starts_with("Response Codes:"))
    {
        let infos = export_bulk(&fx.file, &cfg, &name, dir.path(), Format::Csv).unwrap();
        let file = infos[0]
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_golden(
            &format!("out-response-codes/{file}"),
            &read_csv(&infos[0].path),
        );
    }
}
