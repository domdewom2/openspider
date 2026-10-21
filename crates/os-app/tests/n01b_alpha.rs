//! M2-N01b: Alpha-Befunde A01-15 (XLSX-Blattname) und A01-04 (config.set ohne Crawl).
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
#[path = "session_common.rs"]
mod session;
use fixture::*;
use serde_json::json;
use std::io::Read;

fn workbook_xml(path: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut workbook = String::new();
    archive
        .by_name("xl/workbook.xml")
        .unwrap()
        .read_to_string(&mut workbook)
        .unwrap();
    workbook
}

// spec: OUT-007
// spec: OUT-001 [M2]
#[tokio::test]
async fn xlsx_sheet_names_of_ui_exports_equal_the_cli_names() {
    let fx = Fx::new();
    let h = fx.open(tab(), json!(["Address", "Word Count"])).await;
    let view = fx
        .op(
            "export.view",
            json!({"viewId":h["viewId"],"version":42,"path":fx.dir.path().join("view.xlsx"),"format":"xlsx","allColumns":false}),
        )
        .await
        .unwrap();
    assert!(workbook_xml(&view.path).contains("name=\"Internal All\""));
    let bulk = fx
        .op(
            "export.bulk",
            json!({"name":"Links:All Inlinks","dir":fx.dir.path().join("bulk"),"format":"xlsx","singleWorkbook":false}),
        )
        .await
        .unwrap();
    assert!(bulk.path.ends_with("links_all_inlinks.xlsx"));
    assert!(workbook_xml(&bulk.path).contains("name=\"Links All Inlinks\""));
}

// spec: CRAWL-126 [M2]
// spec: OUT-220
#[tokio::test]
async fn config_set_without_an_open_crawl_applies_to_crawl_start() {
    let site = os_testsite::TestSite::start_dir(session::fixture(), 0)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let app = session::app(dir.path());
    let mut config = session::cfg();
    config.speed.threads = 1;
    config.speed.max_urls_per_sec = Some(50.0);
    let dto = serde_json::to_value(os_app::dto::CrawlConfigDto {
        schema_version: config.schema_version,
        generator: config.generator.clone(),
        config: serde_json::to_value(&config).unwrap(),
    })
    .unwrap();
    session::call(&app, "config.set", dto).await;
    let req = session::request(site.url("/"));
    session::call(&app, "crawl.start", serde_json::to_value(req).unwrap()).await;
    session::completed(&app).await;
    let started = app.sessions().current(&session::window()).unwrap().config;
    assert_eq!(started.speed.threads, 1);
    assert_eq!(started.speed.max_urls_per_sec, Some(50.0));
}

// spec: CRAWL-102
#[tokio::test]
async fn robots_tester_uses_the_draft_user_agent_of_the_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let app = session::app(dir.path());
    let content = "User-agent: *\nDisallow: /blog/\nUser-agent: Special\nAllow: /blog/\n";
    let call = |extra: serde_json::Value| {
        let mut args = json!({"url":"https://site.test/blog/one","content":content});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        session::call(&app, "config.testRobots", args)
    };
    let saved = call(json!({})).await;
    assert_eq!(saved["allowed"], false);
    let draft = call(json!({"userAgent":"Special"})).await;
    assert_eq!(draft["allowed"], true);
    assert_eq!(draft["matchedLine"], 4);
    assert_eq!(draft["userAgent"], "Special");
}
