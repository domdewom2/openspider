//! Detail-Export Inlinks/Outlinks für URL-Listen (OUT-021, OUT-022, ONPAGE-192).

#[path = "links_support.rs"]
mod support;

use std::time::Instant;

use calamine::{open_workbook, Reader, Xlsx};
use os_export::{export_detail, DetailKind, Format};
use os_model::UrlId;
use os_store::{queries, CrawlFile};
use support::*;

const HEADER: &str = "Type,Source,Destination,Size (Bytes),Alt Text,Anchor,Status Code,Status,Follow,Target,Rel,Path Type,Link Path,Link Position,Link Origin";

fn id_of(file: &CrawlFile, address: &str) -> UrlId {
    queries::url_by_address(file.conn(), address)
        .unwrap()
        .unwrap_or_else(|| panic!("{address}"))
        .id
}

fn detail(
    file: &CrawlFile,
    kind: DetailKind,
    urls: &[UrlId],
    dir: &std::path::Path,
) -> Vec<Vec<String>> {
    let path = dir.join("detail.csv");
    let info = export_detail(file, kind, urls, &path, Format::Csv).unwrap();
    let csv = read_csv(&path);
    assert_eq!(csv.lines().next().unwrap(), HEADER);
    let rows: Vec<Vec<String>> = parse_csv(&csv).into_iter().skip(1).collect();
    assert_eq!(info.rows as usize, rows.len());
    rows
}

// spec: OUT-021
#[test]
fn inlinks_of_a_url_with_two_hyperlinks_and_a_canonical_give_three_rows() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "detail.ospider");
    insert_url(
        &file,
        1,
        "https://site.test/a",
        true,
        &[("status_code", "200")],
    );
    insert_url(
        &file,
        2,
        "https://site.test/b",
        true,
        &[("status_code", "200")],
    );
    insert_url(
        &file,
        3,
        "https://site.test/target",
        true,
        &[("status_code", "200")],
    );
    insert_link(
        &file,
        1,
        1,
        Some(3),
        "/target",
        "Hyperlink",
        0,
        &[("anchor", "'Eins'")],
    );
    insert_link(
        &file,
        2,
        2,
        Some(3),
        "/target",
        "Hyperlink",
        0,
        &[("anchor", "'Zwei'")],
    );
    insert_link(&file, 3, 2, Some(3), "/target", "Canonical", 1, &[]);
    let rows = detail(&file, DetailKind::Inlinks, &[UrlId(3)], dir.path());
    let types: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(types, ["Hyperlink", "Hyperlink", "Canonical"]);
    assert!(rows.iter().all(|r| r[2] == "https://site.test/target"));
    assert_eq!(rows[0][1], "https://site.test/a");
}

// spec: ONPAGE-192
#[tokio::test]
async fn inlinks_of_a_404_url_list_all_linking_pages_with_their_anchor_text() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    let e404 = id_of(&fx.file, "http://www.site.test/e404");
    let rows = detail(&fx.file, DetailKind::Inlinks, &[e404], dir.path());
    let got: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| (r[1].as_str(), r[5].as_str(), r[6].as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("http://www.site.test/", "404", "404"),
            (
                "http://www.site.test/second.html",
                "404 von der zweiten Seite",
                "404"
            ),
        ]
    );
}

// spec: OUT-022
#[tokio::test]
async fn selection_of_three_urls_with_two_inlinks_each_gives_six_rows_in_list_order() {
    let fx = crawl_codes().await;
    let dir = tempfile::tempdir().unwrap();
    // e404 (2 Inlinks), ext404 (2 Inlinks), ok.html (index, second, + Redirect/Meta-Refresh)
    let e404 = id_of(&fx.file, "http://www.site.test/e404");
    let ext = id_of(&fx.file, "http://external.test/ext404");
    let second = id_of(&fx.file, "http://www.site.test/second.html");
    let rows = detail(
        &fx.file,
        DetailKind::Inlinks,
        &[ext, e404, second],
        dir.path(),
    );
    // second.html hat einen Inlink (von der Startseite): 2 + 2 + 1
    assert_eq!(rows.len(), 5);
    let order: Vec<&str> = rows.iter().map(|r| r[2].as_str()).collect();
    assert_eq!(
        order,
        [
            "http://external.test/ext404",
            "http://external.test/ext404",
            "http://www.site.test/e404",
            "http://www.site.test/e404",
            "http://www.site.test/second.html"
        ],
        "Reihenfolge der URL-Liste"
    );
    // doppelte Einträge in der Auswahl zählen einmal
    let again = detail(
        &fx.file,
        DetailKind::Inlinks,
        &[ext, e404, ext, second, e404],
        dir.path(),
    );
    assert_eq!(again, rows);
}

// spec: OUT-022
#[test]
fn fifty_thousand_urls_are_streamed() {
    let dir = tempfile::tempdir().unwrap();
    let file = new_file(dir.path(), "viele.ospider");
    exec(
        &file,
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 50000) \
         INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length, status_code) \
         SELECT i, 'https://site.test/p' || i, 'https://site.test/p' || i, 'https://site.test', 1, 'done', 'link', 0, 20, 200 FROM n",
    );
    exec(
        &file,
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 50000) \
         INSERT INTO links (id, source_id, target_id, target_raw, link_type, position, follow) \
         SELECT i, 1, i, '/p' || i, 'Hyperlink', i, 1 FROM n",
    );
    let urls: Vec<UrlId> = (1..=50_000).map(UrlId).collect();
    let path = dir.path().join("viele.csv");
    let t = Instant::now();
    let info = export_detail(&file, DetailKind::Inlinks, &urls, &path, Format::Csv).unwrap();
    assert_eq!(info.rows, 50_000);
    assert!(t.elapsed().as_secs() < 60, "{:?}", t.elapsed());
    let out = export_detail(
        &file,
        DetailKind::Outlinks,
        &[UrlId(1)],
        &dir.path().join("o.csv"),
        Format::Csv,
    )
    .unwrap();
    assert_eq!(out.rows, 50_000, "alle Verweise der Seite 1");
}

// spec: ONPAGE-192
#[tokio::test]
async fn outlinks_of_a_page_are_listed_in_document_order() {
    let fx = crawl_links().await;
    let dir = tempfile::tempdir().unwrap();
    let home = id_of(&fx.file, "http://www.site.test/");
    let rows = detail(&fx.file, DetailKind::Outlinks, &[home], dir.path());
    assert!(rows.iter().all(|r| r[1] == "http://www.site.test/"));
    assert_eq!(rows[0][0], "CSS");
    assert_eq!(rows[2][5], "Kopf");
    let total: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM links WHERE source_id = ?1",
            [home.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows.len() as i64, total);
}

// spec: OUT-021
#[tokio::test]
async fn no_urls_or_unknown_urls_give_the_header_only_and_xlsx_works_too() {
    let fx = crawl_links().await;
    let dir = tempfile::tempdir().unwrap();
    assert!(detail(&fx.file, DetailKind::Inlinks, &[], dir.path()).is_empty());
    assert!(detail(
        &fx.file,
        DetailKind::Outlinks,
        &[UrlId(999_999)],
        dir.path()
    )
    .is_empty());

    let home = id_of(&fx.file, "http://www.site.test/");
    let path = dir.path().join("inlinks.xlsx");
    let info = export_detail(&fx.file, DetailKind::Inlinks, &[home], &path, Format::Xlsx).unwrap();
    assert_eq!(
        info.rows, 4,
        "Canonical, Hreflang, HTTP Canonical, Hyperlink"
    );
    let mut wb: Xlsx<_> = open_workbook(&path).unwrap();
    assert_eq!(wb.sheet_names()[0], "Inlinks");
    let range = wb.worksheet_range_at(0).unwrap().unwrap();
    assert_eq!((range.height(), range.width()), (5, 15));
}
