//! Self-contained K09 fixture, extending the shared basic crawl only inside the temporary database.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::Fx;
use os_app::{dto::*, CallCtx};
use os_model::{CrawlConfig, RenderingMode};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
fn fixture() -> Fx {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    cfg.extraction.store_rendered_html = true;
    cfg.rendering.mode = RenderingMode::JavaScript;
    cfg.duplicates.near_enabled = true;
    cfg.extraction.structured_data.json_ld = true;
    cfg.extraction.structured_data.microdata = true;
    cfg.extraction.structured_data.rdfa = true;
    let fx = Fx::config(cfg);
    fx.write("INSERT INTO page_html(url_id,encoding,body) VALUES(1,'UTF-8',CAST('<html><head><title>Original</title></head><body><main>one two</main><script>hidden</script><style>hidden</style><noscript>hidden</noscript><template>hidden</template></body></html>' AS BLOB)); INSERT INTO render_pages(url_id,status,html) VALUES(1,'ok',CAST('<html><head><title>Rendered</title></head><body><main>one two three four five six</main></body></html>' AS BLOB)); UPDATE urls SET title_1='Original title',meta_desc_1='Original description',hash_md5='same' WHERE id=1; UPDATE urls SET hash_md5='same' WHERE id=2; INSERT INTO url_ext(url_id,key,v_text) VALUES(1,'content.text','one two'),(2,'content.text','one two three');");
    fx
}
async fn diff(fx: &Fx, mode: &str) -> Value {
    let handle: OpHandle = serde_json::from_value(
        fx.call("detail.sourceDiff", json!({"urlId":1,"mode":mode}))
            .await,
    )
    .unwrap();
    let status = tokio::time::timeout(Duration::from_secs(10), fx.app.ops().wait(&handle.op_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.state, OpState::Done, "{status:?}");
    status.result.unwrap()
}
// spec: UI-037
// spec: ONPAGE-194
#[tokio::test]
async fn serp_edits_survive_reopening_do_not_change_extraction_and_reset() {
    let fx = fixture();
    let original = fx
        .call("detail.serp", json!({"urlId":1,"device":"desktop"}))
        .await;
    assert_eq!(original["title"], "Original title");
    assert_eq!(original["host"], "site.test");
    fx.call(
        "detail.serpSaveEdit",
        json!({"urlId":1,"title":"Edited","description":""}),
    )
    .await;
    let cfg = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    fx.app.unregister_crawl(&cfg.key);
    fx.app.register_crawl(cfg.key.clone(), cfg.path.clone());
    let edited = fx
        .call("detail.serp", json!({"urlId":1,"device":"mobile"}))
        .await;
    assert_eq!(edited["title"], "Edited");
    assert_eq!(edited["description"], "");
    assert!(edited["edited"]["editedAt"]
        .as_str()
        .unwrap()
        .starts_with("1970-01-01"));
    let file = os_store::CrawlFile::open(&fx.path, os_store::OpenMode::ReadOnly).unwrap();
    assert_eq!(
        file.conn()
            .query_row("SELECT title_1 FROM urls WHERE id=1", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "Original title"
    );
    drop(file);
    fx.call("detail.serpReset", json!({"urlId":1})).await;
    let reset = fx
        .call("detail.serp", json!({"urlId":1,"device":"tablet"}))
        .await;
    assert_eq!(reset, original);
    fx.write("UPDATE urls SET meta_desc_1=NULL WHERE id=1");
    assert_eq!(
        fx.call("detail.serp", json!({"urlId":1,"device":"desktop"}))
            .await["description"],
        ""
    );
}
// spec: UI-037
// spec: ONPAGE-194
#[tokio::test]
async fn serp_measurement_uses_profiles_normalization_bold_prefix_and_word_cuts() {
    let fx = fixture();
    let measure = |device: &str, description: &str| json!({"title":"Start &amp;  Ende","description":description,"device":device,"boldTerms":[],"prefix":""});
    let desktop = fx
        .call("detail.serpMeasure", measure("desktop", &"a".repeat(110)))
        .await;
    let mobile = fx
        .call("detail.serpMeasure", measure("mobile", &"a".repeat(110)))
        .await;
    assert_eq!(desktop["rows"][0]["text"], "Start & Ende");
    assert_eq!(
        desktop["rows"][0]["px"].as_f64().unwrap(),
        f64::from(os_parse::pixel::width_px("Start & Ende", 20., false))
    );
    assert_eq!(desktop["rows"][1]["truncated"], false);
    assert_eq!(mobile["rows"][1]["truncated"], true);
    let bold=fx.call("detail.serpMeasure",json!({"title":"Start &amp;  Ende","description":"x","device":"desktop","boldTerms":["Start"],"prefix":"21.10.2026 — "})).await;
    assert_ne!(bold["rows"][0]["px"], desktop["rows"][0]["px"]);
    assert_eq!(bold["rows"][1]["text"], "21.10.2026 — x");
    let mut cfg = (*fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap()
        .config)
        .clone();
    cfg.serp.mobile.description_max_px = Some(900);
    cfg.meta_description.max_px = 1000;
    fx.app.sessions().update_config(
        &os_app::pool::CrawlKey("fx-ui-basic".into()),
        std::sync::Arc::new(cfg),
    );
    assert_eq!(
        fx.call("detail.serpMeasure", measure("mobile", &"a".repeat(110)))
            .await["rows"][1]["truncated"],
        false
    );
    assert_eq!(
        fx.call("detail.serpMeasure", measure("desktop", "x")).await["rows"][1]["maxPx"]
            .as_f64()
            .unwrap(),
        1000.0
    );
    let title = "word ".repeat(50);
    let m = fx
        .call(
            "detail.serpMeasure",
            json!({"title":title,"description":"","device":"desktop","boldTerms":[],"prefix":""}),
        )
        .await;
    let preview = m["rows"][0]["preview"].as_str().unwrap();
    assert!(preview.ends_with(" …"));
    assert!(os_parse::pixel::width_px(preview, 20., false) <= 561);
}
// spec: UI-041
#[tokio::test]
async fn source_lines_are_unicode_safe_paged_and_scoped_and_text_ignores_hidden_content() {
    let fx = fixture();
    fx.write("UPDATE page_html SET body=CAST('Ä😀\r\nsecond\n' AS BLOB) WHERE url_id=1");
    let doc = fx
        .call(
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":false}),
        )
        .await;
    assert_eq!(doc["lines"], 3);
    assert_eq!(
        fx.call(
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":0,"count":3})
        )
        .await,
        json!(["Ä😀", "second", ""])
    );
    assert_eq!(
        fx.call(
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":999,"count":3})
        )
        .await,
        json!([])
    );
    let mut other = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    fx.app
        .sessions()
        .set_current(os_app::WindowId("other".into()), other.clone());
    let err = fx
        .app
        .dispatch(
            CallCtx::for_window("other"),
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":0,"count":1}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    other.key = os_app::pool::CrawlKey("different".into());
    fx.app
        .sessions()
        .set_current(os_app::WindowId("main".into()), other);
    assert!(fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":0,"count":1})
        )
        .await
        .is_err());
    assert_eq!(os_app::source::visible_text("<body><p>one</p><script>no</script><style>no</style><noscript>no</noscript><template>no</template><p>two &amp; three</p>"),"one two & three");
}
// spec: UI-041
#[tokio::test]
async fn background_diff_counts_four_visible_words_and_reports_line_changes() {
    let fx = fixture();
    let result = diff(&fx, "words").await;
    assert_eq!(result["added"], 4);
    assert_eq!(result["removed"], 0);
    let result = diff(&fx, "lines").await;
    assert_eq!(result["added"], 1);
    assert_eq!(result["removed"], 1);
    let token = os_analyze::analysis::CancelToken::new();
    token.cancel();
    assert_eq!(
        os_app::source::diff("a", "b", DiffMode::Words, &token)
            .unwrap_err()
            .code,
        ErrorCode::Cancelled
    );
}
// spec: UI-041
// spec: UI-145
#[tokio::test]
async fn source_one_mb_and_diff_meet_service_budgets_and_large_documents_page() {
    let fx = fixture();
    let source = "<p>one two</p>\n".repeat(75_000);
    fx.app
        .write_queue()
        .submit(os_app::pool::CrawlKey("fx-ui-basic".into()), move |conn| {
            conn.execute(
                "UPDATE page_html SET body=?1 WHERE url_id=1",
                [source.as_bytes()],
            )?;
            conn.execute(
                "UPDATE render_pages SET html=?1 WHERE url_id=1",
                [source.as_bytes()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let start = Instant::now();
    let doc = fx
        .call(
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":false}),
        )
        .await;
    assert!(start.elapsed() < Duration::from_millis(300));
    assert_eq!(doc["lines"], 75001);
    let start = Instant::now();
    let result = diff(&fx, "lines").await;
    assert_eq!(result["added"], 0);
    assert!(start.elapsed() < Duration::from_secs(1));
    let big = "x\n".repeat(7_500_000);
    fx.app
        .write_queue()
        .submit(os_app::pool::CrawlKey("fx-ui-basic".into()), move |conn| {
            conn.execute(
                "UPDATE page_html SET body=?1 WHERE url_id=1",
                [big.as_bytes()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let doc = fx
        .call(
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":false}),
        )
        .await;
    assert_eq!(doc["bytes"], 15_000_000);
    assert_eq!(
        fx.call(
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":7_499_999,"count":2})
        )
        .await,
        json!(["x", ""])
    );
}
// spec: UI-039
// spec: UI-035
#[tokio::test]
async fn screenshot_tiles_and_stored_image_resolve_only_for_registered_crawls() {
    let fx = fixture();
    fx.write("INSERT INTO screenshots(url_id,tile,png,width,height) VALUES(1,0,x'010203',1024,4096),(1,1,x'040506',1024,4096); UPDATE urls SET content_class='Images',content_type='image/png' WHERE id=2; INSERT INTO page_html(url_id,body) VALUES(2,x'89504E470D0A1A0A0000000D4948445200000640000004B0');");
    let shot = fx.call("detail.screenshot", json!({"urlId":1})).await;
    assert_eq!(shot["height"], 8192);
    assert_eq!(shot["tiles"], 2);
    let uri = shot["url"].as_str().unwrap();
    assert_eq!(
        fx.app.resolve_asset(&format!("{uri}/1")).await.body,
        vec![4, 5, 6]
    );
    assert_eq!(
        fx.app.resolve_asset(&format!("{uri}/999")).await.status,
        404
    );
    assert_eq!(
        fx.app
            .resolve_asset("ospider://shot/../../etc/passwd")
            .await
            .status,
        404
    );
    let image = fx.call("detail.imagePreview", json!({"urlId":2})).await;
    assert_eq!(image["stored"], true);
    assert_eq!(image["width"], 1600);
    assert_eq!(image["height"], 1200);
    assert_eq!(
        fx.app
            .resolve_asset(image["url"].as_str().unwrap())
            .await
            .status,
        200
    );
    fx.app.sessions().clear(&os_app::WindowId("main".into()));
    assert_eq!(fx.app.resolve_asset(uri).await.status, 404);
}
// spec: UI-035
#[tokio::test]
async fn unstored_images_need_explicit_confirmation_and_svg_has_no_active_content() {
    let fx = fixture();
    fx.write("UPDATE urls SET content_class='Images',content_type='image/svg+xml' WHERE id=2; INSERT INTO page_html(url_id,body) VALUES(2,CAST('<svg xmlns=\"http://www.w3.org/2000/svg\" onload=\"alert(1)\"><script>alert(1)</script><foreignObject><div>bad</div></foreignObject><image href=\"https://example.test/a\"/><path d=\"M0 0L1 1\" onclick=\"bad()\"/></svg>' AS BLOB)); UPDATE urls SET content_class='Images' WHERE id=3;");
    let image = fx.call("detail.imagePreview", json!({"urlId":2})).await;
    let body = String::from_utf8(
        fx.app
            .resolve_asset(image["url"].as_str().unwrap())
            .await
            .body,
    )
    .unwrap();
    assert!(body.contains("<path"));
    for active in ["script", "onload", "onclick", "foreignObject", "https://"] {
        assert!(!body.contains(active), "{body}");
    }
    let image = fx.call("detail.imagePreview", json!({"urlId":3})).await;
    assert_eq!(image["stored"], false);
    assert_eq!(
        fx.app
            .resolve_asset(image["url"].as_str().unwrap())
            .await
            .status,
        404
    );
}
// spec: UI-044
// spec: ONPAGE-145
#[tokio::test]
async fn exact_duplicates_include_selected_text_word_diff_and_work_without_analysis() {
    let fx = fixture();
    let result = fx
        .call(
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","partnerId":2}),
        )
        .await;
    assert_eq!(result["partners"].as_array().unwrap().len(), 1);
    assert_eq!(result["partners"][0]["urlId"], 2);
    assert_eq!(result["partners"][0]["similarity"].as_f64().unwrap(), 100.0);
    assert_eq!(result["partners"][0]["matchType"], "Exact");
    assert_eq!(result["partners"][0]["diff"]["added"], 1);
    assert_eq!(result["text"], "one two");
    let error = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.duplicates",
            json!({"urlId":1,"kind":"near"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.args["state"]["code"], "NEEDS_CRAWL_ANALYSIS");
}
// spec: UI-050
#[tokio::test]
async fn crawl_path_is_root_first_with_discovery_anchors() {
    let fx = fixture();
    fx.write("INSERT INTO discovery_edges(seq,target_id,source_id,kind,link_type,anchor,depth) VALUES(1,1,NULL,'link',NULL,NULL,0),(2,2,1,'link','Hyperlink','go to blog',1);");
    let path = fx.call("crawl.path", json!({"urlId":2})).await;
    assert_eq!(path.as_array().unwrap().len(), 2);
    assert_eq!(path[0]["urlId"], 1);
    assert_eq!(path[1]["urlId"], 2);
    assert_eq!(path[1]["depth"], 1);
    assert_eq!(path[1]["anchor"], "go to blog");
}

// spec: OUT-023
// spec: UI-050
#[tokio::test]
async fn crawl_path_export_reverses_command_order_and_ends_at_root_with_fallback() {
    let fx = fixture();
    for discovery in [
        "DELETE FROM discovery_edges; INSERT INTO discovery_edges(seq,target_id,source_id,kind,link_type,anchor,depth) VALUES(1,1,NULL,'link',NULL,NULL,0),(2,2,1,'link','Hyperlink','go to blog',1)",
        "DELETE FROM discovery_edges",
    ] {
        fx.write(discovery);
        let path = fx.call("crawl.path", json!({"urlId":2})).await;
        assert_eq!(path.as_array().unwrap().len(), 2);
        let destination = fx.dir.path().join("crawl-path.csv");
        fx.op("export.detail", json!({
            "spec":{"kind":"detail","detail":"crawlPath","urlIds":[2],"params":{},"sort":[]},
            "selection":{"kind":"ids","ids":[2]},"path":destination,"format":"csv"
        })).await.unwrap();
        let csv = std::fs::read_to_string(&destination).unwrap();
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap().trim_start_matches('\u{feff}'),
            "Target Address,Step,Address,Status Code,Link Type,Anchor,Crawl Depth");
        let path = path.as_array().unwrap();
        assert_eq!(path[0]["address"], "https://site.test/");
        assert_eq!(path[1]["address"], "https://site.test/blog/a");
        for (step, item) in path.iter().rev().enumerate() {
            let columns = lines.next().unwrap().split(',').collect::<Vec<_>>();
            assert_eq!(columns[0], path[1]["address"].as_str().unwrap());
            assert_eq!(columns[1], step.to_string());
            assert_eq!(columns[2], item["address"].as_str().unwrap());
            assert_eq!(columns[3], item["statusCode"].to_string());
            assert_eq!(columns[5], item["anchor"].as_str().unwrap_or(""));
            assert_eq!(columns[6], item["depth"].to_string());
            if step == path.len() - 1 {
                assert_eq!(columns[2], "https://site.test/");
                assert_eq!(columns[4], "");
                assert_eq!(columns[5], "");
            }
        }
        assert!(lines.next().is_none());
    }
}

// spec: UI-037
#[tokio::test]
async fn serp_short_hint_uses_minimums_without_affecting_truncation() {
    let fx = fixture();
    for device in ["desktop", "mobile", "tablet"] {
        let args = json!({"title":"Short","description":"Brief","device":device,"boldTerms":[],"prefix":""});
        let measured = fx.call("detail.serpMeasure", args.clone()).await;
        for (index, minimum) in [(0, 200.), (1, 400.)] {
            assert_eq!(measured["rows"][index]["minPx"], minimum);
            assert_eq!(measured["rows"][index]["short"], true);
            assert_eq!(measured["rows"][index]["truncated"], false);
        }
        let mut cfg = (*fx
            .app
            .sessions()
            .current(&os_app::WindowId("main".into()))
            .unwrap()
            .config)
            .clone();
        cfg.title.min_px = measured["rows"][0]["px"].as_f64().unwrap() as u32;
        cfg.meta_description.min_px = measured["rows"][1]["px"].as_f64().unwrap() as u32;
        fx.app.sessions().update_config(
            &os_app::pool::CrawlKey("fx-ui-basic".into()),
            std::sync::Arc::new(cfg),
        );
        let boundary = fx.call("detail.serpMeasure", args).await;
        for index in 0..2 {
            assert_eq!(boundary["rows"][index]["short"], false);
            assert_eq!(
                boundary["rows"][index]["maxPx"],
                measured["rows"][index]["maxPx"]
            );
            assert_eq!(
                boundary["rows"][index]["preview"],
                measured["rows"][index]["preview"]
            );
        }
        fx.app.sessions().update_config(
            &os_app::pool::CrawlKey("fx-ui-basic".into()),
            std::sync::Arc::new(CrawlConfig::default()),
        );
    }
}

// spec: UI-037
// spec: ONPAGE-145
#[test]
fn additive_detail_fields_accept_legacy_payloads_and_stay_optional_in_typescript() {
    use ts_rs::TS;
    let row: SerpMeasureRow = serde_json::from_value(json!({
        "chars":3,"px":20.,"maxPx":561.,"remaining":541.,"truncated":false
    }))
    .unwrap();
    assert_eq!(
        (row.text, row.preview, row.short, row.min_px),
        (None, None, None, None)
    );
    let partner: DupPartner =
        serde_json::from_value(json!({"urlId":2,"address":"https://site.test/blog"})).unwrap();
    assert_eq!(partner.match_type, None);
    let config = ts_rs::Config::default();
    let row_type = SerpMeasureRow::decl(&config);
    for field in ["text?", "preview?", "short?", "minPx?"] {
        assert!(row_type.contains(field), "{row_type}");
    }
    assert!(DupPartner::decl(&config).contains("matchType?"));
    let duplicates: DuplicatesResult =
        serde_json::from_value(json!({"partners":[],"text":null})).unwrap();
    assert_eq!(duplicates.total, None);
    assert!(DuplicatesResult::decl(&config).contains("total?"));
}

fn findings(node: &Value, out: &mut Vec<Value>) {
    out.extend(node["issues"].as_array().unwrap().iter().cloned());
    for child in node["children"].as_array().unwrap() {
        findings(child, out);
    }
}
// spec: UI-045
// spec: TECH-189
#[tokio::test]
async fn structured_tree_keeps_format_property_order_and_each_finding_once() {
    let fx = fixture();
    fx.write(r#"INSERT INTO sd_items(id,url_id,format,type_path,json) VALUES
      (1,1,'json-ld','Product','{"@type":"Product","z":"last alphabetically","name":"Name","offers":[{"@type":"Offer","price":"5","_os_order":["@type","price"]}],"_os_order":["@type","z","name","offers"],"_os_source":{"line":3}}'),
      (2,1,'microdata','Person','{"@type":"Person","name":"A","_os_order":["@type","name"]}'),
      (3,1,'rdfa','Person','{"@type":"Person","name":"B"}'),
      (4,1,'json-ld','','{"_os_parse_error":{"message":"Invalid JSON at line 2","line":2,"column":4,"snippet":"{broken}"}}');
      INSERT INTO sd_issues(url_id,item_id,validation,severity,property,feature,code) VALUES
      (1,1,'rich_results','error','offers[0].priceCurrency','Merchant listing','sd.google.missing_required'),
      (1,1,'schema_org','warning','z',NULL,'sd.schema.unknown_property'),
      (1,4,'parse','error',NULL,NULL,'parser diagnostic'),
      (1,NULL,'rich_results','info',NULL,'Merchant listing','sd.google.feature');"#);
    let tree = fx.call("detail.structuredData", json!({"urlId":1})).await;
    assert_eq!(
        tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["format"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["json-ld", "microdata", "rdfa"]
    );
    let product = &tree["nodes"][0]["children"][0];
    assert_eq!(product["sourceLine"], 3);
    assert_eq!(
        product["propertyOrder"],
        json!(["@type", "z", "name", "offers"])
    );
    assert_eq!(product["children"][1]["path"], "z");
    let mut all = vec![];
    for node in tree["nodes"].as_array().unwrap() {
        findings(node, &mut all);
    }
    assert_eq!(all.len(), 3);
    let missing = all
        .iter()
        .find(|i| i["code"] == "sd.google.missing_required")
        .unwrap();
    assert_eq!(missing["path"], "offers[0].priceCurrency");
    assert!(missing["message"]
        .as_str()
        .unwrap()
        .contains("priceCurrency"));
    assert!(missing["docUrl"].as_str().unwrap().starts_with("https://"));
    let parse = all.iter().find(|i| i["validation"] == "parse").unwrap();
    assert_eq!(parse["line"], 2);
    assert_eq!(parse["col"], 4);
    assert_eq!(parse["snippet"], "{broken}");
    assert_eq!(tree["nodes"][0]["children"][1]["typePath"], "Parse Error");
}
// spec: UI-041
// spec: UI-039
// spec: UI-045
#[tokio::test]
async fn missing_settings_and_unknown_urls_have_explicit_states() {
    let fx = Fx::new();
    for (command, args) in [
        (
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":false}),
        ),
        ("detail.screenshot", json!({"urlId":1})),
        ("detail.structuredData", json!({"urlId":1})),
    ] {
        // Shared default SD configuration may enable extraction; disable it explicitly.
        let mut cfg = CrawlConfig::default();
        cfg.extraction.structured_data.json_ld = false;
        cfg.extraction.structured_data.microdata = false;
        cfg.extraction.structured_data.rdfa = false;
        fx.app.sessions().update_config(
            &os_app::pool::CrawlKey("fx-ui-basic".into()),
            std::sync::Arc::new(cfg),
        );
        let err = fx
            .app
            .dispatch(CallCtx::for_window("main"), command, args)
            .await
            .unwrap_err();
        assert_eq!(err.message_key, "err-detail-unavailable", "{command}");
    }
    let err = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.serp",
            json!({"urlId":999,"device":"desktop"}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}
// spec: UI-041
#[tokio::test]
async fn pretty_print_is_display_only_and_keeps_scripts_and_quoted_attributes() {
    let fx = fixture();
    let original="<body><div data-x=\"a>b\"><p>Ä</p><script>if (a < b) { x = '<div>'; }</script></div></body>";
    let text = original.to_owned();
    fx.app
        .write_queue()
        .submit(os_app::pool::CrawlKey("fx-ui-basic".into()), move |conn| {
            conn.execute(
                "UPDATE page_html SET body=?1 WHERE url_id=1",
                [text.as_bytes()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let doc = fx
        .call(
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":true}),
        )
        .await;
    assert!(doc["lines"].as_u64().unwrap() > 1);
    let lines = fx
        .call(
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":0,"count":100}),
        )
        .await;
    let pretty = lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(pretty.contains("if (a < b) { x = '<div>'; }"));
    assert!(pretty.contains("data-x=\"a>b\""));
    let doc = fx
        .call(
            "detail.sourceOpen",
            json!({"urlId":1,"which":"original","mode":"html","pretty":false}),
        )
        .await;
    assert_eq!(
        fx.call(
            "detail.sourceLines",
            json!({"docId":doc["docId"],"from":0,"count":1})
        )
        .await,
        json!([original])
    );
}

// spec: UI-044
// spec: ONPAGE-145
#[tokio::test]
async fn near_partners_use_analysis_order_rounding_and_current_config() {
    let fx = fixture();
    let s = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    let file = os_store::CrawlFile::open(&fx.path, os_store::OpenMode::ReadWrite).unwrap();
    let hash =
        os_analyze::state::step_config_hash(os_model::AnalysisStep::NearDuplicates, &s.config);
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        os_model::StepState::Done,
        Some(&os_model::Timestamp::UNIX_EPOCH),
        Some(&hash),
    )
    .unwrap();
    file.conn()
        .execute_batch(
            "INSERT INTO near_duplicates(url_id,other_id,similarity) VALUES(1,2,0.954),(1,3,0.91)",
        )
        .unwrap();
    drop(file);
    let result = fx
        .call("detail.duplicates", json!({"urlId":1,"kind":"near"}))
        .await;
    assert_eq!(result["partners"].as_array().unwrap().len(), 2);
    assert_eq!(result["partners"][0]["similarity"].as_f64().unwrap(), 95.0);
    assert_eq!(result["partners"][1]["similarity"].as_f64().unwrap(), 91.0);
    assert_eq!(result["partners"][0]["matchType"], "Near");
    let mut cfg = (*s.config).clone();
    cfg.duplicates.near_threshold_pct = 99;
    fx.app
        .sessions()
        .update_config(&s.key, std::sync::Arc::new(cfg));
    assert!(fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.duplicates",
            json!({"urlId":1,"kind":"near"})
        )
        .await
        .is_err());
}
// spec: UI-037
// spec: UI-038
#[tokio::test]
async fn five_hundred_character_measurement_fits_one_frame() {
    let fx = fixture();
    let a = json!({"title":"Test title ".repeat(20),"description":"A description ".repeat(20),"device":"desktop","boldTerms":["Test","description"],"prefix":""});
    fx.call("detail.serpMeasure", a.clone()).await;
    let mut samples = vec![];
    for _ in 0..5 {
        let start = Instant::now();
        fx.call("detail.serpMeasure", a.clone()).await;
        samples.push(start.elapsed());
    }
    samples.sort();
    assert!(samples[2] < Duration::from_millis(16), "{samples:?}");
}

// spec: UI-035
#[tokio::test]
async fn confirmed_live_preview_fetches_only_the_selected_stored_url() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fx = fixture();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/image.png");
    fx.app
        .write_queue()
        .submit(os_app::pool::CrawlKey("fx-ui-basic".into()), move |conn| {
            conn.execute(
                "UPDATE urls SET content_class='Images',url=?1 WHERE id=2",
                [url],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let preview = fx.call("detail.imagePreview", json!({"urlId":2})).await;
    assert_eq!(
        fx.app
            .resolve_asset(preview["url"].as_str().unwrap())
            .await
            .status,
        404
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), listener.accept())
            .await
            .is_err()
    );
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 8\r\nConnection: close\r\n\r\n\x89PNG\r\n\x1a\n").await.unwrap();
    });
    let preview = fx
        .call("detail.imagePreview", json!({"urlId":2,"allowLive":true}))
        .await;
    let image = tokio::time::timeout(
        Duration::from_secs(5),
        fx.app.resolve_asset(preview["url"].as_str().unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(image.status, 200);
    assert_eq!(image.body, b"\x89PNG\r\n\x1a\n");
    server.await.unwrap();
}

// spec: UI-050
#[tokio::test]
async fn directly_uploaded_urls_identify_their_origin_without_a_link_chain() {
    let fx = fixture();
    fx.write("UPDATE urls SET discovered_via='list_input',crawl_depth=0 WHERE id=2; INSERT INTO discovery_edges(seq,target_id,source_id,kind,depth) VALUES(1,2,NULL,'list_input',0)");
    let path = fx.call("crawl.path", json!({"urlId":2})).await;
    assert_eq!(path.as_array().unwrap().len(), 1);
    assert_eq!(path[0]["via"], "list_input");
    assert_eq!(path[0]["depth"], 0);
}

// spec: UI-037
// spec: UI-038
#[test]
fn bold_matching_preserves_emoji_clusters_and_combining_marks() {
    let family = "👨\u{200d}👩\u{200d}👧\u{200d}👦";
    assert_eq!(
        os_parse::pixel::width_px_with_bold_terms(family, 20., &["👨".into()]),
        20
    );
    assert_eq!(
        os_parse::pixel::width_px_with_bold_terms("e\u{301}", 20., &["e".into()]),
        os_parse::pixel::width_px("e\u{301}", 20., true)
    );
    assert_eq!(
        os_parse::pixel::width_px_with_bold_terms("Start", 20., &["unmatched".into()]),
        os_parse::pixel::width_px("Start", 20., false)
    );
}

// spec: UI-037
// spec: ONPAGE-194
#[tokio::test]
async fn bold_terms_preserve_unicode_word_boundaries_and_short_hint_uses_minimum() {
    let fx = fixture();
    for text in [
        "alpha-beta-gamma-delta ".repeat(10),
        "漢字中文你好世界".repeat(10),
        "😀 café-été ".repeat(15),
    ] {
        let request = |terms: Vec<&str>| json!({"title":text,"description":"short","device":"desktop","boldTerms":terms,"prefix":""});
        let plain = fx.call("detail.serpMeasure", request(vec![])).await;
        let unmatched = fx
            .call("detail.serpMeasure", request(vec!["unmatched"]))
            .await;
        assert_eq!(plain, unmatched, "{text}");
        assert_eq!(plain["rows"][1]["short"], true);
        assert_eq!(plain["rows"][1]["truncated"], false);
        let bold = fx
            .call("detail.serpMeasure", request(vec!["alpha", "漢", "café"]))
            .await;
        for row in bold["rows"].as_array().unwrap() {
            if let Some(cut) = row["cutAt"].as_u64() {
                let prefix = row["text"]
                    .as_str()
                    .unwrap()
                    .chars()
                    .take(cut as usize)
                    .collect::<String>();
                assert_eq!(row["preview"], format!("{prefix} …"));
            }
        }
    }
}

// spec: UI-044
// spec: ONPAGE-145
#[tokio::test]
async fn duplicates_always_include_focus_text_and_load_only_the_selected_partner() {
    let fx = fixture();
    fx.write("WITH RECURSIVE n(i) AS (VALUES(20) UNION ALL SELECT i+1 FROM n WHERE i<1020) INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code,hash_md5) SELECT i,'https://site.test/dup/'||i,'https://site.test/dup/'||i,'https://site.test',1,'done','link',1,30,'HTML',200,'same' FROM n; INSERT INTO url_ext(url_id,key,v_text) SELECT id,'content.text','one two three' FROM urls WHERE id>=20");
    let first = fx
        .call(
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","limit":2}),
        )
        .await;
    assert_eq!(first["total"], 1002);
    assert_eq!(first["nextOffset"], 2);
    assert_eq!(first["text"], "one two");
    for partner in first["partners"].as_array().unwrap() {
        assert_eq!(partner["text"], Value::Null);
        assert_eq!(partner["diff"], Value::Null);
    }
    let selected = first["partners"][0]["urlId"].as_i64().unwrap();
    // U21 fetches the other column by making the partner the focus, without partnerId.
    let other = fx
        .call(
            "detail.duplicates",
            json!({"urlId":selected,"kind":"exact","limit":2}),
        )
        .await;
    assert_eq!(other["text"], "one two three");
    for partner in other["partners"].as_array().unwrap() {
        assert_eq!(partner["text"], Value::Null);
        assert_eq!(partner["diff"], Value::Null);
    }
    let focused = fx
        .call(
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","limit":2,"partnerId":selected}),
        )
        .await;
    assert_eq!(focused["text"], "one two");
    assert_eq!(focused["partners"][0]["diff"]["added"], 1);
    assert_eq!(focused["partners"][1]["text"], Value::Null);
    let last = fx
        .call(
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","offset":1000,"limit":500}),
        )
        .await;
    assert_eq!(last["partners"].as_array().unwrap().len(), 2);
    assert_eq!(last["nextOffset"], Value::Null);
    let capped = fx
        .call(
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","limit":10000}),
        )
        .await;
    assert_eq!(capped["partners"].as_array().unwrap().len(), 500);
    assert!(fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.duplicates",
            json!({"urlId":1,"kind":"exact","limit":1,"partnerId":1})
        )
        .await
        .is_err());
}

fn marked_positions(diff: &Value) -> (Vec<u64>, Vec<u64>) {
    let mut removed = vec![];
    let mut added = vec![];
    for hunk in diff["hunks"].as_array().unwrap() {
        let mut old = hunk["oldStart"].as_u64().unwrap() - 1;
        let mut new = hunk["newStart"].as_u64().unwrap() - 1;
        for line in hunk["lines"].as_array().unwrap() {
            match line["kind"].as_str().unwrap() {
                "removed" => {
                    removed.push(old);
                    old += 1;
                }
                "added" => {
                    added.push(new);
                    new += 1;
                }
                "context" => {
                    old += 1;
                    new += 1;
                }
                _ => unreachable!(),
            }
        }
    }
    (removed, added)
}

// spec: ONPAGE-145
// spec: UI-044
#[tokio::test]
async fn selected_partner_diff_matches_every_fixture_word_position() {
    let fx = fixture();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/onpage-m2-content/thresholds");
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected_diff.json")).unwrap())
            .unwrap();
    let base = os_app::source::visible_text(
        &std::fs::read_to_string(root.join("near-base.html")).unwrap(),
    );
    let session = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    let file = os_store::CrawlFile::open(&fx.path, os_store::OpenMode::ReadWrite).unwrap();
    let hash = os_analyze::state::step_config_hash(
        os_model::AnalysisStep::NearDuplicates,
        &session.config,
    );
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        os_model::StepState::Done,
        Some(&os_model::Timestamp::UNIX_EPOCH),
        Some(&hash),
    )
    .unwrap();
    file.conn()
        .execute(
            "INSERT INTO near_duplicates(url_id,other_id,similarity) VALUES(1,2,0.95)",
            [],
        )
        .unwrap();
    drop(file);
    for (name, expected) in expected.as_object().unwrap() {
        let partner = os_app::source::visible_text(
            &std::fs::read_to_string(root.join(format!("near-{name}.html"))).unwrap(),
        );
        let (left, right) = (base.clone(), partner.clone());
        fx.app
            .write_queue()
            .submit(session.key.clone(), move |conn| {
                conn.execute(
                    "UPDATE url_ext SET v_text=?1 WHERE url_id=1 AND key='content.text'",
                    [left],
                )?;
                conn.execute(
                    "UPDATE url_ext SET v_text=?1 WHERE url_id=2 AND key='content.text'",
                    [right],
                )?;
                Ok(())
            })
            .await
            .unwrap();
        // The persisted relation deliberately stays above the threshold: test the command's exact
        // markers for every generator pair independently of the analysis threshold tests.
        let response = fx
            .call(
                "detail.duplicates",
                json!({"urlId":1,"kind":"near","partnerId":2}),
            )
            .await;
        let expected: Vec<u64> = serde_json::from_value(expected.clone()).unwrap();
        let word_diff = &response["partners"][0]["diff"];
        assert_eq!(
            marked_positions(word_diff),
            (expected.clone(), expected.clone()),
            "near-{name}"
        );
        assert_eq!(word_diff["added"], expected.len());
        assert_eq!(word_diff["removed"], expected.len());
        let repeated = fx
            .call(
                "detail.duplicates",
                json!({"urlId":1,"kind":"near","partnerId":2}),
            )
            .await;
        assert_eq!(response, repeated);
    }
}

// spec: UI-039
// spec: UI-041
#[tokio::test]
async fn source_and_screenshot_save_preserve_stored_bytes_and_report_io_errors() {
    let fx = fixture();
    let html_path = fx.dir.path().join("source.html");
    let text_path = fx.dir.path().join("source.txt");
    let png_path = fx.dir.path().join("screenshot.png");
    fx.write(
        "INSERT INTO screenshots(url_id,tile,width,height,png) VALUES(1,0,2,2,X'89504E470D0A1A0A')",
    );
    let original = fx
        .call(
            "detail.sourceSave",
            json!({"urlId":1,"which":"original","mode":"html","path":html_path}),
        )
        .await;
    let bytes = std::fs::read(&html_path).unwrap();
    assert_eq!(original["bytes"], bytes.len());
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("<script>hidden</script>"));
    fx.call(
        "detail.sourceSave",
        json!({"urlId":1,"which":"rendered","mode":"text","path":text_path}),
    )
    .await;
    assert_eq!(
        std::fs::read_to_string(text_path).unwrap(),
        "one two three four five six"
    );
    fx.call("detail.screenshotSave", json!({"urlId":1,"path":png_path}))
        .await;
    assert_eq!(std::fs::read(png_path).unwrap(), b"\x89PNG\r\n\x1a\n");
    let error = fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.sourceSave",
            json!({"urlId":1,"which":"original","mode":"html","path":fx.dir.path()}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Io);
    assert!(fx
        .app
        .dispatch(
            CallCtx::for_window("main"),
            "detail.screenshotSave",
            json!({"urlId":1,"tile":1,"path":html_path})
        )
        .await
        .is_err());
}

// spec: ONPAGE-157
// spec: ONPAGE-158
#[tokio::test]
async fn spelling_actions_recheck_stored_content_and_keep_keys_when_sorted() {
    let fx = fixture();
    let mut session = fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap();
    let mut cfg = (*session.config).clone();
    cfg.spelling.enabled = true;
    cfg.grammar.enabled = true;
    session.config = std::sync::Arc::new(cfg);
    fx.app
        .sessions()
        .set_current(os_app::WindowId("main".into()), session.clone());
    fx.write("UPDATE urls SET title_1='zebra apple',meta_desc_1=NULL WHERE id=1; UPDATE url_ext SET v_text='This is is a test.' WHERE url_id=1 AND key='content.text'; INSERT INTO spelling_findings(id,url_id,kind,language,section,text,detail,suggestion) VALUES(101,1,'Spelling','en-US','Page Title','zebra','err',NULL),(305,1,'Spelling','en-US','Page Title','apple','err',NULL)");
    let handle = fx.open(json!({"kind":"detail","detail":"spelling","urlIds":[1],"params":{},"sort":[{"column":"Error","dir":"asc"}]}), json!([])).await;
    let rows = fx.rows(&handle, 0, 100, json!([])).await;
    assert_eq!(rows["rowKeys"], json!([2, 1]));
    let context = fx
        .call(
            "detail.spellingContext",
            json!({"urlId":1,"findingId":rows["rowKeys"][0]}),
        )
        .await;
    assert_eq!(
        context["text"]
            .as_str()
            .unwrap()
            .chars()
            .skip(context["start"].as_u64().unwrap() as usize)
            .take(5)
            .collect::<String>(),
        "apple"
    );
    for (command, args) in [
        ("detail.spellingIgnore", json!({"word":"zebra"})),
        (
            "detail.spellingDictionary",
            json!({"language":"en-US","word":"xyzzword"}),
        ),
        (
            "detail.spellingIgnore",
            json!({"urlId":1,"ruleId":"WORD_REPEAT"}),
        ),
        ("detail.spellingRecheck", json!({})),
    ] {
        let handle: OpHandle = serde_json::from_value(fx.call(command, args).await).unwrap();
        assert!(handle.cancellable);
        let status = fx.app.ops().wait(&handle.op_id).await.unwrap();
        assert_eq!(status.state, OpState::Done, "{status:?}");
        assert_eq!(status.label_key, "op-spelling-recheck");
        assert_eq!(status.total, Some(status.done));
    }
    let file = os_store::CrawlFile::open(&fx.path, os_store::OpenMode::ReadOnly).unwrap();
    assert!(os_analyze::tabs::content::ignored_spelling_words(&file)
        .unwrap()
        .contains(&"zebra".into()));
    assert!(fx
        .app
        .sessions()
        .current(&os_app::WindowId("main".into()))
        .unwrap()
        .config
        .grammar
        .disabled_rules
        .contains(&"WORD_REPEAT".into()));
    assert!(
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM spelling_findings WHERE rule_id='WORD_REPEAT'",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap()
            == 0
    );
    assert!(std::fs::read_to_string(
        fx.app
            .config()
            .paths
            .settings
            .join("dictionaries/personal/en-US.txt")
    )
    .unwrap()
    .contains("xyzzword"));
}

// spec: UI-041
// spec: UI-145
#[test]
fn divergent_diff_rejects_deterministically_instead_of_approximating() {
    let cancel = os_analyze::analysis::CancelToken::new();
    let left = "left ".repeat(2100);
    let right = "right ".repeat(2100);
    for _ in 0..3 {
        let error = os_app::source::diff(&left, &right, DiffMode::Words, &cancel).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(error.message_key, "err-source-diff-too-complex");
        assert_eq!(error.field.as_deref(), Some("text"));
    }
}

// spec: UI-048
// spec: TECH-259
#[tokio::test]
async fn a11y_detail_keys_are_node_ids_across_sorting_and_pages() {
    let fx = fixture();
    fx.write("INSERT INTO a11y_nodes(id,url_id,rule_id,target,snippet,summary) VALUES(19,1,'z-rule','#z','<img>','z'),(42,1,'a-rule','#a','<img>','a')");
    let handle = fx.open(json!({"kind":"detail","detail":"a11yNodes","urlIds":[1],"params":{},"sort":[{"column":"rule_id","dir":"asc"}]}), json!([])).await;
    let first = fx.rows(&handle, 0, 1, json!([])).await;
    let second = fx.rows(&handle, 1, 1, json!([])).await;
    assert_eq!(first["rowKeys"], json!([42]));
    assert_eq!(second["rowKeys"], json!([19]));
}

// spec: ONPAGE-157
#[tokio::test]
async fn dictionary_action_validates_language_and_word_before_starting_an_operation() {
    let fx = fixture();
    for args in [
        json!({"language":"../../outside","word":"test"}),
        json!({"language":"en-US","word":"one\ntwo"}),
    ] {
        let error = fx
            .app
            .dispatch(
                CallCtx::for_window("main"),
                "detail.spellingDictionary",
                args,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
    }
}
