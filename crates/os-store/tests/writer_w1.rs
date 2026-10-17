//! Infrastruktur (M2-F05a); fachliche Cookie-/Herkunftsauswertung folgt in E01/D17.
mod store_common;
use os_model::*;
use os_store::{queries, CrawlFile, CrawlSink, OpenMode, StorageProfile};
use store_common::*;

// spec: CRAWL-062
// Infrastruktur (M2-F05a) für CRAWL-062; Fachprüfung M2-E01.
#[test]
fn cookies_discoveries_and_compressed_html_roundtrip_and_replace() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "w1.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    let body = include_bytes!("../../../tests/fixtures/config/default.osconfig").to_vec();
    let mut p = page(1, vec![]);
    p.html = Some(PageHtml {
        encoding: Some("utf-8".into()),
        body: body.clone(),
    });
    sink.write(WriteBatch {
        pages: vec![p],
        cookies: vec![CookieWrite {
            url_id: UrlId(1),
            origin: CookieOrigin::Http,
            position: 0,
            name: "test".into(),
            value: "value".into(),
            raw: "test=value; Secure".into(),
            secure: true,
            domain: None,
            path: None,
            expires: None,
            max_age: None,
            http_only: false,
            same_site: None,
        }],
        discoveries: vec![DiscoveryWrite {
            seq: 9,
            target: UrlId(2),
            source: Some(UrlId(1)),
            kind: DiscoveryKind::Link,
            link_type: Some(LinkType::Hyperlink),
            anchor: Some("target".into()),
            depth: Some(1),
        }],
        ext: vec![ExtWrite {
            url: UrlId(1),
            key: ExtKey::parse("validation.checked").unwrap(),
            value: Value::Bool(true),
        }],
        ..Default::default()
    })
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(
        queries::stored_html(&file, UrlId(1))
            .unwrap()
            .unwrap()
            .bytes,
        body
    );
    assert_eq!(
        file.conn()
            .query_row("SELECT compression FROM page_html", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "zstd"
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM cookies WHERE secure = 1"),
        1
    );
    assert_eq!(
        count(&file, "SELECT source_id FROM discovery_edges WHERE seq = 9"),
        1
    );
    file.conn()
        .execute(
            "UPDATE page_html SET body = ?1, compression = NULL",
            [&body],
        )
        .unwrap();
    assert_eq!(
        queries::stored_html(&file, UrlId(1))
            .unwrap()
            .unwrap()
            .bytes,
        body
    );
    for namespace in ["spelling", "sdval", "a11y", "analysis", "api"] {
        file.conn()
            .execute(
                "INSERT INTO url_ext(url_id,key,v_int) VALUES(1,?1,1)",
                [format!("{namespace}.fixture")],
            )
            .unwrap();
    }
    let mut replacement = page(1, vec![]);
    replacement.replace = true;
    sink.write(WriteBatch {
        pages: vec![replacement],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM cookies"), 0);
    for namespace in ["spelling", "sdval", "a11y", "analysis", "api"] {
        assert_eq!(
            file.conn()
                .query_row(
                    "SELECT v_int FROM url_ext WHERE url_id=1 AND key=?1",
                    [format!("{namespace}.fixture")],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM url_ext WHERE key = 'validation.checked'"
        ),
        0
    );
}

// spec: INTEG-084
// Infrastruktur (M2-F05a) für INTEG-084; Fachprüfung M2-D16.
#[test]
fn link_position_filters_before_paging_and_retains_uncrawlable_facts() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "links.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    let mut first = hyperlink(2, 0);
    first.link_position = Some(LinkPosition::Custom("Menu".into()));
    first.pdf_page = Some(3);
    first.crawlable = false;
    first.uncrawlable_reason = Some(UncrawlableReason::AnchorReturnFalse);
    sink.write(WriteBatch {
        pages: vec![page(1, vec![first, hyperlink(2, 1)])],
        ..Default::default()
    })
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let mut req = queries::PageReq::new(0, 1);
    req.position = Some("Menu".into());
    let result = queries::inlinks(file.conn(), UrlId(2), req.clone()).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].pdf_page, Some(3));
    assert!(!result.items[0].crawlable);
    req.offset = 1;
    let result = queries::inlinks(file.conn(), UrlId(2), req).unwrap();
    assert_eq!(result.total, 1);
    assert!(result.items.is_empty());
    assert_eq!(
        count(&file, "SELECT inlinks FROM url_counters WHERE url_id = 2"),
        1
    );
}

// spec: TECH-005
// Infrastruktur (M2-F05a) für TECH-005; Fachprüfung M2-D04/D05/D07.
#[test]
fn changing_a_target_stamps_dependants_and_fact_updates_are_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "stamps.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    let mut source = page(1, vec![]);
    source.canonicals.push(CanonicalRow {
        target_id: Some(UrlId(2)),
        r: CanonicalRef {
            source: HintSource::Html,
            raw: "/p2".into(),
            resolved: Some("https://site.test/p2".into()),
            in_head: true,
            ..Default::default()
        },
    });
    sink.write(WriteBatch {
        pages: vec![source, page(2, vec![])],
        ..Default::default()
    })
    .unwrap();
    let mut file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let unchanged_before = queries::change_seq(file.conn()).unwrap();
    sink.write(WriteBatch {
        pages: vec![page(2, vec![])],
        ..Default::default()
    })
    .unwrap();
    assert!(!queries::changed_since(file.conn(), unchanged_before, 10)
        .unwrap()
        .contains(&UrlId(1)));
    let previous = queries::change_seq(file.conn()).unwrap();
    let mut target = page(2, vec![]);
    target.fetch.status_code = 404;
    sink.write(WriteBatch {
        pages: vec![target],
        ..Default::default()
    })
    .unwrap();
    assert!(queries::changed_since(file.conn(), previous, 10)
        .unwrap()
        .contains(&UrlId(1)));
    let changed = os_store::writer::apply_fact_updates(
        &mut file,
        &os_store::writer::FactUpdates {
            word_count: vec![(UrlId(1), Some(42))],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(changed, vec![UrlId(1)]);
    assert_eq!(count(&file, "SELECT word_count FROM urls WHERE id=1"), 42);
    let before = queries::change_seq(file.conn()).unwrap();
    let failed = os_store::writer::apply_fact_updates(
        &mut file,
        &os_store::writer::FactUpdates {
            word_count: vec![(UrlId(1), Some(99)), (UrlId(999), Some(1))],
            ..Default::default()
        },
    );
    assert!(failed.is_err());
    assert_eq!(count(&file, "SELECT word_count FROM urls WHERE id=1"), 42);
    assert_eq!(queries::change_seq(file.conn()).unwrap(), before);
}

// spec: TECH-219
// Infrastruktur (M2-F05a) für TECH-219; Fachprüfung M2-D13.
#[test]
fn sitemap_memberships_share_the_inlink_paging_stream_without_changing_counters() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "sitemap.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    sink.write(WriteBatch {
        pages: vec![page(1, vec![hyperlink(2, 0)])],
        ..Default::default()
    })
    .unwrap();
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn().execute("INSERT INTO sitemaps(id,url,kind,entries,invalid_entries,size_bytes,depth,found_via) VALUES(1,'https://site.test/sitemap.xml','urlset',1,0,100,0,'fixture')", []).unwrap();
    file.conn().execute("INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES(1,2,'https://site.test/p2')", []).unwrap();
    let first = queries::inlinks(file.conn(), UrlId(2), queries::PageReq::new(0, 1)).unwrap();
    let second = queries::inlinks(file.conn(), UrlId(2), queries::PageReq::new(1, 1)).unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(second.total, 2);
    let rows = [first.items[0].clone(), second.items[0].clone()];
    assert!(rows.iter().any(|row| row.link_type == LinkType::XmlSitemap));
    assert_eq!(
        count(&file, "SELECT inlinks FROM url_counters WHERE url_id = 2"),
        1
    );
}
