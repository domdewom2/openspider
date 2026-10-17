//! Writer der M2-Teile (Schema v3, `docs/plan/M2.md` 3.4): Rundreise je Tabelle, gleiche Transaktion wie die Seite,
//! `change_seq`-Stempel, Ersetzungsregeln und Fehlerfälle. Infrastruktur des Schreibpfads; die Fachbedeutung der Daten
//! prüfen die Fachpakete (D01 … D23).
//!
//! Spec: Die Tabellen tragen die Fakten von ONPAGE-034/035 (`forms`), ONPAGE-144 (`minhash`), CRAWL-060 (Sitemap-Rest),
//! CRAWL-063 (`page_html`), CRAWL-084 (`render_*`), TECH-180 (`sd_*`), TECH-250 (`a11y_*`), ONPAGE-153 (`spelling_findings`),
//! INTEG-020/040/060 (`custom_*`) und INTEG-001 (`api_values`); die Tests hängen deshalb an keiner Funktions-ID außer
//! den genannten, wo sie ein Akzeptanzkriterium wirklich prüfen.

mod store_common;

use os_model::*;
use os_store::{
    decode_minhash, encode_minhash, queries, CrawlFile, CrawlSink, OpenMode, StorageProfile,
};
use store_common::*;

fn setup(urls: i64) -> (tempfile::TempDir, std::path::PathBuf, os_store::SqliteSink) {
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "m2.ospider", StorageProfile::Balanced);
    let mut sink = sink(&path, StorageProfile::Balanced);
    sink.write(batch_with_urls((1..=urls).map(|i| (i, true))))
        .unwrap();
    (dir, path, sink)
}

fn reader(path: &std::path::Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

fn text(file: &CrawlFile, sql: &str) -> String {
    file.conn()
        .query_row(sql, [], |r| r.get::<_, Option<String>>(0))
        .unwrap()
        .unwrap_or_default()
}

fn stamp(file: &CrawlFile, id: i64) -> i64 {
    count(
        file,
        &format!("SELECT change_seq FROM urls WHERE id = {id}"),
    )
}

fn ext(url: i64, key: &str, value: Value) -> ExtWrite {
    ExtWrite {
        url: UrlId(url),
        key: ExtKey::parse(key).unwrap(),
        value,
    }
}

#[test]
fn ext_values_roundtrip_in_their_columns_and_upsert_replaces() {
    let (_dir, path, mut sink) = setup(3);
    let b = WriteBatch {
        ext: vec![
            ext(1, "security.csp_present", Value::Bool(true)),
            ext(1, "readability.flesch", Value::Float(61.5)),
            ext(1, "head.title_outside", Value::Int(2)),
            ext(1, "content.lang", Value::Text("de-DE".into())),
            ext(1, "content.nothing", Value::Null),
            ext(2, "security.csp_present", Value::Bool(false)),
        ],
        ..WriteBatch::default()
    };
    sink.write(b).unwrap();
    let file = reader(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM url_ext"), 6);
    assert_eq!(
        count(
            &file,
            "SELECT v_int FROM url_ext WHERE url_id = 1 AND key = 'security.csp_present'"
        ),
        1
    );
    assert_eq!(
        count(
            &file,
            "SELECT v_int FROM url_ext WHERE url_id = 2 AND key = 'security.csp_present'"
        ),
        0
    );
    let flesch: f64 = file
        .conn()
        .query_row(
            "SELECT v_real FROM url_ext WHERE url_id = 1 AND key = 'readability.flesch'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(flesch, 61.5);
    assert_eq!(
        text(
            &file,
            "SELECT v_text FROM url_ext WHERE key = 'content.lang'"
        ),
        "de-DE"
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM url_ext WHERE key = 'content.nothing' AND v_int IS NULL AND v_real IS NULL AND v_text IS NULL"
        ),
        1
    );

    // Upsert: derselbe Schlüssel wird ersetzt, andere bleiben.
    let b2 = WriteBatch {
        ext: vec![ext(1, "readability.flesch", Value::Float(70.0))],
        ..WriteBatch::default()
    };
    sink.write(b2).unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM url_ext"), 6);
    let flesch: f64 = file
        .conn()
        .query_row(
            "SELECT v_real FROM url_ext WHERE url_id = 1 AND key = 'readability.flesch'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(flesch, 70.0);
}

#[test]
fn ext_rows_stamp_url_ext_and_the_change_feed_lists_the_url() {
    let (_dir, path, mut sink) = setup(3);
    let file = reader(&path);
    let base = queries::change_seq(file.conn()).unwrap();
    let b = WriteBatch {
        ext: vec![ext(2, "linkscore.score", Value::Float(12.0))],
        ..WriteBatch::default()
    };
    sink.write(b).unwrap();
    let seq = queries::change_seq(file.conn()).unwrap();
    assert_eq!(seq, base + 1);
    assert_eq!(
        count(&file, "SELECT change_seq FROM url_ext WHERE url_id = 2"),
        seq as i64
    );
    // nur url_ext trägt den Stempel; die Zeile von `urls` bleibt, der Feed liefert die URL trotzdem.
    assert_eq!(stamp(&file, 2) as u64, base);
    let changed = queries::changed_since(file.conn(), base, 100).unwrap();
    assert_eq!(changed, vec![UrlId(2)]);
}

#[test]
fn invalid_ext_key_and_unknown_url_are_rejected_without_touching_the_file() {
    let (_dir, path, mut sink) = setup(2);
    let file = reader(&path);
    let before = queries::change_seq(file.conn()).unwrap();

    let bad_key = WriteBatch {
        ext: vec![ExtWrite {
            url: UrlId(1),
            key: ExtKey(std::borrow::Cow::Borrowed("kein_namensraum")),
            value: Value::Int(1),
        }],
        ..WriteBatch::default()
    };
    assert!(matches!(
        sink.write(bad_key),
        Err(os_store::StoreError::InvalidInput(_))
    ));
    let unknown_url = WriteBatch {
        ext: vec![
            ext(1, "head.a", Value::Int(1)),
            ext(99, "head.a", Value::Int(1)),
        ],
        ..WriteBatch::default()
    };
    assert!(matches!(
        sink.write(unknown_url),
        Err(os_store::StoreError::InvalidInput(_))
    ));
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM url_ext"),
        0,
        "Transaktion zurückgerollt"
    );
    assert_eq!(queries::change_seq(file.conn()).unwrap(), before);
}

fn sd_item(url: i64, format: &str, type_path: &str) -> SdItemWrite {
    SdItemWrite {
        url_id: UrlId(url),
        format: format.into(),
        type_path: type_path.into(),
        json: format!("{{\"@type\":\"{type_path}\"}}"),
    }
}

fn sd_issue(url: i64, item: Option<u32>, validation: &str, code: &str) -> SdIssueWrite {
    SdIssueWrite {
        url_id: UrlId(url),
        item,
        validation: validation.into(),
        feature: Some("Product snippets".into()),
        severity: "error".into(),
        property: Some("offers".into()),
        code: code.into(),
    }
}

#[test]
fn structured_data_roundtrip_links_issues_to_items_and_replaces_per_url() {
    let (_dir, path, mut sink) = setup(3);
    let b = WriteBatch {
        structured_data: vec![
            sd_item(1, "json-ld", "Product"),
            sd_item(1, "microdata", "Product/Offer"),
            sd_item(2, "json-ld", "Article"),
        ],
        sd_issues: vec![
            sd_issue(1, Some(1), "schema_org", "missing-price"),
            sd_issue(1, None, "parse", "bad-json"),
            sd_issue(2, Some(0), "rich_results", "missing-headline"),
        ],
        ..WriteBatch::default()
    };
    sink.write(b).unwrap();
    let file = reader(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM sd_items"), 3);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM sd_issues"), 3);
    // Befund 0 gehört zum zweiten Element der URL 1 (Microdata).
    assert_eq!(
        text(
            &file,
            "SELECT i.format FROM sd_issues d JOIN sd_items i ON i.id = d.item_id WHERE d.code = 'missing-price'"
        ),
        "microdata"
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM sd_issues WHERE code = 'bad-json' AND item_id IS NULL"
        ),
        1
    );
    assert!(stamp(&file, 1) > 0 && stamp(&file, 2) > 0);

    // Eine spätere Prüfung (anderer Batch) nur mit Befunden: Index zählt in den gespeicherten Elementen,
    // ersetzt die Befunde ihrer Art und lässt Elemente und andere Arten unangetastet.
    let later = WriteBatch {
        sd_issues: vec![sd_issue(1, Some(0), "schema_org", "wrong-type")],
        ..WriteBatch::default()
    };
    sink.write(later).unwrap();
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM sd_items WHERE url_id = 1"),
        2
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM sd_issues WHERE url_id = 1 AND validation = 'schema_org'"
        ),
        1
    );
    assert_eq!(
        text(
            &file,
            "SELECT i.format FROM sd_issues d JOIN sd_items i ON i.id = d.item_id WHERE d.code = 'wrong-type'"
        ),
        "json-ld"
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM sd_issues WHERE url_id = 1 AND validation = 'parse'"
        ),
        1
    );

    // Neue Elemente für URL 1 ersetzen Elemente und alle Befunde dieser URL; URL 2 bleibt.
    let replace = WriteBatch {
        structured_data: vec![sd_item(1, "rdfa", "Organization")],
        ..WriteBatch::default()
    };
    sink.write(replace).unwrap();
    assert_eq!(
        text(&file, "SELECT format FROM sd_items WHERE url_id = 1"),
        "rdfa"
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM sd_issues WHERE url_id = 1"),
        0
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM sd_items WHERE url_id = 2"),
        1
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM sd_issues WHERE url_id = 2"),
        1
    );

    // Index außerhalb der Elemente ist ein Fehler.
    let bad = WriteBatch {
        sd_issues: vec![sd_issue(2, Some(5), "schema_org", "x")],
        ..WriteBatch::default()
    };
    assert!(sink.write(bad).is_err());
}

#[test]
fn render_results_roundtrip_with_children_and_replace_per_url() {
    let (_dir, path, mut sink) = setup(2);
    let render = |status: &str, console: usize| RenderWrite {
        url_id: UrlId(1),
        status: status.into(),
        html: Some(b"<html></html>".to_vec()),
        word_count: Some(42),
        timings: RenderTimings {
            load_ms: 100,
            wait_ms: 200,
            total_ms: 300,
        },
        console: (0..console)
            .map(|i| ConsoleEntry {
                level: "error".into(),
                text: format!("Fehler {i}"),
                source: Some("https://site.test/app.js".into()),
                line: Some(i as u32 + 1),
                ..Default::default()
            })
            .collect(),
        blocked: vec![BlockedResource {
            url: "https://cdn.test/x.js".into(),
            reason: "robots".into(),
            status: None,
        }],
        network: vec![NetworkEntry {
            url: "https://site.test/api".into(),
            status: 200,
            resource_type: "xhr".into(),
            bytes: 1234,
            duration_ms: Some(15),
        }],
        screenshots: vec![
            ScreenshotTile {
                tile: 0,
                data: vec![1, 2, 3],
                width: 10,
                height: 20,
            },
            ScreenshotTile {
                tile: 1,
                data: vec![4, 5],
                width: 10,
                height: 5,
            },
        ],
        console_dropped: 0,
    };
    sink.write(WriteBatch {
        render: vec![render("ok", 3)],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    assert_eq!(
        text(&file, "SELECT status FROM render_pages WHERE url_id = 1"),
        "ok"
    );
    assert_eq!(count(&file, "SELECT word_count FROM render_pages"), 42);
    assert_eq!(count(&file, "SELECT total_ms FROM render_pages"), 300);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM render_console"), 3);
    assert_eq!(
        text(&file, "SELECT text FROM render_console WHERE position = 2"),
        "Fehler 2"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM render_blocked"), 1);
    assert_eq!(count(&file, "SELECT bytes FROM render_network"), 1234);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM screenshots"), 2);
    let png: Vec<u8> = file
        .conn()
        .query_row("SELECT png FROM screenshots WHERE tile = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(png, vec![4, 5]);
    assert!(stamp(&file, 1) > 0);

    // Ein neuer Lauf ersetzt alles der URL.
    sink.write(WriteBatch {
        render: vec![render("timeout", 1)],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(text(&file, "SELECT status FROM render_pages"), "timeout");
    assert_eq!(count(&file, "SELECT COUNT(*) FROM render_console"), 1);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM screenshots"), 2);
}

#[test]
fn accessibility_spelling_forms_and_custom_values_roundtrip() {
    let (_dir, path, mut sink) = setup(3);
    let b = WriteBatch {
        a11y: vec![A11yWrite {
            url_id: UrlId(1),
            rules: vec![
                A11yRuleResult {
                    rule_id: "image-alt".into(),
                    status: "violation".into(),
                    impact: Some("critical".into()),
                },
                A11yRuleResult {
                    rule_id: "label".into(),
                    status: "pass".into(),
                    impact: None,
                },
            ],
            nodes: vec![A11yNode {
                rule_id: "image-alt".into(),
                target: "img.logo".into(),
                snippet: "<img src=\"logo.png\">".into(),
                summary: "Alt fehlt".into(),
            }],
        }],
        spelling: vec![
            SpellingWrite {
                url_id: UrlId(1),
                kind: "spelling".into(),
                language: "de-DE".into(),
                section: "body".into(),
                text: "Rechtschreibfeler".into(),
                detail: None,
                suggestion: Some("Rechtschreibfehler".into()),
                rule_id: None,
            },
            SpellingWrite {
                url_id: UrlId(1),
                kind: "grammar".into(),
                language: "de-DE".into(),
                section: "title".into(),
                text: "Ein Fehler".into(),
                detail: Some("Kongruenz".into()),
                suggestion: None,
                rule_id: Some("DE_AGREEMENT".into()),
            },
        ],
        forms: vec![
            FormWrite {
                url_id: UrlId(2),
                idx: 0,
                action: Some("http://site.test/login".into()),
                method: "post".into(),
            },
            FormWrite {
                url_id: UrlId(2),
                idx: 1,
                action: None,
                method: "get".into(),
            },
        ],
        custom_search: vec![CustomSearchWrite {
            url_id: UrlId(1),
            def_index: 0,
            count: 4,
            error: None,
        }],
        custom_extraction: vec![
            CustomExtractionWrite {
                url_id: UrlId(1),
                def_index: 0,
                number: 0,
                value: "A".into(),
                error: None,
            },
            CustomExtractionWrite {
                url_id: UrlId(1),
                def_index: 0,
                number: 1,
                value: "B".into(),
                error: None,
            },
        ],
        custom_js: vec![CustomJsWrite {
            url_id: UrlId(3),
            def_index: 1,
            number: 0,
            value: None,
            error: Some("Zeitlimit".into()),
        }],
        ..WriteBatch::default()
    };
    sink.write(b).unwrap();
    let file = reader(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM a11y_rules"), 2);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM a11y_nodes"), 1);
    assert_eq!(
        text(
            &file,
            "SELECT impact FROM a11y_rules WHERE rule_id = 'image-alt'"
        ),
        "critical"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM spelling_findings"), 2);
    assert_eq!(
        text(
            &file,
            "SELECT rule_id FROM spelling_findings WHERE kind = 'grammar'"
        ),
        "DE_AGREEMENT"
    );
    assert_eq!(count(&file, "SELECT COUNT(*) FROM forms"), 2);
    assert_eq!(text(&file, "SELECT method FROM forms WHERE idx = 1"), "get");
    assert_eq!(count(&file, "SELECT count FROM custom_search_hits"), 4);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM custom_extraction_values"),
        2
    );
    assert_eq!(
        text(&file, "SELECT error FROM custom_js_values WHERE url_id = 3"),
        "Zeitlimit"
    );
    for id in 1..=3 {
        assert!(stamp(&file, id) > 0, "URL {id} gestempelt");
    }

    // spelling ersetzt je (URL, Art): nur Rechtschreibung neu → Grammatik bleibt.
    sink.write(WriteBatch {
        spelling: vec![SpellingWrite {
            url_id: UrlId(1),
            kind: "spelling".into(),
            language: "en-GB".into(),
            section: "h1".into(),
            text: "teh".into(),
            detail: None,
            suggestion: Some("the".into()),
            rule_id: None,
        }],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM spelling_findings"), 2);
    assert_eq!(
        text(
            &file,
            "SELECT text FROM spelling_findings WHERE kind = 'spelling'"
        ),
        "teh"
    );
    assert_eq!(
        count(
            &file,
            "SELECT COUNT(*) FROM spelling_findings WHERE kind = 'grammar'"
        ),
        1
    );
}

#[test]
fn minhash_signatures_roundtrip_as_little_endian_blobs() {
    let sig: Vec<u32> = (0..128u32).map(|i| i.wrapping_mul(2_654_435_761)).collect();
    assert_eq!(decode_minhash(&encode_minhash(&sig)).unwrap(), sig);
    assert_eq!(decode_minhash(&[1, 2, 3]), None);
    let (_dir, path, mut sink) = setup(2);
    sink.write(WriteBatch {
        minhash: vec![(UrlId(1), sig.clone()), (UrlId(2), vec![7, 8, 9])],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    let stored: Vec<u8> = file
        .conn()
        .query_row("SELECT sig FROM minhash WHERE url_id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored.len(), 128 * 4);
    assert_eq!(decode_minhash(&stored).unwrap(), sig);
    // Upsert
    sink.write(WriteBatch {
        minhash: vec![(UrlId(2), vec![1])],
        ..WriteBatch::default()
    })
    .unwrap();
    let stored: Vec<u8> = file
        .conn()
        .query_row("SELECT sig FROM minhash WHERE url_id = 2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(decode_minhash(&stored).unwrap(), vec![1]);
}

#[test]
fn api_values_roundtrip_upsert_and_unknown_urls_land_in_api_urls() {
    let (_dir, path, mut sink) = setup(2);
    let at = Timestamp::from_db("2026-10-17T10:00:00.000Z").unwrap();
    let value = |url_id: Option<i64>, metric: &str, v: f64| ApiValueWrite {
        url_id: url_id.map(UrlId),
        url: "https://site.test/p1".into(),
        source: "gsc".into(),
        metric: metric.into(),
        level: "url".into(),
        v_real: Some(v),
        v_text: None,
        fetched_at: at,
    };
    sink.write(WriteBatch {
        api_values: vec![
            value(Some(1), "clicks", 10.0),
            value(Some(1), "impressions", 100.0),
            value(None, "clicks", 3.0),
            value(None, "impressions", 30.0),
        ],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM api_values"), 2);
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM api_urls WHERE source = 'gsc'"),
        1,
        "dieselbe unbekannte URL steht einmal in api_urls"
    );
    sink.write(WriteBatch {
        api_values: vec![value(Some(1), "clicks", 11.0)],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM api_values"), 2);
    let clicks: f64 = file
        .conn()
        .query_row(
            "SELECT v_real FROM api_values WHERE metric = 'clicks'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(clicks, 11.0);
    assert_eq!(
        text(&file, "SELECT fetched_at FROM api_values LIMIT 1"),
        "2026-10-17T10:00:00.000Z"
    );
}

#[test]
fn page_html_is_stored_with_the_page_and_replaced_or_cleared_on_recrawl() {
    let (_dir, path, mut sink) = setup(2);
    let mut p = page(1, vec![]);
    p.html = Some(PageHtml {
        encoding: Some("utf-8".into()),
        body: b"<html>Quelltext</html>".to_vec(),
    });
    sink.write(WriteBatch {
        pages: vec![p],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    let stored = queries::stored_html(&file, UrlId(1)).unwrap().unwrap();
    assert_eq!(stored.charset.as_deref(), Some("utf-8"));
    assert_eq!(stored.bytes, b"<html>Quelltext</html>");

    // Neuer Abruf (replace) ohne Quelltext: der alte Quelltext und die M2-Kindzeilen der URL verschwinden,
    // Abrufgebundene Zusatzfakten (url_ext) verschwinden ebenfalls.
    sink.write(WriteBatch {
        ext: vec![ext(1, "head.a", Value::Int(1))],
        forms: vec![FormWrite {
            url_id: UrlId(1),
            idx: 0,
            action: None,
            method: "get".into(),
        }],
        ..WriteBatch::default()
    })
    .unwrap();
    let mut again = page(1, vec![]);
    again.replace = true;
    sink.write(WriteBatch {
        pages: vec![again],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(count(&file, "SELECT COUNT(*) FROM page_html"), 0);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM forms"), 0);
    assert_eq!(count(&file, "SELECT COUNT(*) FROM url_ext"), 0);
}

#[test]
fn sitemap_error_and_entry_extras_roundtrip() {
    let (_dir, path, mut sink) = setup(3);
    let sm = SitemapWrite {
        id: SitemapId(1),
        url: "https://site.test/sitemap.xml".into(),
        url_id: None,
        parent: None,
        kind: SitemapKind::Urlset,
        entries: 2,
        invalid_entries: 0,
        size_bytes: 100,
        status_code: Some(200),
        depth: 0,
        found_via: "robots".into(),
        entry_urls: vec![
            (UrlId(1), "/p1".into(), Some("2026-01-01".into())),
            (UrlId(2), "/p2".into(), None),
        ],
        hreflang: vec![],
        error: Some("XML-Fehler in Zeile 12".into()),
        entry_meta: vec![SitemapEntryMeta {
            url: UrlId(1),
            changefreq: Some("weekly".into()),
            priority: Some(0.8),
            images: 2,
            videos: 0,
            news: 1,
            extensions: Vec::new(),
        }],
    };
    sink.write(WriteBatch {
        sitemaps: vec![sm.clone()],
        ..WriteBatch::default()
    })
    .unwrap();
    let file = reader(&path);
    assert_eq!(
        text(&file, "SELECT error FROM sitemaps WHERE id = 1"),
        "XML-Fehler in Zeile 12"
    );
    assert_eq!(
        text(
            &file,
            "SELECT changefreq FROM sitemap_entries WHERE url_id = 1"
        ),
        "weekly"
    );
    let priority: f64 = file
        .conn()
        .query_row(
            "SELECT priority FROM sitemap_entries WHERE url_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(priority, 0.8);
    assert_eq!(
        count(&file, "SELECT images FROM sitemap_entries WHERE url_id = 1"),
        2
    );
    assert_eq!(
        count(&file, "SELECT news FROM sitemap_entries WHERE url_id = 1"),
        1
    );
    assert_eq!(
        count(&file, "SELECT images FROM sitemap_entries WHERE url_id = 2"),
        0
    );
    assert_eq!(
        count(&file, "SELECT COUNT(*) FROM sitemap_entries WHERE url_id = 2 AND changefreq IS NULL AND priority IS NULL"),
        1
    );

    // entry_meta ohne Eintrag in entry_urls ist ein Fehler.
    let mut bad = sm;
    bad.entry_meta[0].url = UrlId(3);
    assert!(sink
        .write(WriteBatch {
            sitemaps: vec![bad],
            ..WriteBatch::default()
        })
        .is_err());
}

#[test]
fn m2_parts_in_the_same_batch_as_the_page_share_one_transaction_and_sequence() {
    let (_dir, path, mut sink) = setup(2);
    let file = reader(&path);
    let before = queries::change_seq(file.conn()).unwrap();
    let mut b = WriteBatch {
        pages: vec![page(1, vec![])],
        ..WriteBatch::default()
    };
    b.ext.push(ext(1, "head.a", Value::Int(1)));
    b.forms.push(FormWrite {
        url_id: UrlId(1),
        idx: 0,
        action: None,
        method: "get".into(),
    });
    sink.write(b).unwrap();
    let seq = queries::change_seq(file.conn()).unwrap();
    assert_eq!(seq, before + 1, "genau eine Sequenz für den ganzen Batch");
    assert_eq!(stamp(&file, 1) as u64, seq);
    assert_eq!(
        count(&file, "SELECT change_seq FROM url_ext WHERE url_id = 1") as u64,
        seq
    );

    // Ein Batch, der nur aus einem M2-Teil besteht, ist kein No-op.
    sink.write(WriteBatch {
        forms: vec![FormWrite {
            url_id: UrlId(2),
            idx: 0,
            action: None,
            method: "post".into(),
        }],
        ..WriteBatch::default()
    })
    .unwrap();
    assert_eq!(queries::change_seq(file.conn()).unwrap(), seq + 1);

    // Ein Fehler in einem M2-Teil rollt den ganzen Batch zurück, auch die Seite.
    let failing = WriteBatch {
        pages: vec![{
            let mut p = page(2, vec![]);
            p.fetch.status_code = 500;
            p
        }],
        forms: vec![FormWrite {
            url_id: UrlId(77),
            idx: 0,
            action: None,
            method: "get".into(),
        }],
        ..WriteBatch::default()
    };
    assert!(sink.write(failing).is_err());
    assert_eq!(queries::change_seq(file.conn()).unwrap(), seq + 1);
    let status: Option<i64> = file
        .conn()
        .query_row("SELECT status_code FROM urls WHERE id = 2", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        status, None,
        "die Seite aus dem fehlgeschlagenen Batch ist nicht geschrieben"
    );
}
