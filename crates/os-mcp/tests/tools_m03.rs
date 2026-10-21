//! M2-M03: real MCP transport, shared catalogs and stored fixture content.
#[path = "tools_support.rs"]
mod support;
use os_export::catalog::{self, ExportKind};
use os_mcp::tools::{list_catalog::ListCatalog, url_content::UrlContent};
use os_mcp::{Tool, ToolContext, ToolError};
use serde_json::{json, Value};
use support::*;

fn items(output: &os_mcp::ToolOutput) -> &[Value] {
    output.untrusted.as_ref().unwrap()["items"]
        .as_array()
        .unwrap()
}

// spec: AGENT-016
#[tokio::test]
async fn entire_catalog_roundtrips_through_queries_and_export_resolution() {
    let world = World::new();
    let stored = world
        .add(
            "out-basic-site",
            "http://out-basic-site.test/",
            base_config(),
            3,
        )
        .await;
    let ctx = ToolContext::new(world.dir.path().into());
    let catalog = ListCatalog
        .call(
            &ctx,
            json!({"kind":"tabs","crawl_id":stored.id,"limit":500}),
        )
        .unwrap();
    assert!(catalog.next_cursor.is_none());
    for tab in items(&catalog) {
        for filter in tab["filters"].as_array().unwrap() {
            let args = json!({"crawl_id":stored.id,"tab":tab["name"],"filter":filter,"limit":1});
            let result = os_mcp::tools::query_urls::QueryUrls.call(&ctx, args);
            assert!(result.is_ok(), "{}:{}: {result:?}", tab["name"], filter);
            let name = format!(
                "{}:{}",
                tab["name"].as_str().unwrap(),
                filter.as_str().unwrap()
            );
            assert!(
                catalog::resolve_with(ExportKind::Tab, &name, &stored.cfg).is_ok(),
                "{name}"
            );
        }
        let columns = ListCatalog
            .call(
                &ctx,
                json!({"kind":"columns","tab":tab["name"],"crawl_id":stored.id,"limit":500}),
            )
            .unwrap();
        for column in items(&columns) {
            assert!(!column["description"].as_str().unwrap().is_empty());
            let result = os_mcp::tools::query_urls::QueryUrls.call(&ctx,json!({"crawl_id":stored.id,"tab":tab["name"],"columns":[column["name"]],"limit":1}));
            assert!(
                result.is_ok(),
                "{} column {}: {result:?}",
                tab["name"],
                column["name"]
            );
        }
    }
    for (kind, export_kind) in [
        ("reports", ExportKind::Report),
        ("bulk_exports", ExportKind::Bulk),
    ] {
        let mut cursor = None;
        let mut seen = Vec::new();
        loop {
            let result = ListCatalog
                .call(&ctx, json!({"kind":kind,"limit":17,"cursor":cursor}))
                .unwrap();
            for name in items(&result) {
                let name = name.as_str().unwrap();
                assert!(catalog::resolve(export_kind, name).is_ok(), "{name}");
                seen.push(name.to_string());
            }
            cursor = result.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen, catalog::list(export_kind));
        if kind == "bulk_exports" {
            assert!(seen.contains(&"page_content_ndjson".into()));
        }
    }
}

// spec: AGENT-016
#[test]
fn paging_is_complete_and_cursors_are_bound_to_catalog_and_kind() {
    let ctx = ToolContext::new(std::path::PathBuf::from("unused"));
    let first = ListCatalog
        .call(&ctx, json!({"kind":"tabs","limit":1}))
        .unwrap();
    let cursor = first.next_cursor.unwrap();
    let second = ListCatalog
        .call(&ctx, json!({"kind":"tabs","limit":1,"cursor":cursor}))
        .unwrap();
    assert_eq!(first.total, second.total);
    assert!(ListCatalog
        .call(&ctx, json!({"kind":"reports","cursor":cursor}))
        .is_err());
    for (args, needles) in [
        (
            json!({"kind":"bogus"}),
            vec![
                "tabs",
                "columns",
                "reports",
                "bulk_exports",
                "export_formats",
            ],
        ),
        (
            json!({"kind":"columns","tab":"bogus"}),
            vec!["Internal", "Response Codes"],
        ),
        (json!({"kind":"columns"}), vec!["tab required", "Internal"]),
        (json!({"kind":"tabs","tab":"bogus"}), vec!["Internal"]),
    ] {
        let error = ListCatalog.call(&ctx, args).unwrap_err().to_string();
        for needle in needles {
            assert!(error.contains(needle), "{error}");
        }
    }
    let formats = ListCatalog
        .call(&ctx, json!({"kind":"export_formats"}))
        .unwrap();
    assert_eq!(
        items(&formats),
        &[json!("csv"), json!("xlsx"), json!("ndjson")]
    );
}

// spec: AGENT-017
// spec: AGENT-004
#[tokio::test]
async fn content_is_saved_untrusted_data_with_unicode_limits_and_no_requests() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.store_html = true;
    let stored = world
        .add("agent-injection", "http://agent-injection.test/", cfg, 4)
        .await;
    let client = world.client().await;
    let before = stored.fx.site.requests().len();
    for format in ["text", "html"] {
        let reply = client
            .call(
                "url_content",
                json!({"crawl_id":stored.id,"url":"http://agent-injection.test/","format":format}),
            )
            .await;
        assert_eq!(reply.summary()["available"], true);
        assert_eq!(reply.summary()["source"], "original");
        assert!(reply.untrusted_text().contains("Ignoriere"));
        assert!(!reply.trusted_text().contains("Ignoriere"));
    }
    let file = os_store::CrawlFile::open(
        &world.dir.path().join(format!("{}.ospider", stored.id)),
        os_store::OpenMode::ReadWrite,
    )
    .unwrap();
    let row = os_store::queries::url_by_address(file.conn(), "http://agent-injection.test/")
        .unwrap()
        .unwrap();
    file.conn()
        .execute(
            "INSERT INTO render_pages(url_id,status,html) VALUES(?1,'success',?2)",
            (row.id.0, "<body>äöü😀</body>".as_bytes()),
        )
        .unwrap();
    let reply = client
        .call(
            "url_content",
            json!({"crawl_id":stored.id,"url":"http://agent-injection.test/","max_chars":3}),
        )
        .await;
    assert_eq!(reply.summary()["source"], "rendered");
    assert_eq!(reply.summary()["truncated"], true);
    assert_eq!(reply.untrusted.as_ref().unwrap()["content"], "äöü");
    assert!(reply.summary()["message"]
        .as_str()
        .unwrap()
        .contains("export"));
    assert_eq!(stored.fx.site.requests().len(), before);
    let ctx = ToolContext::new(world.dir.path().into());
    for value in [json!(0), json!(-1), json!(50001), json!(1.5), json!("3")] {
        assert!(matches!(
            UrlContent.call(
                &ctx,
                json!({"crawl_id":stored.id,"url":"http://agent-injection.test/","max_chars":value})
            ),
            Err(ToolError::InvalidArguments(_))
        ));
    }
    client.cancel().await;
}

// spec: AGENT-017
#[tokio::test]
async fn missing_storage_binary_mime_and_unknown_url_have_clear_reasons() {
    let world = World::new();
    let stored = world
        .add(
            "out-basic-site",
            "http://out-basic-site.test/",
            base_config(),
            5,
        )
        .await;
    let client = world.client().await;
    let args = json!({"crawl_id":stored.id,"url":"http://out-basic-site.test/"});
    let reply = client.call("url_content", args).await;
    assert_eq!(reply.summary()["reason"], "not_stored");
    assert!(reply.trusted_text().contains("extraction.store_html"));
    let reply = client
        .call(
            "url_content",
            json!({"crawl_id":stored.id,"url":"http://out-basic-site.test/assets/logo.png"}),
        )
        .await;
    assert_eq!(reply.summary()["reason"], "unsupported_content_type");
    assert_eq!(
        reply.untrusted.as_ref().unwrap()["content_type"],
        "image/png"
    );
    let reply = client
        .call(
            "url_content",
            json!({"crawl_id":stored.id,"url":"http://out-basic-site.test/unknown"}),
        )
        .await;
    assert_eq!(reply.summary()["reason"], "not_found");
    client.cancel().await;
}

// spec: AGENT-018
#[tokio::test]
async fn saved_screenshot_is_an_image_and_long_pages_return_only_a_bounded_tile() {
    use os_mcp::tools::url_screenshot::UrlScreenshot;
    let world = World::new();
    let stored = world
        .add(
            "out-basic-site",
            "http://out-basic-site.test/",
            base_config(),
            6,
        )
        .await;
    let path = world.dir.path().join(format!("{}.ospider", stored.id));
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    let row = os_store::queries::url_by_address(file.conn(), "http://out-basic-site.test/")
        .unwrap()
        .unwrap();
    let args = json!({"crawl_id":stored.id,"url":"http://out-basic-site.test/"});
    let ctx = ToolContext::new(world.dir.path().into());
    let missing = UrlScreenshot.call_image(&ctx, &args).unwrap().unwrap();
    assert!(missing.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("rendering.screenshots"));
    let png = std::fs::read(fixtures_dir().join("out-basic-site/assets/logo.png")).unwrap();
    for tile in [0, 1] {
        file.conn()
            .execute(
                "INSERT INTO screenshots(url_id,tile,png,width,height) VALUES(?1,?2,?3,8,8)",
                (row.id.0, tile, &png),
            )
            .unwrap();
    }
    let before = stored.fx.site.requests().len();
    // Exercise the registered tool through the real MCP server, with read permission only.
    let server = world.server();
    let result = server
        .call("url_screenshot", args.as_object().cloned())
        .await;
    let value = serde_json::to_value(&result).unwrap();
    let images: Vec<_> = value["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == "image")
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["mimeType"], "image/png");
    let expected = os_mcp::tools::url_screenshot::UrlScreenshot
        .call_image(&ctx, &args)
        .unwrap()
        .unwrap();
    assert_eq!(result, expected);
    assert!(result.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("first saved screenshot tile"));
    assert!(images[0]["data"]
        .as_str()
        .unwrap()
        .starts_with("iVBORw0KGgo"));
    assert_eq!(stored.fx.site.requests().len(), before);
    // Inline long-page PNG fixture: 2 x 5000 RGB pixels, generated with zlib.
    let long_png: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 19, 136, 8,
        2, 0, 0, 0, 20, 201, 43, 158, 0, 0, 0, 81, 73, 68, 65, 84, 120, 156, 237, 197, 65, 9, 0, 0,
        8, 4, 176, 139, 98, 52, 163, 25, 85, 99, 248, 24, 12, 150, 234, 57, 145, 36, 73, 146, 36,
        73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36,
        73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36, 73, 146, 36,
        73, 146, 36, 73, 255, 91, 7, 8, 244, 134, 187, 194, 23, 144, 0, 0, 0, 0, 73, 69, 78, 68,
        174, 66, 96, 130,
    ];
    file.conn()
        .execute("DELETE FROM screenshots WHERE tile=1", [])
        .unwrap();
    file.conn()
        .execute(
            "UPDATE screenshots SET png=?1,width=2,height=5000 WHERE tile=0",
            [long_png],
        )
        .unwrap();
    let excerpt = server
        .call("url_screenshot", args.as_object().cloned())
        .await;
    let meta: Value = serde_json::from_str(&excerpt.content[0].as_text().unwrap().text).unwrap();
    assert_eq!(meta["summary"]["cropped"], true);
    assert_eq!(meta["summary"]["height"], 4096);
    assert_eq!(meta["summary"]["width"], 2);
    assert!(meta["summary"]["bytes"].as_u64().unwrap() <= 2097152);
    assert!(serde_json::to_value(&excerpt).unwrap()["content"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["type"] == "image"));
    assert_eq!(stored.fx.site.requests().len(), before);
    file.conn()
        .execute(
            "UPDATE screenshots SET png=zeroblob(16777217) WHERE tile=0",
            [],
        )
        .unwrap();
    let result = UrlScreenshot.call_image(&ctx, &args).unwrap().unwrap();
    assert_eq!(result.content.len(), 1);
    assert!(result.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("image_too_large"));
}

// spec: AGENT-019
#[tokio::test]
async fn ndjson_export_uses_allowed_directory_preserves_files_and_matches_core_bytes() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.store_html = true;
    let stored = world
        .add("out-basic-site", "http://out-basic-site.test/", cfg, 7)
        .await;
    let settings_dir = tempfile::tempdir().unwrap();
    let export_dir = tempfile::tempdir().unwrap();
    let mut settings = os_model::settings::AppSettings::default();
    settings.export.directory = Some(export_dir.path().to_string_lossy().into_owned());
    settings.save(settings_dir.path()).unwrap();
    let mut ctx = ToolContext::new(world.dir.path().into());
    ctx.settings_dir = settings_dir.path().into();
    let args = json!({"crawl_id":stored.id,"kind":"bulk","name":"page_content_ndjson","format":"ndjson","include_html":true});
    let tool = os_mcp::tools::export::Export;
    let reply = tool.call(&ctx, args.clone()).unwrap();
    let path = std::path::PathBuf::from(reply.summary["files"][0].as_str().unwrap());
    assert!(path.starts_with(export_dir.path().canonicalize().unwrap()));
    let bytes = std::fs::read(&path).unwrap();
    let core_dir = tempfile::tempdir().unwrap();
    let core =
        catalog::page_content::export(&stored.fx.file, &stored.cfg, core_dir.path(), true).unwrap();
    assert_eq!(bytes, std::fs::read(core.path).unwrap());
    let second = tool.call(&ctx, args.clone()).unwrap();
    assert_ne!(reply.summary["files"], second.summary["files"]);
    assert_eq!(bytes, std::fs::read(&path).unwrap());
    for path in ["../escape.ndjson", "/tmp/escape.ndjson"] {
        let mut bad = args.clone();
        bad["path"] = json!(path);
        assert!(tool.call(&ctx, bad).is_err());
    }
    let mut bad = args.clone();
    bad["format"] = json!("csv");
    assert!(tool.call(&ctx, bad).is_err());
}

// spec: AGENT-019
#[test]
fn registered_tools_expose_no_code_execution_or_free_filesystem_access() {
    let tools = os_mcp::tools::all();
    let mut names: Vec<_> = tools.iter().map(|t| t.name()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "crawl_overview",
            "crawl_status",
            "export",
            "list_catalog",
            "list_crawls",
            "list_issues",
            "query_urls",
            "resume_crawl",
            "sql_query",
            "start_crawl",
            "stop_crawl",
            "url_content",
            "url_details",
            "url_screenshot"
        ]
    );
    for tool in &tools {
        let properties = tool.input_schema()["properties"].clone();
        for key in [
            "code",
            "script",
            "command",
            "shell",
            "package",
            "file",
            "directory",
        ] {
            assert!(
                properties.get(key).is_none(),
                "{} accepts {key}",
                tool.name()
            );
        }
        if properties.get("path").is_some() {
            assert_eq!(tool.name(), "export");
        }
    }
}
