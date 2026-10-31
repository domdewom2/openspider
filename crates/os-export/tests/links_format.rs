//! Standard-Linkformat: Links:All Inlinks / All Outlinks (OUT-031, OUT-032). Fixture `out-links`
//! (Hosts `*.test`, Adressen ohne Port, damit die Goldens stabil sind).

#[path = "links_support.rs"]
mod support;

use std::collections::BTreeSet;

use calamine::{open_workbook, Data, Reader, Xlsx};
use os_export::catalog::bulk_links;
use os_export::links_export::LINK_COLUMNS;
use os_export::{export_bulk, export_detail, DetailKind, Format};
use os_model::{CrawlConfig, StoreCrawl, UrlId};
use os_store::queries;
use support::*;

const HEADER: &str = "Type,Source,Destination,Size (Bytes),Alt Text,Anchor,Status Code,Status,Follow,Target,Rel,Path Type,Link Path,Link Position,Link Origin";

const INLINKS: &str = "Links:All Inlinks";
const OUTLINKS: &str = "Links:All Outlinks";

fn bulk_csv(file: &os_store::CrawlFile, name: &str, out: &std::path::Path) -> String {
    let infos = export_bulk(file, &CrawlConfig::default(), name, out, Format::Csv).unwrap();
    assert_eq!(infos.len(), 1);
    read_csv(&infos[0].path)
}

/// Zeilen als Zellen, ohne Kopfzeile.
fn rows(csv: &str) -> Vec<Vec<String>> {
    parse_csv(csv).into_iter().skip(1).collect()
}

fn find<'a>(rows: &'a [Vec<String>], source: &str, dest: &str, ty: &str) -> &'a Vec<String> {
    rows.iter()
        .find(|r| r[0] == ty && r[1] == source && r[2] == dest)
        .unwrap_or_else(|| panic!("keine Zeile {ty} {source} -> {dest}"))
}

const HOME: &str = "http://www.site.test/";

fn u(path: &str) -> String {
    format!("http://www.site.test{path}")
}

// spec: OUT-031
#[tokio::test]
async fn header_is_exactly_the_standard_link_format() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    for name in [INLINKS, OUTLINKS] {
        let csv = bulk_csv(&fx.file, name, out.path());
        assert_eq!(csv.lines().next().unwrap(), HEADER, "{name}");
    }
    assert_eq!(LINK_COLUMNS.join(","), HEADER);
    // bei null Verweisen bleibt die Kopfzeile (leerer Crawl)
    let empty = new_file(out.path(), "leer.ospider");
    let csv = bulk_csv(&empty, INLINKS, &out.path().join("leer"));
    assert_eq!(csv, format!("{HEADER}\n"));
}

// spec: OUT-031
#[tokio::test]
async fn all_inlinks_golden() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let csv = bulk_csv(&fx.file, INLINKS, out.path());
    assert_golden("out-links/links_all_inlinks.csv", &csv);
}

// spec: OUT-032
#[tokio::test]
async fn all_outlinks_golden() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let csv = bulk_csv(&fx.file, OUTLINKS, out.path());
    assert_golden("out-links/links_all_outlinks.csv", &csv);
}

// spec: OUT-031
#[tokio::test]
async fn nofollow_link_with_target_has_the_expected_cells() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    // <a href="/x" rel="nofollow" target="_blank">Text</a>
    let r = all
        .iter()
        .find(|r| r[0] == "Hyperlink" && r[2] == u("/x") && r[5] == "Text")
        .unwrap();
    assert_eq!(r[1], HOME);
    assert_eq!(r[8], "False", "Follow");
    assert_eq!(r[9], "_blank", "Target");
    assert_eq!(r[10], "nofollow", "Rel");
    assert_eq!(r[11], "Root-Relative", "Path Type");
    // <a href="/x" rel="Nofollow  UGC">: Rel kleingeschrieben, ein Leerzeichen
    let r = all
        .iter()
        .find(|r| r[0] == "Hyperlink" && r[2] == u("/x") && r[5] == "ugc")
        .unwrap();
    assert_eq!(r[10], "nofollow ugc");
    assert_eq!(r[8], "False");
    // rel="SPONSORED"
    let r = find(&all, HOME, &u("/sponsor.html"), "Hyperlink");
    assert_eq!((r[10].as_str(), r[8].as_str()), ("sponsored", "False"));
}

// spec: OUT-031
#[tokio::test]
async fn image_row_has_alt_text_path_type_and_size_of_the_crawled_image() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    // <img src="pic.png" alt="Hund">
    let r = find(&all, HOME, &u("/pic.png"), "Image");
    assert_eq!(r[4], "Hund", "Alt Text");
    assert_eq!(r[11], "Path-Relative");
    assert_eq!(r[3], "1234", "Größe des gecrawlten Bildes");
    assert_eq!((r[6].as_str(), r[7].as_str()), ("200", "OK"));
    // Hyperlink um ein Bild trägt den Alt-Text des Bildes
    let r = find(&all, HOME, &u("/bild.html"), "Hyperlink");
    assert_eq!(r[4], "Hund");
    // alle vier Pfadarten kommen vor
    let kinds: BTreeSet<&str> = all.iter().map(|r| r[11].as_str()).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "Absolute",
            "Path-Relative",
            "Protocol-Relative",
            "Root-Relative"
        ])
    );
}

// spec: OUT-031
#[tokio::test]
async fn identical_links_are_not_deduplicated() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    let dups: Vec<_> = all
        .iter()
        .filter(|r| r[1] == HOME && r[2] == u("/dup.html"))
        .collect();
    assert_eq!(
        dups.len(),
        2,
        "zwei gleiche Links auf einer Seite = zwei Zeilen"
    );
    assert_ne!(dups[0][12], dups[1][12], "unterscheiden sich im Link Path");
}

// spec: OUT-031
#[tokio::test]
async fn target_without_fetch_is_not_crawled_and_has_no_status_code() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    // nofollow-Ziel wurde nie abgerufen; Paginierung wird nur gespeichert (hreflang nicht: K-62, siehe unten)
    for (dest, ty) in [
        (u("/x"), "Hyperlink"),
        (u("/page2.html"), "Rel Next"),
        (u("/page0.html"), "Rel Prev"),
        (u("/sponsor.html"), "Hyperlink"),
    ] {
        let r = all
            .iter()
            .find(|r| r[0] == ty && r[2] == dest)
            .unwrap_or_else(|| panic!("{ty} {dest}"));
        assert_eq!(r[6], "", "Status Code von {dest}");
        assert_eq!(r[7], "Not Crawled", "Status von {dest}");
        assert_eq!(r[3], "0", "Ungecrawltes internes Ziel: Größe von {dest}");
    }
    // Fehlerziele behalten ihren Code und Text
    let r = find(&all, HOME, &u("/missing.html"), "Hyperlink");
    assert_eq!((r[6].as_str(), r[7].as_str()), ("404", "Not Found"));
    let r = find(&all, HOME, &u("/old"), "Hyperlink");
    assert_eq!((r[6].as_str(), r[7].as_str()), ("301", "Moved Permanently"));
}

// spec: OUT-031, TECH-071
#[tokio::test]
async fn hreflang_only_targets_have_no_link_row_unless_hreflang_crawl_is_on() {
    // K-62 (N-08): /en.html ist nur per hreflang bekannt. Bei spider.hreflang.crawl = false (Standard) steht es
    // weder in den Tabellen noch in den Link-Exporten; ein Hreflang auf die eigene Seite bleibt.
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    assert!(!all
        .iter()
        .any(|r| (r[0] == "Hreflang" || r[0] == "HTTP Hreflang") && r[2] == u("/en.html")));
    assert!(all
        .iter()
        .any(|r| r[0] == "Hreflang" && r[1] == HOME && r[2] == HOME));
    // mit Crawl der hreflang-Ziele: Zeilen und Abruf wie bei jedem Link
    let mut cfg = links_config();
    cfg.spider.hreflang = StoreCrawl::ON;
    let on =
        os_engine::testing::crawl_fixture_from("out-links", "http://www.site.test/", cfg).await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&on.file, INLINKS, out.path()));
    let has = |ty: &str, src: &str| {
        all.iter()
            .any(|r| r[0] == ty && r[1] == src && r[2] == u("/en.html"))
    };
    assert!(has("Hreflang", HOME));
    assert!(has("HTTP Hreflang", &u("/headers.html")));
    let r = all
        .iter()
        .find(|r| r[0] == "Hreflang" && r[2] == u("/en.html"))
        .unwrap();
    assert_eq!(r[6], "200", "Ziel wurde abgerufen");
}

// spec: OUT-031
#[tokio::test]
async fn link_types_follow_their_source() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    let has = |ty: &str, src: &str, dest: &str| {
        all.iter()
            .any(|r| r[0] == ty && r[1] == src && r[2] == dest)
    };
    // Elemente im Kopf der Seite
    assert!(has("Canonical", HOME, HOME));
    assert!(has("Hreflang", HOME, HOME));
    assert!(has("Rel Next", HOME, &u("/page2.html")));
    assert!(has("Rel Prev", HOME, &u("/page0.html")));
    assert!(has("CSS", HOME, &u("/style.css")));
    assert!(has("JavaScript", HOME, &u("/app.js")));
    assert!(has("Iframe", HOME, &u("/frame.html")));
    assert!(has("Media", HOME, &u("/v.mp4")));
    // Antwortheader: Link (canonical/hreflang) und Refresh
    assert!(has("HTTP Canonical", &u("/headers.html"), HOME));
    assert!(has(
        "HTTP Refresh",
        &u("/headers.html"),
        &u("/refresh-target.html")
    ));
    // 3xx-Location, Meta-Refresh
    assert!(has("HTTP Redirect", &u("/old"), &u("/target.html")));
    assert!(has(
        "Meta Refresh",
        &u("/meta.html"),
        &u("/meta-target.html")
    ));
}

// spec: OUT-031
#[tokio::test]
async fn link_position_follows_the_enclosing_element() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    let pos = |dest: &str| find(&all, HOME, &u(dest), "Hyperlink")[13].clone();
    assert_eq!(pos("/nav.html"), "Navigation");
    assert_eq!(pos("/header.html"), "Header");
    assert_eq!(pos("/aside.html"), "Aside");
    assert_eq!(pos("/footer.html"), "Footer");
    assert_eq!(pos("/dup.html"), "Content");
    // Link Path: absoluter XPath mit Positionsindizes in jedem Schritt
    let path = &find(&all, HOME, &u("/nav.html"), "Hyperlink")[12];
    assert_eq!(path, "/html[1]/body[1]/nav[1]/a[1]");
    assert!(
        all.iter().all(|r| r[14] == "HTML"),
        "Link Origin ohne Rendering"
    );
}

// spec: OUT-031
#[tokio::test]
async fn unresolvable_target_keeps_the_raw_attribute_value() {
    // mailto:/tel: haben keine Ziel-URL-Zeile; Destination ist dann der Rohwert (K-37).
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "roh.ospider");
    insert_url(
        &file,
        1,
        "https://site.test/",
        true,
        &[("status_code", "200")],
    );
    insert_url(
        &file,
        2,
        "https://site.test/ziel",
        true,
        &[("status_code", "200")],
    );
    insert_link(
        &file,
        1,
        1,
        None,
        "mailto:info@site.test",
        "Hyperlink",
        0,
        &[("anchor", "'Mail'")],
    );
    insert_link(&file, 2, 1, None, "tel:+4912345", "Hyperlink", 1, &[]);
    // aufgelöst: Rohwert steht im Markup, Destination ist die Adresse der Ziel-URL
    insert_link(&file, 3, 1, Some(2), "/ziel#abschnitt", "Hyperlink", 2, &[]);
    let csv = bulk_csv(&file, OUTLINKS, dir.path());
    let r = rows(&csv);
    assert_eq!(r[0][2], "mailto:info@site.test");
    assert_eq!(
        (r[0][6].as_str(), r[0][7].as_str()),
        ("", ""),
        "ohne Ziel weder Code noch Status"
    );
    assert_eq!(r[1][2], "tel:+4912345");
    assert_eq!(r[2][2], "https://site.test/ziel");
    assert_eq!(r[2][6], "200");
}

// spec: OUT-031
#[tokio::test]
async fn every_link_type_of_the_spec_is_exported_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "typen.ospider");
    insert_url(
        &file,
        1,
        "https://site.test/",
        true,
        &[("status_code", "200")],
    );
    insert_url(
        &file,
        2,
        "https://site.test/z",
        true,
        &[("status_code", "200")],
    );
    let types = [
        "Hyperlink",
        "Image",
        "CSS",
        "JavaScript",
        "Media",
        "Iframe",
        "Canonical",
        "HTTP Canonical",
        "Rel Next",
        "Rel Prev",
        "Hreflang",
        "HTTP Hreflang",
        "AMP",
        "Mobile Alternate",
        "Meta Refresh",
        "HTTP Refresh",
        "HTTP Redirect",
        "HSTS Policy",
        "JavaScript Redirect",
        "XML Sitemap",
        "PDF Link",
    ];
    for (i, t) in types.iter().enumerate() {
        insert_link(&file, i as i64 + 1, 1, Some(2), "/z", t, i as i64, &[]);
    }
    let got: Vec<String> = rows(&bulk_csv(&file, OUTLINKS, dir.path()))
        .into_iter()
        .map(|r| r[0].clone())
        .collect();
    assert_eq!(got, types, "alle Typen der Spec, in Dokumentreihenfolge");
}

// spec: OUT-031
#[tokio::test]
async fn inlinks_are_sorted_by_destination_then_source_then_document_order() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    let key = |r: &Vec<String>| (r[2].clone(), r[1].clone());
    let keys: Vec<_> = all.iter().map(key).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "nach Destination, dann Source (bytewise)");
    // Gleichstand: Dokumentreihenfolge (Canonical steht vor Hreflang der Startseite)
    let home_self: Vec<&str> = all
        .iter()
        .filter(|r| r[1] == HOME && r[2] == HOME)
        .map(|r| r[0].as_str())
        .collect();
    assert_eq!(home_self, ["Canonical", "Hreflang"]);
    // die beiden gleichen Links stehen in Dokumentreihenfolge (a[1] vor a[2])
    let dup: Vec<&str> = all
        .iter()
        .filter(|r| r[1] == HOME && r[2] == u("/dup.html"))
        .map(|r| r[12].as_str())
        .collect();
    assert!(
        dup[0].ends_with("a[1]") && dup[1].ends_with("a[2]"),
        "{dup:?}"
    );
}

// spec: OUT-032
#[tokio::test]
async fn outlinks_are_sorted_by_source_and_keep_the_document_order_within_a_source() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, OUTLINKS, out.path()));
    let sources: Vec<&str> = all.iter().map(|r| r[1].as_str()).collect();
    let mut sorted = sources.clone();
    sorted.sort();
    assert_eq!(sources, sorted, "nach Source");
    // Reihenfolge innerhalb einer Quelle = Reihenfolge im Quelltext, wie sie im Store steht
    let conn = fx.file.conn();
    let positions: Vec<i64> = conn
        .prepare(
            "SELECT l.position FROM links l JOIN urls s ON s.id = l.source_id \
             WHERE s.address = 'http://www.site.test/' ORDER BY l.position",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let home: Vec<&Vec<String>> = all.iter().filter(|r| r[1] == HOME).collect();
    assert_eq!(home.len(), positions.len());
    // die ersten Zeilen sind die Elemente aus dem Kopf in Dokumentreihenfolge
    assert_eq!(home[0][0], "CSS");
    assert_eq!(home[1][0], "JavaScript");
    assert_eq!(home[2][2], u("/header.html"));
}

// spec: OUT-032
#[tokio::test]
async fn outlinks_of_each_page_match_the_outlinks_detail_export() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, OUTLINKS, out.path()));
    let sources: BTreeSet<String> = all.iter().map(|r| r[1].clone()).collect();
    assert!(sources.len() >= 5);
    for source in sources {
        let page = queries::url_by_address(fx.file.conn(), &source)
            .unwrap()
            .unwrap();
        let path = out.path().join("detail.csv");
        export_detail(
            &fx.file,
            DetailKind::Outlinks,
            &[page.id],
            &path,
            Format::Csv,
        )
        .unwrap();
        let detail = rows(&read_csv(&path));
        let bulk: Vec<Vec<String>> = all.iter().filter(|r| r[1] == source).cloned().collect();
        assert_eq!(detail, bulk, "Outlinks von {source}");
    }
}

// spec: OUT-031
#[tokio::test]
async fn store_off_means_no_rows_of_that_link_type() {
    // Speichern aus: weder Zeilen noch Spaltenwerte (CRAWL-038/046)
    let mut cfg = links_config();
    cfg.spider.images = StoreCrawl::OFF;
    cfg.spider.canonicals = StoreCrawl::OFF;
    let fx =
        os_engine::testing::crawl_fixture_from("out-links", "http://www.site.test/", cfg).await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    assert!(all.iter().all(|r| r[0] != "Image"));
    assert!(all
        .iter()
        .all(|r| r[0] != "Canonical" && r[0] != "HTTP Canonical"));
    assert!(all.iter().any(|r| r[0] == "Hyperlink"));
}

// spec: OUT-031
#[tokio::test]
async fn xlsx_has_the_same_cells_with_numbers_where_the_format_has_numbers() {
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let infos = export_bulk(
        &fx.file,
        &CrawlConfig::default(),
        INLINKS,
        out.path(),
        Format::Xlsx,
    )
    .unwrap();
    assert_eq!(infos[0].path.file_name().unwrap(), "links_all_inlinks.xlsx");
    let mut wb: Xlsx<_> = open_workbook(&infos[0].path).unwrap();
    let range = wb.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!(range.width(), 15);
    let csv = parse_csv(&bulk_csv(&fx.file, INLINKS, out.path()));
    assert_eq!(range.height(), csv.len());
    // Zeile 1: Extern -> Status Code 404 als Zahl, Follow als Text
    assert!(matches!(
        range.get((1, 6)),
        Some(Data::Int(404) | Data::Float(_))
    ));
    assert!(matches!(range.get((1, 8)), Some(Data::String(s)) if s == "True"));
    assert!(
        matches!(range.get((1, 3)), Some(Data::Int(_) | Data::Float(_))),
        "Size (Bytes)"
    );
    assert_eq!(wb.sheet_names()[0], "Links All Inlinks");
}

// spec: OUT-031
#[tokio::test]
async fn catalog_names_resolve_without_regard_to_case() {
    let e = bulk_links::resolve(" links : all inlinks ").unwrap();
    assert_eq!(e.name, INLINKS);
    assert!(matches!(
        bulk_links::resolve("Links:All Inlink"),
        Err(os_analyze::NameError::Unknown { ref suggestions, .. }) if suggestions == &[INLINKS.to_string()]
    ));
    // Datei- und Spaltenzähler passen zu den Abfragen des Stores: gleiche Menge wie die Detailabfragen
    let fx = crawl_links().await;
    let out = tempfile::tempdir().unwrap();
    let all = rows(&bulk_csv(&fx.file, INLINKS, out.path()));
    let total: i64 = fx
        .file
        .conn()
        .query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        all.len() as i64,
        total,
        "jeder Verweis des Crawls genau einmal"
    );
    let home = queries::url_by_address(fx.file.conn(), HOME)
        .unwrap()
        .unwrap();
    let page = queries::inlinks(fx.file.conn(), home.id, queries::PageReq::new(0, 500)).unwrap();
    let from_bulk = all.iter().filter(|r| r[2] == HOME).count();
    assert_eq!(page.total as usize, from_bulk);
    let _ = UrlId(0);
}

// spec: OUT-050
// Infrastruktur (M2-F05a) für OUT-050; Fachprüfung M2-D12.
#[tokio::test]
async fn error_inlinks_writer_orders_missing_status_first_then_status_destination_and_source() {
    use os_export::types::{ExportCtx, TableWriter, WriterOptions};
    let fx = crawl_links().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ordered.csv");
    let cfg = links_config();
    let tabs = os_analyze::tabs::configured(&cfg);
    let mut entry = bulk_links::entries()
        .iter()
        .find(|e| e.name == INLINKS)
        .unwrap()
        .clone();
    entry.order = os_export::links_export::LinkOrder::ErrorInlinks;
    let job = os_export::catalog::TableJob {
        entry_name: &entry.name,
        tabs: &tabs,
        file: &fx.file,
        cfg: &cfg,
    };
    let token = os_model::CancelToken::default();
    let progress = std::cell::Cell::new(0);
    let report = |n| progress.set(n);
    let mut ctx = ExportCtx {
        cancel: &token,
        progress: &report,
        skip_empty: false,
        workbook: None,
        skipped: vec![],
    };
    let mut writer = os_export::csv::CsvWriter::create(&path, &WriterOptions::default()).unwrap();
    let n =
        os_export::links_export::export_link_entry_to(&entry, &job, &mut writer, &mut ctx).unwrap();
    Box::new(writer).finish().unwrap();
    assert_eq!(progress.get(), n);
    let rows = rows(&read_csv(&path));
    assert_eq!(rows.len() as u64, n);
    let keys: Vec<_> = rows
        .iter()
        .map(|r| (r[6].parse::<u16>().ok(), r[2].as_str(), r[1].as_str()))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]));
    assert!(keys.first().unwrap().0.is_none());
    assert!(keys.last().unwrap().0.is_some());
}
