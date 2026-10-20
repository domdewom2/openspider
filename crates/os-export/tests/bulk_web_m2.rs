#[path = "bulk_web_support.rs"]
mod support;
use os_export::{export_bulk, Format};
use os_model::{CancelToken, RenderingMode};
use support::*;

// spec: OUT-048
// spec: CRAWL-062
#[test]
fn headers_preserve_case_order_repetitions_and_redact_only_request_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    let request = bulk(&file, &cfg, "Web:All HTTP Request Headers", dir.path());
    golden("request-headers.csv", &request);
    assert!(!request.contains("secret"));
    assert_eq!(request.matches("<redacted>").count(), 3);
    cfg.export.show_credentials = true;
    let shown = bulk(&file, &cfg, "Web:All HTTP Request Headers", dir.path());
    assert!(shown.contains("Basic secret"));
    assert!(shown.contains("Proxy secret"));
    assert!(shown.contains("session=secret"));
    let response = bulk(&file, &cfg, "Web:All HTTP Response Headers", dir.path());
    golden("response-headers.csv", &response);
    assert_eq!(
        records(&response)
            .iter()
            .filter(|r| r[1] == "Set-Cookie")
            .count(),
        2
    );
    assert_eq!(
        records(&response).iter().filter(|r| r[1] == "Link").count(),
        2
    );
    cfg.extraction.http_headers = false;
    assert!(records(&bulk(
        &file,
        &cfg,
        "Web:All HTTP Response Headers",
        dir.path()
    ))
    .is_empty());
}

// spec: OUT-049
// spec: CRAWL-062
#[test]
fn cookies_follow_ui_columns_defaults_party_classification_and_rendering_switch() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.rendering.mode = RenderingMode::JavaScript;
    let text = bulk(&file, &cfg, "Web:All Cookies", dir.path());
    golden("cookies.csv", &text);
    assert_eq!(
        text.lines().next().unwrap(),
        "Address,Cookie Name,Cookie Value,Domain,Path,Expires,Max-Age,Secure,HttpOnly,SameSite,Type,Session,First-Party"
    );
    let rows = records(&text);
    let session = rows.iter().find(|r| r[1] == "session").unwrap();
    assert_eq!(
        &session[3..],
        &[
            "www.site.test",
            "/",
            "",
            "",
            "True",
            "True",
            "Strict",
            "HTTP",
            "True",
            "True"
        ]
    );
    assert!(rows
        .iter()
        .any(|r| r[1] == "third" && r[10] == "JavaScript" && r[12] == "False"));
    assert!(rows.iter().any(|r| r[1] == "sibling" && r[12] == "True"));
    assert!(rows.iter().any(|r| r[1] == "trap" && r[12] == "False"));
    assert!(rows
        .iter()
        .any(|r| r[1] == "expires" && r[5] == "2030-01-02T00:00:00Z" && r[11] == "False"));
    cfg.rendering.mode = RenderingMode::Text;
    assert!(!bulk(&file, &cfg, "Web:All Cookies", dir.path()).contains("JavaScript"));
    cfg.extraction.cookies = false;
    assert!(records(&bulk(&file, &cfg, "Web:All Cookies", dir.path())).is_empty());
}

// spec: OUT-045
// spec: OUT-046
// spec: CRAWL-063
// spec: ONPAGE-165
#[tokio::test]
async fn real_crawl_exports_exact_source_bytes_and_configured_utf8_text() {
    let mut cfg = config();
    cfg.content.area.include_elements = vec!["main".into()];
    cfg.content.area.exclude_classes = vec!["drop".into()];
    let fx = os_engine::testing::crawl_dir_with(
        &fixture_dir(),
        "/index.html",
        cfg.clone(),
        os_engine::testing::test_deps(),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let source = export_bulk(
        &fx.file,
        &cfg,
        "Web:All Page Source",
        dir.path(),
        Format::Xlsx,
    )
    .unwrap();
    assert_eq!(
        source.last().unwrap().path,
        dir.path().join("page_source/index.csv")
    );
    assert_eq!(source.last().unwrap().rows, 3);
    let source_rows = records(&std::fs::read_to_string(&source.last().unwrap().path).unwrap());
    let index = source_rows
        .iter()
        .find(|r| r[0].ends_with("/index.html"))
        .unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("page_source").join(&index[1])).unwrap(),
        std::fs::read(fixture_dir().join("index.html")).unwrap()
    );
    let compression: String = fx
        .file
        .conn()
        .query_row("SELECT compression FROM page_html LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(compression, "zstd");
    let text = export_bulk(&fx.file, &cfg, "Web:All Page Text", dir.path(), Format::Csv).unwrap();
    assert_eq!(text.last().unwrap().rows, 1);
    let rows = records(&std::fs::read_to_string(&text.last().unwrap().path).unwrap());
    let content = std::fs::read_to_string(dir.path().join("page_text").join(&rows[0][1])).unwrap();
    golden("page.txt", &content);
    assert_eq!(content, "Café & tea\nVisible content.");
    let table = bulk(&fx.file, &cfg, "Content:All Page Text", dir.path());
    let normalized = os_testsite::golden::normalize(&table, &[&fx.site.base_url()]);
    golden("content-text.csv", &normalized);
    assert_eq!(records(&table)[0][1], content);
}

// spec: OUT-045
// spec: CRAWL-063
#[test]
fn rendered_selection_requires_rendering_and_saved_dom_and_never_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    file.conn()
        .execute(
            "INSERT INTO render_pages(url_id,status,html) VALUES(1,'ok',?1)",
            [b"<html>rendered</html>".as_slice()],
        )
        .unwrap();
    assert!(records(&bulk(
        &file,
        &cfg,
        "Web:All Rendered Page Source",
        dir.path()
    ))
    .is_empty());
    cfg.extraction.store_rendered_html = true;
    assert!(records(&bulk(
        &file,
        &cfg,
        "Web:All Rendered Page Source",
        dir.path()
    ))
    .is_empty());
    cfg.rendering.mode = RenderingMode::JavaScript;
    let rows = records(&bulk(
        &file,
        &cfg,
        "Web:All Page Source:Rendered",
        dir.path(),
    ));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][2], "Rendered");
    assert_eq!(
        std::fs::read(dir.path().join("page_source/rendered").join(&rows[0][1])).unwrap(),
        b"<html>rendered</html>"
    );
    assert!(records(&bulk(&file, &cfg, "Web:All Page Source", dir.path())).is_empty());
    let entry = os_export::catalog::bulk_web::entries()
        .iter()
        .find(|e| e.name == "Web:All Rendered Page Source")
        .unwrap();
    file.conn().execute("DELETE FROM render_pages", []).unwrap();
    assert!(
        !os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg).enabled
    );
    cfg.extraction.store_rendered_html = false;
    assert!(
        !os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg).enabled
    );
}

// spec: OUT-045
// spec: OUT-046
#[test]
fn batch_writes_directories_outside_workbook_and_observes_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    file.conn()
        .execute(
            "INSERT INTO page_html(url_id,encoding,body) VALUES(1,'UTF-8',?1)",
            [b"<body>Batch text</body>".as_slice()],
        )
        .unwrap();
    let cfg = config();
    let cancel = CancelToken::new();
    let mut ctx = os_export::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    let result = os_export::pipeline::export_batch(
        &file,
        &cfg,
        &[
            os_export::types::ExportSelection {
                kind: "bulk".into(),
                name: "Web:All Page Source".into(),
            },
            os_export::types::ExportSelection {
                kind: "bulk".into(),
                name: "Web:All Page Text".into(),
            },
        ],
        &os_export::pipeline::BatchOptions {
            format: Format::Xlsx,
            directory: dir.path(),
            crawl_name: "fixture",
            single_workbook: true,
        },
        &mut ctx,
    )
    .unwrap();
    assert_eq!(result.files.len(), 4);
    assert!(dir.path().join("page_source/index.csv").exists());
    assert!(dir.path().join("page_text/index.csv").exists());
    cancel.cancel();
    assert!(os_export::catalog::bulk_web::export_pages(
        &file,
        &cfg,
        os_export::catalog::bulk_web::PageExport::Original,
        dir.path(),
        Some(&ctx)
    )
    .is_err());
}

// spec: OUT-045
// spec: OUT-046
// spec: CRAWL-063
#[test]
fn original_non_utf8_bytes_are_preserved_while_text_is_utf8_and_respects_population() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let cfg = config();
    let raw = b"<body><nav>menu</nav><p>Caf\xe9 &amp; tea</p><script>secret</script></body>";
    file.conn()
        .execute(
            "INSERT INTO page_html(url_id,encoding,body) VALUES(1,'windows-1252',?1)",
            [raw.as_slice()],
        )
        .unwrap();
    // Stored bodies on excluded populations must not leak into the export.
    for id in [2, 3, 4] {
        file.conn()
            .execute(
                "INSERT INTO page_html(url_id,encoding,body) VALUES(?1,'UTF-8',?2)",
                (id, b"<body>excluded</body>".as_slice()),
            )
            .unwrap();
    }
    file.conn()
        .execute("UPDATE urls SET is_internal=0 WHERE id=2", [])
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET content_class='PDF' WHERE id=3", [])
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET status_code=201 WHERE id=4", [])
        .unwrap();
    let source = export_bulk(&file, &cfg, "Web:All Page Source", dir.path(), Format::Csv).unwrap();
    assert_eq!(source.last().unwrap().rows, 2);
    let text = export_bulk(&file, &cfg, "Web:All Page Text", dir.path(), Format::Csv).unwrap();
    assert_eq!(text.last().unwrap().rows, 1);
    let source_rows = records(&std::fs::read_to_string(&source.last().unwrap().path).unwrap());
    let first = source_rows.iter().find(|r| r[0].ends_with("/01")).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("page_source").join(&first[1])).unwrap(),
        raw
    );
    let rows = records(&std::fs::read_to_string(&text.last().unwrap().path).unwrap());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("page_text").join(&rows[0][1])).unwrap(),
        "Café & tea"
    );
}

// spec: OUT-048
#[test]
fn showing_credentials_is_explicit_portable_and_disabled_in_older_profiles() {
    let cfg = os_model::CrawlConfig::load_str("schema_version = 2\n[export]\nutf8_bom = false\n")
        .unwrap();
    let cfg = cfg.config;
    assert!(!cfg.export.show_credentials);
    let mut cfg = cfg;
    cfg.export.show_credentials = true;
    let saved = cfg.to_osconfig_string().unwrap();
    assert!(saved.contains("show_credentials = true"));
    assert!(
        os_model::CrawlConfig::load_str(&saved)
            .unwrap()
            .config
            .export
            .show_credentials
    );
}

// spec: OUT-045
// spec: CRAWL-063
#[test]
fn rendered_export_decompresses_the_writer_format_without_changing_dom_bytes() {
    use os_store::CrawlSink;
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.extraction.store_rendered_html = true;
    cfg.rendering.mode = RenderingMode::JavaScript;
    let body = b"<!doctype html>\n<html><body>Caf\xc3\xa9</body></html>\n";
    let mut sink = os_store::SqliteSink::open(
        &dir.path().join("fixture.ospider"),
        os_store::StorageProfile::Balanced,
    )
    .unwrap();
    sink.write(os_model::WriteBatch {
        render: vec![os_model::RenderWrite {
            url_id: os_model::UrlId(1),
            status: "ok".into(),
            html: Some(body.to_vec()),
            word_count: None,
            timings: Default::default(),
            console: Vec::new(),
            console_dropped: 0,
            blocked: Vec::new(),
            network: Vec::new(),
            screenshots: Vec::new(),
        }],
        ..Default::default()
    })
    .unwrap();
    let compression: String = file
        .conn()
        .query_row("SELECT compression FROM render_pages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(compression, "zstd");
    let index = records(&bulk(
        &file,
        &cfg,
        "Web:All Rendered Page Source",
        dir.path(),
    ));
    assert_eq!(index.len(), 1);
    assert_eq!(
        std::fs::read(dir.path().join("page_source/rendered").join(&index[0][1])).unwrap(),
        body
    );
}

// spec: OUT-045
// spec: OUT-046
#[test]
fn skipping_empty_page_exports_does_not_publish_an_index() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let cfg = config();
    let cancel = CancelToken::new();
    let mut ctx = os_export::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: true,
        workbook: None,
        skipped: Vec::new(),
    };
    let result = os_export::pipeline::export_batch(
        &file,
        &cfg,
        &[os_export::types::ExportSelection {
            kind: "bulk".into(),
            name: "Web:All Page Text".into(),
        }],
        &os_export::pipeline::BatchOptions {
            format: Format::Csv,
            directory: dir.path(),
            crawl_name: "empty",
            single_workbook: false,
        },
        &mut ctx,
    )
    .unwrap();
    assert!(result.files.is_empty());
    assert_eq!(result.skipped, vec!["Web:All Page Text"]);
    assert!(!dir.path().join("page_text/index.csv").exists());
}

// spec: OUT-045
// spec: OUT-022
#[test]
fn batch_keeps_original_and_rendered_sources_and_indexes_in_either_order() {
    for reversed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let file = file(dir.path());
        file.conn()
            .execute(
                "INSERT INTO page_html(url_id,encoding,body) VALUES(1,'UTF-8',?1)",
                [b"original".as_slice()],
            )
            .unwrap();
        file.conn()
            .execute(
                "INSERT INTO render_pages(url_id,status,html) VALUES(1,'ok',?1)",
                [b"rendered".as_slice()],
            )
            .unwrap();
        let mut cfg = config();
        cfg.extraction.store_rendered_html = true;
        cfg.rendering.mode = RenderingMode::JavaScript;
        let mut names = ["Web:All Page Source", "Web:All Rendered Page Source"];
        if reversed {
            names.reverse();
        }
        let selections = names.map(|name| os_export::types::ExportSelection {
            kind: "bulk".into(),
            name: name.into(),
        });
        let cancel = CancelToken::new();
        let progress = std::cell::RefCell::new(Vec::new());
        let report_progress = |n| progress.borrow_mut().push(n);
        let mut ctx = os_export::types::ExportCtx {
            cancel: &cancel,
            progress: &report_progress,
            skip_empty: false,
            workbook: None,
            skipped: Vec::new(),
        };
        let result = os_export::pipeline::export_batch(
            &file,
            &cfg,
            &selections,
            &os_export::pipeline::BatchOptions {
                format: Format::Xlsx,
                directory: dir.path(),
                crawl_name: "both",
                single_workbook: true,
            },
            &mut ctx,
        )
        .unwrap();
        assert_eq!(result.files.len(), 4);
        let unique: std::collections::HashSet<_> = result.files.iter().map(|f| &f.path).collect();
        assert_eq!(unique.len(), result.files.len());
        for (subdir, kind, body) in [
            ("page_source", "Original", b"original".as_slice()),
            ("page_source/rendered", "Rendered", b"rendered".as_slice()),
        ] {
            let index_path = dir.path().join(subdir).join("index.csv");
            let index = records(&std::fs::read_to_string(&index_path).unwrap());
            assert_eq!(index.len(), 1);
            assert_eq!(index[0][2], kind);
            assert_eq!(
                std::fs::read(index_path.parent().unwrap().join(&index[0][1])).unwrap(),
                body
            );
            assert_eq!(
                result.files.iter().filter(|f| f.path == index_path).count(),
                1
            );
        }
        assert!(progress.borrow().windows(2).all(|ns| ns[0] <= ns[1]));
        assert_eq!(progress.borrow().last(), Some(&2));
    }
}

// spec: OUT-010
// spec: OUT-045
// spec: OUT-046
#[test]
fn page_index_is_published_only_after_success_and_cleaned_on_midstream_failure() {
    use os_export::catalog::bulk_web::{export_pages, PageExport};
    for kind in [PageExport::Original, PageExport::Rendered, PageExport::Text] {
        for existing in [false, true] {
            for cancel_during_export in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let file = file(dir.path());
                for id in [1, 2] {
                    let compression = if id == 2 && !cancel_during_export {
                        Some("zstd")
                    } else {
                        None
                    };
                    file.conn().execute("INSERT INTO page_html(url_id,encoding,body,compression) VALUES(?1,'UTF-8',?2,?3)", (id, b"<body>page</body>".as_slice(), compression)).unwrap();
                    file.conn().execute("INSERT INTO render_pages(url_id,status,html,compression) VALUES(?1,'ok',?2,?3)", (id, b"<body>rendered</body>".as_slice(), compression)).unwrap();
                }
                let subdir = match kind {
                    PageExport::Original => "page_source",
                    PageExport::Rendered => "page_source/rendered",
                    PageExport::Text => "page_text",
                };
                let out = dir.path().join(subdir);
                std::fs::create_dir_all(&out).unwrap();
                let index = out.join("index.csv");
                if existing {
                    std::fs::write(&index, b"previous complete index").unwrap();
                }
                let cancel = CancelToken::new();
                let visited = std::cell::Cell::new(0);
                let progress = |rows| {
                    visited.set(rows);
                    if cancel_during_export {
                        cancel.cancel();
                    }
                };
                let ctx = os_export::types::ExportCtx {
                    cancel: &cancel,
                    progress: &progress,
                    skip_empty: false,
                    workbook: None,
                    skipped: Vec::new(),
                };
                let mut cfg = config();
                cfg.rendering.mode = RenderingMode::JavaScript;
                cfg.extraction.store_rendered_html = true;
                let error = export_pages(&file, &cfg, kind, dir.path(), Some(&ctx)).unwrap_err();
                assert_eq!(
                    visited.get(),
                    1,
                    "failure must occur after writing the first index row"
                );
                if cancel_during_export {
                    assert!(error.to_string().contains("abgebrochen"));
                }
                if existing {
                    assert_eq!(std::fs::read(&index).unwrap(), b"previous complete index");
                } else {
                    assert!(!index.exists());
                }
                assert!(std::fs::read_dir(&out).unwrap().all(|e| {
                    e.unwrap()
                        .path()
                        .extension()
                        .is_none_or(|ext| ext != "part")
                }));
                if !cancel_during_export {
                    file.conn()
                        .execute("UPDATE page_html SET compression=NULL", [])
                        .unwrap();
                    file.conn()
                        .execute("UPDATE render_pages SET compression=NULL", [])
                        .unwrap();
                    let result = export_pages(&file, &cfg, kind, dir.path(), None).unwrap();
                    assert_eq!(result.last().unwrap().rows, 2);
                    assert_eq!(records(&std::fs::read_to_string(&index).unwrap()).len(), 2);
                }
            }
        }
    }
}

// spec: OUT-049
#[test]
fn cookie_dates_keep_valid_legacy_expiries_and_session_flags() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let cases = cookie_dates(&file);
    let rows = records(&bulk(&file, &config(), "Web:All Cookies", dir.path()));
    for (i, (_, expected)) in cases.iter().enumerate() {
        let row = rows.iter().find(|r| r[1] == format!("date-{i}")).unwrap();
        assert_eq!(row[5], expected.as_deref().unwrap_or(""));
        assert_eq!(row[11], if expected.is_some() { "False" } else { "True" });
    }
}
