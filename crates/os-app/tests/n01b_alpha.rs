//! M2-N01b: Alpha-Befunde A01-15 (XLSX-Blattname) und A01-04 (config.set ohne Crawl).
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
#[path = "session_common.rs"]
mod session;
use fixture::*;
use serde_json::json;
use std::{io::Read, time::Duration};

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

// spec: CRAWL-109
#[tokio::test]
async fn config_validate_reports_an_invalid_regex_once_per_path() {
    let dir = tempfile::tempdir().unwrap();
    let app = session::app(dir.path());
    let mut config = serde_json::to_value(os_model::CrawlConfig::default()).unwrap();
    config["scope"]["include"] = json!(["a", "["]);
    config["rewrite"]["regex_replace"] = json!([{"pattern":"[","replacement":"x"}]);
    let dto = json!({"schemaVersion": 2, "generator": "n01b", "config": config});
    let issues = session::call(&app, "config.validate", dto).await;
    let paths: Vec<_> = issues
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(paths, ["scope.include[1]", "rewrite.regex_replace[0].pattern"]);
    assert!(!issues.to_string().contains("Zeile 0"));
}

// spec: OUT-220 [M2]
#[tokio::test]
async fn profile_list_ranks_the_recently_loaded_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let app = session::app(dir.path());
    let config = serde_json::to_value(session::cfg()).unwrap();
    let dto = json!({"schemaVersion": 2, "generator": "n01b", "config": config});
    for name in ["a", "b", "c"] {
        session::call(
            &app,
            "profile.save",
            json!({"name": name, "config": dto, "overwrite": false}),
        )
        .await;
    }
    for name in ["a", "c", "a"] {
        session::call(&app, "profile.load", json!({"name": name})).await;
    }
    let list = session::call(&app, "profile.list", json!(null)).await;
    let rank = |name: &str| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap()["recentRank"]
            .clone()
    };
    assert_eq!(rank("a"), json!(0));
    assert_eq!(rank("c"), json!(1));
    assert_eq!(rank("b"), json!(null));
}

// spec: OUT-206
#[tokio::test]
async fn shutting_down_follows_the_injected_clock() {
    let dir = tempfile::tempdir().unwrap();
    let (app, clock) = session::manual_app(dir.path());
    assert!(!app.ops().shutting_down());
    app.ops().cancel_all();
    assert!(app.ops().shutting_down());
    clock.advance(Duration::from_secs(59));
    assert!(app.ops().shutting_down());
    clock.advance(Duration::from_secs(2));
    assert!(!app.ops().shutting_down(), "the 60 s window ends on the injected clock");
}
