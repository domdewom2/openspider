//! `SqliteSink`: eine Transaktion je Batch, Reihenfolge, Kindzeilen, Queue, Sitemaps, Robots, Meta (P07).
//! Infrastruktur des Writers (CRAWL-124/OUT-230 stützen sich darauf), daher ohne eigene Spec-ID.

mod store_common;

use std::sync::Arc;
use std::time::Duration;

use os_model::time::FixedClock;
use os_model::*;
use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile, StoreError};
use store_common::*;

fn extracted(text: &str, in_head: bool) -> ExtractedText {
    ExtractedText {
        text: text.to_string(),
        len: u32::try_from(text.chars().count()).unwrap(),
        px: Some(300),
        in_head,
    }
}

fn open_file(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

#[test]
fn batch_writes_urls_page_facts_and_children() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);

    let mut batch = batch_with_urls([(1, true), (2, true), (3, false)]);
    let mut page = page(1, vec![hyperlink(2, 0), hyperlink(3, 1)]);
    page.fetch.redirect_url = Some("https://site.test/p2".into());
    page.fetch.redirect_target = Some(UrlId(2));
    page.fetch.redirect_type = Some(RedirectType::Http);
    page.fetch.hash_md5 = Some("abc".into());
    page.page = Some(PageFacts {
        title: ElementPair {
            count: 2,
            first: Some(extracted("Titel eins", true)),
            second: Some(extracted("Titel zwei", false)),
        },
        meta_description: ElementPair {
            count: 1,
            first: Some(extracted("Beschreibung", true)),
            second: None,
        },
        meta_keywords: ElementPair::default(),
        h1: ElementPair {
            count: 1,
            first: Some(extracted("Hauptüberschrift", false)),
            second: None,
        },
        h2: ElementPair::default(),
        word_count: Some(120),
        text_ratio: Some(0.25),
        canonical_count: 1,
        canonical_url: Some("https://site.test/p1".into()),
        canonical_target: Some(UrlId(1)),
        canonical_self: Some(true),
        effective_index: true,
        effective_follow: false,
        has_rel_next: false,
        has_rel_prev: true,
        hreflang_count: 1,
    });
    page.canonicals.push(CanonicalRow {
        target_id: Some(UrlId(1)),
        r: CanonicalRef {
            source: HintSource::Html,
            raw: "/p1".into(),
            resolved: Some("https://site.test/p1".into()),
            in_head: true,
            order: 0,
            xpath: None,
            ..Default::default()
        },
    });
    page.directives.push(DirectiveRow(Directive {
        source: DirectiveSource::MetaRobots,
        agent: None,
        raw: "noindex, nofollow".into(),
        tokens: vec!["noindex".into(), "nofollow".into()],
        applies: true,
        in_head: true,
        order: 0,
    }));
    page.refresh.push(RefreshRow {
        target_id: Some(UrlId(2)),
        r: RefreshRef {
            source: RefreshSource::Meta,
            raw: "0; url=/p2".into(),
            delay: Some(0),
            target_raw: Some("/p2".into()),
            target: Some("https://site.test/p2".into()),
            in_head: true,
            order: 0,
            xpath: None,
        },
    });
    page.hreflang.push(HreflangRow {
        target_id: Some(UrlId(2)),
        sitemap_id: None,
        r: HreflangRef {
            source: HintSource::Html,
            code_raw: "de-AT".into(),
            code_norm: "de-at".into(),
            code_valid: true,
            invalid_reason: None,
            href_raw: "/p2".into(),
            resolved: Some("https://site.test/p2".into()),
            in_head: true,
            order: 0,
            xpath: None,
        },
    });
    page.pagination.push(PaginationRow {
        target_id: Some(UrlId(2)),
        r: PaginationRef {
            rel_next: true,
            source: HintSource::Html,
            raw: "/p2".into(),
            resolved: Some("https://site.test/p2".into()),
            in_head: true,
            order: 0,
            xpath: None,
        },
    });
    page.images.push(ImageRow {
        image_id: Some(UrlId(3)),
        instance: ImageInstance {
            src_raw: "/p3".into(),
            resolved: Some("https://other.test/p3".into()),
            source: ImageSource::Img,
            alt_present: true,
            alt: Some("Hund".into()),
            alt_len: Some(4),
            width_attr: Some("10".into()),
            height_attr: None,
            xpath: "/html[1]/body[1]/img[1]".into(),
            doc_order: 7,
            ..Default::default()
        },
    });
    page.headers = vec![
        HeaderRow {
            request: false,
            position: 0,
            name: "Content-Type".into(),
            value: "text/html".into(),
        },
        HeaderRow {
            request: true,
            position: 0,
            name: "User-Agent".into(),
            value: "OpenSpider".into(),
        },
    ];
    batch.pages.push(page);
    sink.write(batch).unwrap();

    let file = open_file(&path);
    let c = file.conn();
    let col = |name: &str| -> rusqlite::types::Value {
        c.query_row(&format!("SELECT {name} FROM urls WHERE id = 1"), [], |r| {
            r.get(0)
        })
        .unwrap()
    };
    let text = |name: &str| -> String {
        match col(name) {
            rusqlite::types::Value::Text(t) => t,
            other => panic!("{name}: {other:?}"),
        }
    };
    let num = |name: &str| -> Option<i64> {
        match col(name) {
            rusqlite::types::Value::Integer(i) => Some(i),
            rusqlite::types::Value::Null => None,
            other => panic!("{name}: {other:?}"),
        }
    };
    let state = text("state");
    let status = num("status_code").unwrap();
    let class = text("content_class");
    let title_count = num("title_count").unwrap();
    let t1 = text("title_1");
    let t1px = num("title_1_px").unwrap();
    let t1head = num("title_1_in_head").unwrap();
    let t2 = text("title_2");
    let t2head = num("title_2_in_head").unwrap();
    let desc2 = col("meta_desc_2");
    let kw = num("meta_kw_count");
    let h1len = num("h1_1_len").unwrap();
    assert_eq!(state, "done");
    assert_eq!(status, 200);
    assert_eq!(class, "HTML");
    assert_eq!(title_count, 2);
    assert_eq!(t1, "Titel eins");
    assert_eq!(t1px, 300);
    assert_eq!(t1head, 1);
    assert_eq!(t2, "Titel zwei");
    assert_eq!(t2head, 0);
    assert_eq!(desc2, rusqlite::types::Value::Null);
    assert_eq!(kw, Some(0));
    assert_eq!(h1len, 16);
    let (ei, ef, rn, rp, canon_self, redirect): (i64, i64, i64, i64, i64, String) = c
        .query_row(
            "SELECT effective_index, effective_follow, has_rel_next, has_rel_prev, canonical_self, redirect_type \
             FROM urls WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();
    assert_eq!((ei, ef, rn, rp, canon_self), (1, 0, 0, 1, 1));
    assert_eq!(redirect, RedirectType::Http.as_str());

    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM links WHERE source_id = 1"),
        2
    );
    let tokens: String = c
        .query_row("SELECT tokens FROM directives WHERE url_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(tokens, "noindex,nofollow");
    let rel: String = c
        .query_row("SELECT rel FROM pagination WHERE url_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(rel, "next");
    let code: String = c
        .query_row("SELECT code_norm FROM hreflang WHERE url_id = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(code, "de-at");
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM canonicals WHERE url_id = 1"),
        1
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM refresh WHERE url_id = 1"),
        1
    );
    let (xpath, order): (String, i64) = c
        .query_row(
            "SELECT link_path, position FROM image_instances WHERE page_id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(xpath, "/html[1]/body[1]/img[1]");
    assert_eq!(order, 7);

    let headers = os_store::queries::headers(c, UrlId(1)).unwrap();
    assert_eq!(headers.len(), 2);
    assert!(headers[0].request, "Anfrage-Header zuerst");
    assert_eq!(headers[1].name, "Content-Type");
}

#[test]
fn non_html_page_resets_html_columns_to_null() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls([(1, true)]);
    batch.pages.push(page(1, vec![]));
    sink.write(batch).unwrap();
    let file = open_file(&path);
    let (title, canon, hreflang, idx): (Option<String>, i64, i64, Option<i64>) = file
        .conn()
        .query_row(
            "SELECT title_1, canonical_count, hreflang_count, effective_index FROM urls WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((title, canon, hreflang, idx), (None, 0, 0, None));
}

#[test]
fn url_updates_queue_sitemaps_robots_meta_events_counters() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let clock = Arc::new(FixedClock::new(
        Timestamp::from_db("2026-10-13T08:00:00.000Z").unwrap(),
        Duration::from_secs(1),
    ));
    let mut sink = sink(&path, StorageProfile::Balanced).with_clock(clock);

    let mut b1 = batch_with_urls([(1, true), (2, true), (3, true)]);
    b1.queue = vec![
        QueueOp::Push {
            id: UrlId(1),
            seq: 5,
            depth: Some(0),
            priority: 0,
        },
        QueueOp::Push {
            id: UrlId(2),
            seq: 6,
            depth: Some(1),
            priority: 0,
        },
        QueueOp::Push {
            id: UrlId(3),
            seq: 7,
            depth: None,
            priority: 1,
        },
    ];
    b1.meta = vec![("started_at".into(), "2026-10-13T08:00:00.000Z".into())];
    b1.counters = vec![("excluded_skipped".into(), 2)];
    b1.events = vec![CrawlEvent {
        at: created_at(),
        kind: "warn".into(),
        code: "regex_too_complex".into(),
        message: "Regel zu komplex".into(),
    }];
    sink.write(b1).unwrap();

    let b2 = WriteBatch {
        url_updates: vec![
            UrlUpdate::Depth {
                id: UrlId(2),
                depth: 0,
            },
            UrlUpdate::State {
                id: UrlId(3),
                state: UrlState::Discovered,
                reason: Some(NotCrawledReason::DepthLimit),
            },
            UrlUpdate::InTables {
                id: UrlId(1),
                value: false,
            },
        ],
        queue: vec![QueueOp::Pop { id: UrlId(1) }],
        sitemaps: vec![SitemapWrite {
            id: SitemapId(1),
            url: "https://site.test/sitemap.xml".into(),
            url_id: None,
            parent: None,
            kind: SitemapKind::Urlset,
            entries: 2,
            invalid_entries: 0,
            size_bytes: 321,
            status_code: Some(200),
            depth: 0,
            found_via: "robots".into(),
            entry_urls: vec![
                (UrlId(1), "/p1".into(), Some("2026-01-01".into())),
                (UrlId(2), "/p2".into(), None),
            ],
            hreflang: vec![],
            error: None,
            entry_meta: vec![],
        }],
        robots: vec![RobotsWrite {
            origin: "https://site.test".into(),
            robots_url: "https://site.test/robots.txt".into(),
            fetch_status: "ok".into(),
            status_code: Some(200),
            attempts: 1,
            body: Some("User-agent: *\nDisallow: /x".into()),
            note: None,
        }],
        counters: vec![("excluded_skipped".into(), 3), ("include_hits:1".into(), 1)],
        meta: vec![("heartbeat_at".into(), "2026-10-13T08:00:02.000Z".into())],
        ..WriteBatch::default()
    };
    sink.write(b2).unwrap();

    let file = open_file(&path);
    let (depth, in_tables): (i64, i64) = file
        .conn()
        .query_row(
            "SELECT crawl_depth, in_tables FROM urls WHERE id = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((depth, in_tables), (0, 1));
    let in_tables: i64 = file
        .conn()
        .query_row("SELECT in_tables FROM urls WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(in_tables, 0);
    let (state, reason): (String, String) = file
        .conn()
        .query_row(
            "SELECT state, not_crawled_reason FROM urls WHERE id = 3",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (state.as_str(), reason.as_str()),
        ("discovered", "depth_limit")
    );

    // Queue: URL 1 entnommen, Rest in Abrufreihenfolge.
    let resume = file.resume_state().unwrap();
    let order: Vec<i64> = resume.queue.iter().map(|q| q.id.0).collect();
    assert_eq!(order, vec![2, 3]);
    assert_eq!(resume.next_seq, 8);
    assert_eq!(resume.next_url_id, 4);

    assert_eq!(count(&file, "SELECT entries FROM sitemaps WHERE id = 1"), 2);
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM sitemap_entries WHERE sitemap_id = 1"
        ),
        2
    );
    let fetched: String = file
        .conn()
        .query_row("SELECT fetched_at FROM robots_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fetched, "2026-10-13T08:00:00.000Z");
    assert_eq!(
        file.get_meta("started_at").unwrap().unwrap(),
        "2026-10-13T08:00:00.000Z"
    );
    assert_eq!(
        file.get_meta("heartbeat_at").unwrap().unwrap(),
        "2026-10-13T08:00:02.000Z"
    );
    assert_eq!(
        count(
            &file,
            "SELECT value FROM crawl_counters WHERE key = 'excluded_skipped'"
        ),
        5
    );
    assert_eq!(
        count(
            &file,
            "SELECT value FROM crawl_counters WHERE key = 'include_hits:1'"
        ),
        1
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM crawl_events"), 1);

    // Sitemap erneut schreiben: alte Einträge werden ersetzt.
    let b3 = WriteBatch {
        sitemaps: vec![SitemapWrite {
            id: SitemapId(1),
            url: "https://site.test/sitemap.xml".into(),
            url_id: None,
            parent: None,
            kind: SitemapKind::Urlset,
            entries: 1,
            invalid_entries: 0,
            size_bytes: 100,
            status_code: Some(200),
            depth: 0,
            found_via: "robots".into(),
            entry_urls: vec![(UrlId(3), "/p3".into(), None)],
            hreflang: vec![],
            error: None,
            entry_meta: vec![],
        }],
        ..WriteBatch::default()
    };
    sink.write(b3).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM sitemap_entries"), 1);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM sitemaps"), 1);
}

#[test]
fn failing_batch_rolls_back_completely() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let before = meta_change_seq(&open_file(&path));

    // Zweite URL ist neu, die dritte kollidiert mit der Adresse von 1 → ganze Transaktion zurück.
    let mut bad = batch_with_urls([(2, true)]);
    let mut clash = new_url(3, true);
    clash.url = url_of(1, true);
    bad.new_urls.push(clash);
    bad.meta = vec![("x".into(), "y".into())];
    let err = sink.write(bad).unwrap_err();
    assert!(
        matches!(err, StoreError::InvalidInput(ref m) if m.contains("Adresse")),
        "{err:?}"
    );

    let file = open_file(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 1);
    assert_eq!(file.get_meta("x").unwrap(), None);
    assert_eq!(meta_change_seq(&file), before);

    // Seite für nicht vorhandene URL: Fehler, nichts bleibt zurück.
    let mut batch = batch_with_urls([(5, true)]);
    batch.pages.push(page(99, vec![hyperlink(1, 0)]));
    let err = sink.write(batch).unwrap_err();
    assert!(
        matches!(err, StoreError::InvalidInput(ref m) if m.contains("99")),
        "{err:?}"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 1);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM links"), 0);

    // Die Senke bleibt benutzbar.
    sink.write(batch_with_urls([(2, true)])).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 2);
}

#[test]
fn update_of_an_unknown_url_is_invalid_input_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "u.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let before = meta_change_seq(&open_file(&path));

    let mut batch = batch_with_urls([(2, true)]);
    batch.url_updates.push(UrlUpdate::Depth {
        id: UrlId(77),
        depth: 3,
    });
    let err = sink.write(batch).unwrap_err();
    assert!(
        matches!(err, StoreError::InvalidInput(ref m) if m.contains("77")),
        "{err:?}"
    );
    let file = open_file(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM urls"), 1);
    assert_eq!(meta_change_seq(&file), before);
}

#[test]
fn next_ids_follow_written_ids_even_if_engine_meta_lags() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls([(1, true), (7, true)]);
    batch.queue = vec![QueueOp::Push {
        id: UrlId(7),
        seq: 40,
        depth: Some(1),
        priority: 0,
    }];
    batch.meta = vec![("next_url_id".into(), "2".into())];
    sink.write(batch).unwrap();
    let state = open_file(&path).resume_state().unwrap();
    assert_eq!(state.next_url_id, 8);
    assert_eq!(state.next_seq, 41);
}

#[test]
fn change_seq_cannot_be_overwritten_through_meta() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    let mut batch = batch_with_urls([(1, true)]);
    batch.meta = vec![("change_seq".into(), "999".into())];
    sink.write(batch).unwrap();
    assert_eq!(meta_change_seq(&open_file(&path)), 1);
}

#[test]
fn empty_batch_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(WriteBatch::default()).unwrap();
    assert_eq!(meta_change_seq(&open_file(&path)), 0);
}

#[test]
fn checkpoint_merges_the_wal_and_keeps_writing_possible() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "a.ospider", StorageProfile::MaxSpeed);
    let mut sink = sink(&path, StorageProfile::MaxSpeed);
    sink.write(batch_with_urls([(1, true), (2, true)])).unwrap();
    sink.checkpoint().unwrap();
    // synchronous bleibt nach dem Checkpoint auf dem Wert des Profils.
    assert_eq!(
        os_store::profile::read(sink.file().conn())
            .unwrap()
            .synchronous,
        0
    );
    sink.write(batch_with_urls([(3, true)])).unwrap();
    sink.finish().unwrap();
    assert_eq!(
        os_store::profile::read(sink.file().conn())
            .unwrap()
            .synchronous,
        1
    );
    let wal = std::fs::metadata(format!("{}-wal", path.display())).map_or(0, |m| m.len());
    assert_eq!(wal, 0, "WAL nach finish() leer");
    assert_eq!(count(sink.file(), "SELECT COUNT(*) FROM urls"), 3);
}

fn wal_autocheckpoint_of(sink: &os_store::SqliteSink) -> i64 {
    sink.file()
        .conn()
        .pragma_query_value(None, "wal_autocheckpoint", |r| r.get(0))
        .unwrap()
}

// spec: CRAWL-125
#[test]
fn the_checkpoint_threshold_starts_at_256_mib_and_grows_with_the_file() {
    use os_store::profile::WAL_CHECKPOINT_MIN_PAGES;
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "wal.ospider", StorageProfile::Balanced);
    let sink0 = sink(&path, StorageProfile::Balanced);
    assert_eq!(wal_autocheckpoint_of(&sink0), WAL_CHECKPOINT_MIN_PAGES);
    assert_eq!(WAL_CHECKPOINT_MIN_PAGES * 4096, 256 * 1024 * 1024);
    drop(sink0);

    // Mit kleinen Grenzen (2 000 bis 6 000 Seiten) lässt sich das Mitwachsen mit einer kleinen Datei prüfen.
    let mut sink = sink(&path, StorageProfile::Balanced)
        .with_checkpoint_bounds(2_000, 6_000)
        .unwrap();
    assert_eq!(wal_autocheckpoint_of(&sink), 2_000);
    {
        let conn = sink.file().conn();
        conn.execute_batch("CREATE TABLE filler (a BLOB)").unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for _ in 0..4_000 {
            tx.execute("INSERT INTO filler VALUES (zeroblob(4000))", [])
                .unwrap();
        }
        tx.commit().unwrap();
    }
    sink.write(batch_with_urls([(1, true)])).unwrap();
    let grown = wal_autocheckpoint_of(&sink);
    assert!(
        (4_000..=6_000).contains(&grown),
        "Schwelle {grown} Seiten bei rund 4 000 Seiten Dateigröße"
    );
    // Über der Obergrenze bleibt sie dort.
    {
        let tx = sink.file().conn().unchecked_transaction().unwrap();
        for _ in 0..4_000 {
            tx.execute("INSERT INTO filler VALUES (zeroblob(4000))", [])
                .unwrap();
        }
        tx.commit().unwrap();
    }
    sink.write(batch_with_urls([(2, true)])).unwrap();
    assert_eq!(wal_autocheckpoint_of(&sink), 6_000);
}

// spec: CRAWL-125
#[test]
fn an_externally_set_checkpoint_threshold_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "wal2.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.file()
        .conn()
        .pragma_update(None, "wal_autocheckpoint", 5_000)
        .unwrap();
    sink.write(batch_with_urls([(1, true)])).unwrap();
    sink.write(batch_with_urls([(2, true)])).unwrap();
    assert_eq!(wal_autocheckpoint_of(&sink), 5_000);
}

// Die Tests des optionalen Hintergrund-Checkpoints (K-76) stehen in `bg_checkpoint.rs`.
