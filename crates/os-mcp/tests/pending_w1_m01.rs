//! Complete cross-package integration check. The rendering implementation is provided by E05/R01.
#[path = "tools_support.rs"]
mod support;
use os_mcp::cli::{build_server, McpArgs};
use os_mcp::settings::{self, McpSettings};
use os_model::CrawlConfig;
use os_store::paths::app_paths_with_home;
use serde_json::json;

// spec: AGENT-020
#[tokio::test]
#[ignore = "wartet auf M2-E05"]
async fn a_saved_rendering_profile_is_used_by_headless_mcp() {
    let home = tempfile::tempdir().unwrap();
    let paths = app_paths_with_home(Some(home.path().as_os_str()));
    let fixture = support::fixtures_dir().join("agent-m2-crawl");
    let site = os_testsite::TestSite::start_dir(fixture, 0).await.unwrap();
    std::fs::create_dir_all(paths.settings.join("profiles")).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    cfg.extraction.store_rendered_html = true;
    cfg.save_file(&paths.settings.join("profiles").join("render.osconfig"))
        .unwrap();
    let settings = McpSettings {
        allow_private_targets: true,
        ..McpSettings::default()
    };
    settings::save(&settings::path_in(&paths), &settings).unwrap();
    let server = build_server(
        &McpArgs {
            allow: vec!["crawl".into()],
            ..McpArgs::default()
        },
        &paths,
        os_mcp::tools::all(),
    )
    .unwrap();
    let client = support::Client::connect(server).await;
    let result = client
        .call(
            "start_crawl",
            json!({"url":site.url("/"),"config_profile":"render"}),
        )
        .await;
    assert!(!result.is_error, "{:?}", result.blocks);
    let id = result.summary()["crawl_id"].as_str().unwrap();
    let path = os_store::registry::find(&paths.crawls, &id.parse().unwrap()).unwrap();
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    assert_eq!(
        file.config().unwrap().rendering.mode,
        os_model::RenderingMode::JavaScript
    );
    drop(file);
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        loop {
            let status = client.call("crawl_status", json!({"crawl_id":id})).await;
            assert!(!status.is_error, "{:?}", status.blocks);
            if status.summary()["progress"]["state"] == "Completed" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let pages: i64 = file
        .conn()
        .query_row(
            "SELECT count(*) FROM render_pages WHERE html IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        pages >= 2,
        "rendered HTML is persisted for both fixture pages"
    );
    client.cancel().await;
}
