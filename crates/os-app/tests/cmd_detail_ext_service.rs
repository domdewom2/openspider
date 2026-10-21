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
async fn exact_duplicates_include_text_word_diff_and_work_without_analysis() {
    let fx = fixture();
    let result = fx
        .call("detail.duplicates", json!({"urlId":1,"kind":"exact"}))
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
