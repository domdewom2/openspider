#[path = "tools_support.rs"]
mod support;
use os_mcp::{
    tools::{
        compare_crawls::CompareCrawls,
        link_metrics::{ApiEnvironment, FetchLinkMetrics},
    },
    Tool, ToolContext,
};
use os_model::{ApiKind, ApiLevel, CrawlConfig};
use serde_json::json;
use std::sync::Arc;
use support::*;

// spec: AGENT-015
#[tokio::test]
async fn comparison_matches_app_classes_swaps_and_rejects_unknown_or_running_crawls() {
    let world = World::new();
    let b = world
        .add(
            "out-m2-compare/v1",
            "http://www.site.test/",
            CrawlConfig::default(),
            41,
        )
        .await;
    let a = world
        .add(
            "out-m2-compare/v2",
            "http://www.site.test/",
            CrawlConfig::default(),
            42,
        )
        .await;
    let ctx = ToolContext::new(world.dir.path().into());
    let result = CompareCrawls
        .call(&ctx, json!({"a":a.id,"b":b.id}))
        .unwrap();
    let reversed = CompareCrawls
        .call(&ctx, json!({"a":b.id,"b":a.id}))
        .unwrap();
    let side = |id: &str| {
        os_analyze::compare::CrawlSide::open(
            &world.dir.path().join(format!("{id}.ospider")),
            world.dir.path(),
        )
        .unwrap()
    };
    let mut comparison = os_analyze::compare::Comparison::open(
        side(&a.id),
        side(&b.id),
        &[],
        &os_model::CancelToken::new(),
    )
    .unwrap();
    if comparison.current().meta.crawl_id.to_string() != a.id {
        comparison.swap();
    }
    let expected = comparison.overview(&os_model::CancelToken::new()).unwrap();
    let rows = result.summary["filters"].as_array().unwrap();
    assert_eq!(rows.len(), expected.len());
    for (r, e) in rows.iter().zip(expected) {
        use os_analyze::compare::CompareClass::*;
        for (name, class) in [
            ("added", Added),
            ("new", New),
            ("removed", Removed),
            ("missing", Missing),
        ] {
            assert_eq!(r[name], json!(e.class(class)));
        }
        let back = reversed.summary["filters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["tab"] == r["tab"] && v["filter"] == r["filter"])
            .unwrap();
        assert_eq!(r["added"], back["removed"]);
        assert_eq!(r["new"], back["missing"]);
    }
    assert!(CompareCrawls
        .call(&ctx, json!({"a":"unknown","b":b.id}))
        .is_err());
    let file = os_store::CrawlFile::open(
        &world.dir.path().join(format!("{}.ospider", a.id)),
        os_store::OpenMode::ReadWrite,
    )
    .unwrap();
    file.set_meta("state", "Running").unwrap();
    file.set_meta("heartbeat_at", &os_model::Timestamp::now_utc().to_db())
        .unwrap();
    assert!(CompareCrawls
        .call(&ctx, json!({"a":a.id,"b":b.id}))
        .is_err());
}
// spec: AGENT-015
#[tokio::test]
#[ignore = "Klärung K2-72: wartet auf M2-C02"]
async fn page_title_changes_are_counted_and_paginated() {
    let world = World::new();
    let b = world
        .add(
            "out-m2-compare/v1",
            "http://www.site.test/",
            CrawlConfig::default(),
            41,
        )
        .await;
    let a = world
        .add(
            "out-m2-compare/v2",
            "http://www.site.test/",
            CrawlConfig::default(),
            42,
        )
        .await;
    let ctx = ToolContext::new(world.dir.path().into());
    let summary = CompareCrawls
        .call(&ctx, json!({"a":a.id,"b":b.id}))
        .unwrap();
    assert!(!summary.summary["change_detection"]
        .as_array()
        .unwrap()
        .is_empty());
    let first = CompareCrawls
        .call(
            &ctx,
            json!({"a":a.id,"b":b.id,"element":"page_title","limit":1}),
        )
        .unwrap();
    assert_eq!(first.total, Some(1));
    let mut urls = first.untrusted.unwrap()["urls"].as_array().unwrap().clone();
    let mut cursor = first.next_cursor;
    while let Some(c) = cursor {
        let page = CompareCrawls
            .call(
                &ctx,
                json!({"a":a.id,"b":b.id,"element":"page_title","limit":1,"cursor":c}),
            )
            .unwrap();
        urls.extend(page.untrusted.unwrap()["urls"].as_array().unwrap().clone());
        cursor = page.next_cursor;
    }
    assert_eq!(urls.len() as u64, first.total.unwrap());
    let unique: std::collections::HashSet<_> = urls.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(urls.len(), unique.len());
    assert_eq!(urls, vec![json!("http://www.site.test/dup1.html")]);
}
// spec: AGENT-003 [M2]
#[tokio::test]
async fn paid_link_metrics_estimate_does_not_call_provider_and_confirm_does() {
    use os_integrations::credentials::{Credential, Credentials};
    use os_testsite::mocks::{ahrefs::AhrefsMock, MockServer};
    let mock = Arc::new(AhrefsMock::default());
    let http = MockServer::start(vec![mock.clone()]).await.unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.api.ahrefs.base_url = Some(http.base_url("ahrefs"));
    cfg.api.ahrefs.levels = vec![ApiLevel::Domain];
    cfg.api.ahrefs.metrics = vec!["backlinks".into()];
    let world = World::new();
    let stored = world
        .add("out-basic-site", "http://out-basic-site.test/", cfg, 61)
        .await;
    let secrets = Arc::new(Credentials::new(Arc::new(
        os_fetch::MemorySecretStore::new(),
    )));
    secrets
        .put(
            ApiKind::Ahrefs,
            Credential {
                account_id: "fixture".into(),
                label: "Fixture".into(),
                api_key: None,
                access_token: Some("fixture-token".into()),
                refresh_token: None,
                expires_at: None,
                client_secret: None,
                scopes: vec![],
            },
            false,
        )
        .unwrap();
    let mut ctx = ToolContext::new(world.dir.path().into());
    ctx.api_environment = Some(ApiEnvironment {
        secrets,
        network: Default::default(),
        allow_api_override: true,
    });
    let args = json!({"crawl_id":stored.id,"provider":"ahrefs","urls":["https://example.com/a"]});
    let estimate = FetchLinkMetrics.call(&ctx, args.clone()).unwrap();
    assert_eq!(estimate.summary["confirmation_required"], true);
    assert_eq!(mock.requests.all().len(), 0);
    let mut args = args;
    args["confirm"] = json!(true);
    let result = tokio::task::spawn_blocking(move || FetchLinkMetrics.call(&ctx, args))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.summary["confirmation_required"], false);
    assert!(!mock.requests.all().is_empty());
}
// spec: AGENT-012 [M2]
#[tokio::test]
async fn structured_data_and_rendered_html_are_untrusted_sections() {
    let world = World::new();
    let stored = world
        .add(
            "out-basic-site",
            "http://out-basic-site.test/",
            CrawlConfig::default(),
            62,
        )
        .await;
    let path = world.dir.path().join(format!("{}.ospider", stored.id));
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
    let (id, url): (i64, String) = file
        .conn()
        .query_row("SELECT id,url FROM urls ORDER BY id LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    file.conn().execute("INSERT INTO sd_items(url_id,format,type_path,json) VALUES(?1,'json-ld','Thing','{\"name\":\"UNTRUSTED-SD\"}')",[id]).unwrap();
    file.conn()
        .execute(
            "INSERT INTO render_pages(url_id,status,html) VALUES(?1,'ok',?2)",
            (id, b"<html>UNTRUSTED-DOM</html>".as_slice()),
        )
        .unwrap();
    drop(file);
    let reply = world
        .server()
        .call(
            "url_details",
            Some(
                json!({"crawl_id":stored.id,"url":url,"sections":["structured_data","rendering"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert_eq!(reply.is_error, Some(false));
    let summary = &reply.content[0].as_text().unwrap().text;
    assert!(!summary.contains("UNTRUSTED-"));
    let data = &reply.content[1].as_text().unwrap().text;
    assert!(data.contains("UNTRUSTED-SD") && data.contains("UNTRUSTED-DOM"));
}

// spec: AGENT-003 [M2]
#[tokio::test]
async fn revoked_permissions_apply_to_existing_server_clones() {
    let world = World::new();
    let server = world.server();
    let connection = server.clone();
    let mut settings = os_mcp::McpSettings::default();
    settings.tools.insert("list_crawls".into(), false);
    server
        .set_permissions(os_mcp::permissions::Permissions::resolve(&[], false, &settings).unwrap());
    assert!(!connection.exposed_tool_names().contains(&"list_crawls"));
    assert_eq!(
        connection.call("list_crawls", None).await.is_error,
        Some(true)
    );
}

// spec: AGENT-020 [M2]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chromium_uses_the_connection_guard_for_document_requests() {
    let site = os_testsite::TestSite::start_dir(fixtures_dir().join("out-basic-site"), 0)
        .await
        .unwrap();
    let url = format!("http://127.0.0.1:{}/", site.port());
    let config = CrawlConfig::default();
    let opts = os_render::RenderOpts {
        ajax_timeout_ms: 100,
        response_timeout_ms: 3000,
        page_timeout_ms: 5000,
        ..Default::default()
    };
    let protected =
        os_mcp::render_guard::launch(&config, os_mcp::permissions::DomainPolicy::default())
            .await
            .unwrap();
    let _ = protected.render(&url, &opts).await;
    assert!(site.requests().is_empty(), "Chromium bypassed the gateway");
    drop(protected);
    let allowed =
        os_mcp::render_guard::launch(&config, os_mcp::permissions::DomainPolicy::new(&[], true))
            .await
            .unwrap();
    let page = allowed.render(&url, &opts).await.unwrap();
    assert!(!page.dom_html.is_empty());
    assert!(!site.requests().is_empty());
}
