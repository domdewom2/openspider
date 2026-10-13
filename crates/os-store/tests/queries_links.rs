//! `queries`: Inlinks/Outlinks mit Paging und Sortierung, Header, Weiterleitungsketten, URL-Suche (P07).
//! Lesefunktionen für Export, MCP und Engine; die Standardreihenfolge folgt OUT-031/OUT-032, daher ohne
//! eigene Spec-ID.

mod store_common;

use os_model::*;
use os_store::queries::{self, LinkSortKey, PageReq};
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

fn reader(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

/// Ziel 1 (`/p1`) wird von mehreren Seiten verlinkt; Adressen sortieren sich anders als die Kennungen.
fn graph(dir: &std::path::Path) -> (std::path::PathBuf, os_store::SqliteSink) {
    let path = new_crawl(dir, "q.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    // Quellen: 10 (/p10), 2 (/p2), 3 (/p3), Ziele: 1, externe 20.
    let mut batch = batch_with_urls([(1, true), (2, true), (3, true), (10, true), (20, false)]);
    let mut l = hyperlink(1, 0);
    l.anchor = Some("zebra".into());
    l.alt_text = Some("Bild".into());
    let mut own = hyperlink(1, 1);
    own.anchor = Some("Äpfel".into());
    own.rel = Some("nofollow".into());
    own.follow = false;
    own.target_attr = Some("_blank".into());
    batch.pages.push(page(10, vec![l, own, hyperlink(20, 2)]));
    let mut a = hyperlink(1, 0);
    a.anchor = Some("apfel".into());
    batch
        .pages
        .push(page(2, vec![a, link(Some(1), LinkType::Image, 1)]));
    let mut z = hyperlink(1, 0);
    z.anchor = None;
    batch.pages.push(page(3, vec![z]));
    sink.write(batch).unwrap();
    (path, sink)
}

#[test]
fn inlinks_default_order_is_source_address_then_document_position() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let page = queries::inlinks(file.conn(), UrlId(1), PageReq::new(0, 50)).unwrap();
    assert_eq!(page.total, 5);
    let order: Vec<(String, LinkType, Option<String>)> = page
        .items
        .iter()
        .map(|l| (l.source_address.clone(), l.link_type, l.anchor.clone()))
        .collect();
    // Adressen bytewise: /p10 < /p2 < /p3
    assert_eq!(order[0].0, "https://site.test/p10");
    assert_eq!(order[1].0, "https://site.test/p10");
    assert_eq!(order[0].2.as_deref(), Some("zebra"));
    assert_eq!(order[1].2.as_deref(), Some("Äpfel"));
    assert_eq!(order[2].0, "https://site.test/p2");
    assert_eq!(order[2].1, LinkType::Hyperlink);
    assert_eq!(order[3].1, LinkType::Image);
    assert_eq!(order[4].0, "https://site.test/p3");
    let first = &page.items[1];
    assert!(!first.follow);
    assert_eq!(first.target_id, Some(UrlId(1)));
    assert_eq!(first.target_address, "https://site.test/p1");
    assert_eq!(first.status_code, None, "Ziel nicht gecrawlt");
    assert_eq!(first.status.as_deref(), Some("Not Crawled"));
}

#[test]
fn inlinks_paging_without_overlap_and_total_stays() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let all = queries::inlinks(file.conn(), UrlId(1), PageReq::new(0, 500)).unwrap();
    let mut paged = Vec::new();
    for offset in [0u64, 2, 4, 6] {
        let p = queries::inlinks(file.conn(), UrlId(1), PageReq::new(offset, 2)).unwrap();
        assert_eq!(p.total, 5);
        assert!(p.items.len() <= 2);
        paged.extend(p.items);
    }
    assert_eq!(paged, all.items);
    // limit außerhalb von 1..=500 wird begrenzt.
    let none = queries::inlinks(file.conn(), UrlId(1), PageReq::new(0, 0)).unwrap();
    assert_eq!(none.items.len(), 1);
}

#[test]
fn inlinks_sorted_by_anchor_uses_os_text_with_empty_values_last() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let mut req = PageReq::new(0, 50);
    req.sort = vec![LinkSortKey {
        column: "Anchor".into(),
        desc: false,
    }];
    let asc: Vec<Option<String>> = queries::inlinks(file.conn(), UrlId(1), req.clone())
        .unwrap()
        .items
        .into_iter()
        .map(|l| l.anchor)
        .collect();
    // OS_TEXT (Grundbuchstaben vor Akzenten, Groß/Klein gleichwertig): Anker 1 < apfel < Äpfel < zebra;
    // der fehlende Anker steht zuletzt.
    let texts: Vec<Option<&str>> = asc.iter().map(|a| a.as_deref()).collect();
    assert_eq!(
        texts,
        vec![
            Some("Anker 1"),
            Some("apfel"),
            Some("Äpfel"),
            Some("zebra"),
            None
        ]
    );

    req.sort = vec![LinkSortKey {
        column: "anchor".into(),
        desc: true,
    }];
    let desc: Vec<Option<String>> = queries::inlinks(file.conn(), UrlId(1), req)
        .unwrap()
        .items
        .into_iter()
        .map(|l| l.anchor)
        .collect();
    assert_eq!(desc[0].as_deref(), Some("zebra"));
    assert_eq!(
        desc.last().unwrap(),
        &None,
        "leere Werte auch absteigend am Ende"
    );
}

#[test]
fn inlinks_sorted_by_type_then_source() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let mut req = PageReq::new(0, 50);
    req.sort = vec![LinkSortKey {
        column: "Type".into(),
        desc: true,
    }];
    let items = queries::inlinks(file.conn(), UrlId(1), req).unwrap().items;
    assert_eq!(items[0].link_type, LinkType::Image);
    // Gleichstand beim Typ: nach Quelladresse (bytewise), dann Link-Kennung.
    let hyper: Vec<&str> = items[1..]
        .iter()
        .map(|l| l.source_address.as_str())
        .collect();
    assert_eq!(
        hyper,
        vec![
            "https://site.test/p10",
            "https://site.test/p10",
            "https://site.test/p2",
            "https://site.test/p3"
        ]
    );
}

#[test]
fn every_column_of_the_standard_link_format_is_sortable() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let columns = [
        "Type",
        "Source",
        "Destination",
        "Size (Bytes)",
        "Alt Text",
        "Anchor",
        "Status Code",
        "Status",
        "Follow",
        "Target",
        "Rel",
        "Path Type",
        "Link Path",
        "Link Position",
        "Link Origin",
    ];
    for column in columns {
        for desc in [false, true] {
            let mut req = PageReq::new(0, 50);
            req.sort = vec![LinkSortKey {
                column: column.to_string(),
                desc,
            }];
            let inl = queries::inlinks(file.conn(), UrlId(1), req.clone())
                .unwrap_or_else(|e| panic!("inlinks nach {column}: {e}"));
            assert_eq!(inl.items.len(), 5, "{column}");
            let out = queries::outlinks(file.conn(), UrlId(10), req)
                .unwrap_or_else(|e| panic!("outlinks nach {column}: {e}"));
            assert_eq!(out.items.len(), 3, "{column}");
        }
    }
    // Zwei Schlüssel hintereinander: erst Follow (False zuerst), dann Anchor.
    let mut req = PageReq::new(0, 50);
    req.sort = vec![
        LinkSortKey {
            column: "Follow".into(),
            desc: false,
        },
        LinkSortKey {
            column: "Anchor".into(),
            desc: false,
        },
    ];
    let items = queries::inlinks(file.conn(), UrlId(1), req).unwrap().items;
    assert!(!items[0].follow);
    assert!(items[1..].iter().all(|l| l.follow));
}

#[test]
fn unknown_sort_column_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let mut req = PageReq::new(0, 10);
    req.sort = vec![LinkSortKey {
        column: "Gibt es nicht".into(),
        desc: false,
    }];
    assert!(queries::inlinks(file.conn(), UrlId(1), req).is_err());
}

#[test]
fn outlinks_default_order_is_document_order_and_include_external_targets() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _sink) = graph(dir.path());
    let file = reader(&path);
    let page = queries::outlinks(file.conn(), UrlId(10), PageReq::default()).unwrap();
    assert_eq!(page.total, 3);
    let targets: Vec<&str> = page
        .items
        .iter()
        .map(|l| l.target_address.as_str())
        .collect();
    assert_eq!(
        targets,
        vec![
            "https://site.test/p1",
            "https://site.test/p1",
            "https://other.test/p20"
        ]
    );
    assert!(page.items.iter().all(|l| l.source_id == UrlId(10)));
    assert_eq!(page.items[2].anchor.as_deref(), Some("Anker 2"));

    let mut req = PageReq::new(0, 10);
    req.sort = vec![LinkSortKey {
        column: "Destination".into(),
        desc: true,
    }];
    let sorted = queries::outlinks(file.conn(), UrlId(10), req)
        .unwrap()
        .items;
    // Absteigend nach Zieladresse: „https://site…“ vor „https://other…“.
    assert_eq!(sorted[0].target_address, "https://site.test/p1");
    assert_eq!(sorted[1].target_address, "https://site.test/p1");
    assert_eq!(sorted[2].target_address, "https://other.test/p20");
    // Gleichstand (zweimal /p1): Link-Kennung, also Dokumentreihenfolge.
    assert_eq!(sorted[0].anchor.as_deref(), Some("zebra"));
}

#[test]
fn link_status_comes_from_the_target_row() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = graph(dir.path());
    let mut batch = WriteBatch::default();
    let mut target = page(1, vec![]);
    target.fetch.status_code = 404;
    target.fetch.status = "Not Found".into();
    batch.pages.push(target);
    sink.write(batch).unwrap();
    let file = reader(&path);
    let items = queries::inlinks(file.conn(), UrlId(1), PageReq::new(0, 50))
        .unwrap()
        .items;
    assert!(items.iter().all(|l| l.status_code == Some(404)));
    assert!(items
        .iter()
        .all(|l| l.status.as_deref() == Some("Not Found")));
}

#[test]
fn url_by_address_and_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut sink) = graph(dir.path());
    let mut batch = WriteBatch::default();
    let mut p = page(2, vec![]);
    p.indexability = Indexability::NonIndexable;
    p.indexability_status = Some(IndexabilityStatus::Noindex);
    batch.pages.push(p);
    sink.write(batch).unwrap();
    let file = reader(&path);
    let row = queries::url_by_address(file.conn(), "https://site.test/p2")
        .unwrap()
        .unwrap();
    assert_eq!(row.id, UrlId(2));
    assert!(row.is_internal);
    assert_eq!(row.state, UrlState::Done);
    assert_eq!(row.status_code, Some(200));
    assert_eq!(row.content_class, ContentClass::Html);
    assert_eq!(row.indexability, Some(Indexability::NonIndexable));
    assert_eq!(row.indexability_status, Some(IndexabilityStatus::Noindex));
    assert_eq!(
        queries::url_by_id(file.conn(), UrlId(2)).unwrap().unwrap(),
        row
    );
    assert!(
        queries::url_by_address(file.conn(), "https://site.test/none")
            .unwrap()
            .is_none()
    );
    let queued = queries::url_by_address(file.conn(), "https://site.test/p1")
        .unwrap()
        .unwrap();
    assert_eq!(queued.state, UrlState::Queued);
    assert_eq!(queued.status_code, None);
}

fn redirect(id: i64, to: Option<i64>, kind: RedirectType) -> PageWrite {
    let mut p = page(id, vec![]);
    p.fetch.status_code = 301;
    p.fetch.status = "Moved Permanently".into();
    p.fetch.redirect_target = to.map(UrlId);
    p.fetch.redirect_type = Some(kind);
    p
}

#[test]
fn redirect_chain_follows_hops_detects_loops_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "r.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls((1..=8).map(|i| (i, true)));
    // 1 -> 2 -> 3 (final 200); 4 -> 5 -> 4 (Schleife); 6 -> 7 -> 8 -> (kein Ziel)
    batch.pages.push(redirect(1, Some(2), RedirectType::Http));
    batch
        .pages
        .push(redirect(2, Some(3), RedirectType::MetaRefresh));
    batch.pages.push(page(3, vec![]));
    batch.pages.push(redirect(4, Some(5), RedirectType::Http));
    batch.pages.push(redirect(5, Some(4), RedirectType::Http));
    batch.pages.push(redirect(6, Some(7), RedirectType::Http));
    batch.pages.push(redirect(7, Some(8), RedirectType::Http));
    batch.pages.push(redirect(8, None, RedirectType::Http));
    sink.write(batch).unwrap();
    let file = reader(&path);
    let c = file.conn();

    let chain = queries::redirect_chain(c, UrlId(1), 10).unwrap();
    let ids: Vec<i64> = chain.hops.iter().map(|h| h.id.0).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(chain.hops[0].redirect_type, Some(RedirectType::Http));
    assert_eq!(chain.hops[1].redirect_type, Some(RedirectType::MetaRefresh));
    assert_eq!(chain.hops[2].status_code, Some(200));
    assert!(!chain.is_loop && !chain.limit_reached);

    let looped = queries::redirect_chain(c, UrlId(4), 10).unwrap();
    assert_eq!(looped.hops.len(), 2);
    assert!(looped.is_loop);

    let limited = queries::redirect_chain(c, UrlId(6), 2).unwrap();
    assert_eq!(limited.hops.len(), 2);
    assert!(limited.limit_reached && !limited.is_loop);
    let exact = queries::redirect_chain(c, UrlId(1), 3).unwrap();
    assert_eq!(exact.hops.len(), 3);
    assert!(
        !exact.limit_reached,
        "genau max_hops Glieder ohne weiteres Glied"
    );

    let unknown = queries::redirect_chain(c, UrlId(99), 5).unwrap();
    assert!(unknown.hops.is_empty());
}

#[test]
fn headers_come_back_request_first_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "h.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls([(1, true)]);
    let mut p = page(1, vec![]);
    p.headers = vec![
        HeaderRow {
            request: false,
            position: 1,
            name: "Server".into(),
            value: "x".into(),
        },
        HeaderRow {
            request: false,
            position: 0,
            name: "Content-Type".into(),
            value: "text/html".into(),
        },
        HeaderRow {
            request: true,
            position: 0,
            name: "Accept".into(),
            value: "*/*".into(),
        },
    ];
    batch.pages.push(p);
    sink.write(batch).unwrap();
    let file = reader(&path);
    let h = queries::headers(file.conn(), UrlId(1)).unwrap();
    let names: Vec<&str> = h.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Accept", "Content-Type", "Server"]);
    assert!(h[0].request && !h[1].request);
    assert!(queries::headers(file.conn(), UrlId(7)).unwrap().is_empty());
}
