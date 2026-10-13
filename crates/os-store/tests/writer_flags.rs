//! `SqliteSink`: `WriteBatch.url_flags` und Sitemap-hreflang (M1.md 3.11, M1-W25).
//!
//! * Flags (`redirect_loop`, `redirect_limit_reached`, …) landen in derselben Transaktion in `url_flags`,
//!   stempeln `change_seq` der URL (K-20) und werden von `PageWrite.replace` – nur die `redirect_*`-Flags –
//!   zurückgesetzt.
//! * `SitemapWrite.hreflang` schreibt nach `hreflang` mit `source = 'sitemap'`, `sitemap_id` und
//!   `position = order`; `PageWrite.replace` löscht diese Zeilen nicht.

mod store_common;

use os_model::*;
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn open_file(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

fn flag(id: i64, name: &str, value: Option<&str>) -> (UrlId, String, Option<String>) {
    (UrlId(id), name.to_string(), value.map(str::to_string))
}

fn flags_of(file: &CrawlFile, id: i64) -> Vec<(String, Option<String>)> {
    let mut stmt = file
        .conn()
        .prepare("SELECT flag, value FROM url_flags WHERE url_id = ?1 ORDER BY flag")
        .unwrap();
    stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn sitemap_hreflang(code: &str, href: &str, order: u32, target: Option<i64>) -> HreflangRow {
    HreflangRow {
        target_id: target.map(UrlId),
        sitemap_id: None,
        r: HreflangRef {
            source: HintSource::Sitemap,
            code_raw: code.to_string(),
            code_norm: code.to_ascii_lowercase(),
            code_valid: true,
            invalid_reason: None,
            href_raw: href.to_string(),
            resolved: Some(format!("https://site.test{href}")),
            in_head: false,
            order,
        },
    }
}

fn sitemap(id: i64, hreflang: Vec<(UrlId, HreflangRow)>) -> SitemapWrite {
    SitemapWrite {
        id: SitemapId(id),
        url: format!("https://site.test/sitemap{id}.xml"),
        url_id: None,
        parent: None,
        kind: SitemapKind::Urlset,
        entries: 1,
        invalid_entries: 0,
        size_bytes: 10,
        status_code: Some(200),
        depth: 0,
        found_via: "config".into(),
        entry_urls: vec![(UrlId(1), "/p1".into(), None)],
        hreflang,
    }
}

// spec: CRAWL-017
// spec: CRAWL-072
#[test]
fn url_flags_are_written_in_the_batch_transaction_and_stamp_change_seq() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true), (3, true)]))
        .unwrap();
    let file = open_file(&path);
    let before = meta_change_seq(&file);

    sink.write(WriteBatch {
        url_flags: vec![
            flag(1, "redirect_loop", None),
            flag(2, "redirect_loop", None),
            flag(2, "redirect_limit_reached", Some("5")),
        ],
        ..WriteBatch::default()
    })
    .unwrap();

    assert_eq!(
        flags_of(&file, 1),
        vec![("redirect_loop".to_string(), None)]
    );
    assert_eq!(
        flags_of(&file, 2),
        vec![
            ("redirect_limit_reached".to_string(), Some("5".to_string())),
            ("redirect_loop".to_string(), None)
        ]
    );
    assert!(flags_of(&file, 3).is_empty());
    // Ein Batch nur mit Flags ist kein No-op: Sequenz steigt, betroffene URLs sind gestempelt.
    let after = meta_change_seq(&file);
    assert_eq!(after, before + 1);
    assert_eq!(change_seq_of(&file, 1), after);
    assert_eq!(change_seq_of(&file, 2), after);
    assert_eq!(change_seq_of(&file, 3), before, "URL 3 blieb unberührt");

    // Dasselbe Flag erneut: INSERT OR REPLACE, kein Duplikat, neuer Wert.
    sink.write(WriteBatch {
        url_flags: vec![flag(2, "redirect_limit_reached", Some("7"))],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM url_flags WHERE url_id = 2 AND flag = 'redirect_limit_reached'"
        ),
        1
    );
    let value: String = file
        .conn()
        .query_row(
            "SELECT value FROM url_flags WHERE url_id = 2 AND flag = 'redirect_limit_reached'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(value, "7");
}

// spec: CRAWL-017
#[test]
fn flag_for_an_unknown_url_rolls_the_whole_batch_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let file = open_file(&path);

    let mut bad = batch_with_urls([(2, true)]);
    bad.url_flags = vec![
        flag(1, "redirect_loop", None),
        flag(99, "redirect_loop", None),
    ];
    let err = sink.write(bad).unwrap_err();
    assert!(
        matches!(err, os_store::StoreError::InvalidInput(_)),
        "{err}"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM url_flags"), 0);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 1, "URL 2 fehlt");
}

// spec: CRAWL-017
#[test]
fn replace_clears_only_the_redirect_flags_of_that_url() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut b = batch_with_urls([(1, true), (2, true)]);
    b.pages.push(page(1, vec![]));
    b.pages.push(page(2, vec![]));
    b.url_flags = vec![
        flag(1, "redirect_loop", None),
        flag(1, "redirect_limit_reached", None),
        flag(1, "in_sitemap", None),
        flag(2, "redirect_loop", None),
    ];
    sink.write(b).unwrap();
    let file = open_file(&path);
    assert_eq!(flags_of(&file, 1).len(), 3);

    // Erneuter Abruf von URL 1: Weiterleitungs-Flags weg, andere Flags und fremde URLs bleiben.
    let mut again = page(1, vec![]);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(
        flags_of(&file, 1),
        vec![("in_sitemap".to_string(), None)],
        "nur redirect_* wird zurückgesetzt"
    );
    assert_eq!(flags_of(&file, 2).len(), 1);

    // Flags im selben Batch wie die Ersetzung gelten (der Writer löscht zuerst, schreibt danach).
    let mut third = page(2, vec![]);
    third.replace = true;
    sink.write(WriteBatch {
        pages: vec![third],
        url_flags: vec![flag(2, "redirect_loop", None)],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(
        flags_of(&file, 2),
        vec![("redirect_loop".to_string(), None)]
    );
}

// spec: TECH-210
#[test]
fn sitemap_hreflang_is_stored_with_source_sitemap_id_and_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut b = batch_with_urls([(1, true), (2, true)]);
    b.sitemaps.push(sitemap(
        3,
        vec![
            (UrlId(1), sitemap_hreflang("de", "/p1", 0, Some(1))),
            (UrlId(1), sitemap_hreflang("fr", "/p2", 1, Some(2))),
            (UrlId(2), sitemap_hreflang("de", "/p1", 0, Some(1))),
        ],
    ));
    sink.write(b).unwrap();
    let file = open_file(&path);

    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM hreflang WHERE source = 'sitemap' AND sitemap_id = 3"
        ),
        3
    );
    let rows: Vec<(i64, i64, String, Option<i64>)> = {
        let mut stmt = file
            .conn()
            .prepare("SELECT url_id, position, code_norm, target_id FROM hreflang ORDER BY url_id, position")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        rows,
        vec![
            (1, 0, "de".to_string(), Some(1)),
            (1, 1, "fr".to_string(), Some(2)),
            (2, 0, "de".to_string(), Some(1)),
        ]
    );

    // Dieselbe Sitemap erneut: ersetzt ihre Angaben, keine Duplikate.
    let mut again = WriteBatch::default();
    again.sitemaps.push(sitemap(
        3,
        vec![(UrlId(1), sitemap_hreflang("de", "/p1", 0, Some(1)))],
    ));
    sink.write(again).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM hreflang"), 1);
}

// spec: TECH-210
#[test]
fn page_replace_keeps_sitemap_hreflang_and_removes_page_hreflang() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);

    let mut first = batch_with_urls([(1, true), (2, true)]);
    first.sitemaps.push(sitemap(
        1,
        vec![(UrlId(1), sitemap_hreflang("fr", "/p2", 5, Some(2)))],
    ));
    let mut p = page(1, vec![]);
    p.hreflang.push(HreflangRow {
        target_id: Some(UrlId(2)),
        sitemap_id: None,
        r: HreflangRef {
            source: HintSource::Html,
            code_raw: "de".into(),
            code_norm: "de".into(),
            code_valid: true,
            invalid_reason: None,
            href_raw: "/p2".into(),
            resolved: Some("https://site.test/p2".into()),
            in_head: true,
            order: 0,
        },
    });
    first.pages.push(p);
    sink.write(first).unwrap();
    let file = open_file(&path);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM hreflang WHERE url_id = 1"),
        2
    );

    let mut replace = page(1, vec![]);
    replace.replace = true;
    sink.write(WriteBatch {
        pages: vec![replace],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM hreflang WHERE url_id = 1 AND source = 'html'"
        ),
        0,
        "Seitenangaben werden beim erneuten Abruf entfernt"
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM hreflang WHERE url_id = 1 AND source = 'sitemap' AND position = 5"
        ),
        1,
        "Sitemap-Angaben bleiben"
    );
}
