//! M2-N01b: Alpha-Befunde A01-15 (XLSX-Blattname) und A01-04 (config.set ohne Crawl).
#[path = "cmd_catalog_fixture.rs"]
mod fixture;
#[path = "session_common.rs"]
mod session;
use fixture::*;
use serde_json::{json, Value};
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
    assert_eq!(
        paths,
        ["scope.include[1]", "rewrite.regex_replace[0].pattern"]
    );
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
    assert!(
        !app.ops().shutting_down(),
        "the 60 s window ends on the injected clock"
    );
}

// spec: UI-036
#[tokio::test]
async fn resources_list_has_content_type_blocked_by_and_type_filters() {
    let fx = Fx::new();
    {
        let file = os_store::CrawlFile::open(&fx.path, os_store::OpenMode::ReadWrite).unwrap();
        file.conn()
            .execute_batch(
                "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_type,content_class,status_code) VALUES
                  (101,'https://site.test/s.css','https://site.test/s.css','https://site.test',1,1,'done','link',0,23,'text/css','CSS',200),
                  (102,'https://site.test/f.woff2','https://site.test/f.woff2','https://site.test',1,1,'done','link',0,25,'font/woff2','Other',200);
                 INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,robots_blocked) VALUES
                  (103,'https://site.test/blocked.js','https://site.test/blocked.js','https://site.test',1,1,'done','link',0,28,'JavaScript',1);
                 INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,no_response_reason) VALUES
                  (104,'https://site.test/slow.js','https://site.test/slow.js','https://site.test',1,1,'done','link',0,25,'JavaScript','connection_timeout');
                 INSERT INTO links(id,source_id,target_id,target_raw,link_type,position,follow,path_type,link_path,link_position,link_origin,crawlable) VALUES
                  (9001,1,101,'/s.css','CSS',1,1,'Root-Relative','//link','Head','HTML',1),
                  (9002,1,102,'/f.woff2','Other',2,1,'Root-Relative','//link','Head','HTML',1),
                  (9003,1,103,'/blocked.js','JavaScript',3,1,'Root-Relative','//script','Head','HTML',1),
                  (9004,1,104,'/slow.js','JavaScript',4,1,'Root-Relative','//script','Head','HTML',1)",
            )
            .unwrap();
    }
    async fn rows(fx: &Fx, filter: &str) -> Value {
        let columns = json!(["To", "Content Type", "Size", "Blocked By", "Status Code"]);
        let h = fx
            .open(
                json!({"kind":"resources","urlIds":[1],"filter":{"id":filter},"search":null,"sort":[]}),
                columns.clone(),
            )
            .await;
        fx.rows(&h, 0, 50, columns).await
    }
    let all = rows(&fx, "All").await;
    let cells = all["cells"].as_array().unwrap();
    let find = |name: &str| {
        cells
            .iter()
            .find(|r| r[0].as_str().is_some_and(|t| t.ends_with(name)))
            .unwrap()
            .clone()
    };
    assert_eq!(find("s.css")[1], "text/css");
    assert_eq!(find("blocked.js")[3], "robots.txt");
    assert!(find("blocked.js")[4].is_null());
    assert_eq!(find("slow.js")[3], "Timeout");
    assert!(find("s.css")[3].is_null());
    for (filter, expect) in [
        ("CSS", vec!["s.css"]),
        ("Fonts", vec!["f.woff2"]),
        ("JavaScript", vec!["blocked.js", "slow.js"]),
        ("Blocked", vec!["blocked.js", "slow.js"]),
    ] {
        let page = rows(&fx, filter).await;
        let mut got: Vec<String> = page["cells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r[0].as_str()
                    .unwrap()
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        got.sort();
        assert_eq!(got, expect, "filter {filter}");
    }
    let unknown = fx.app.dispatch(os_app::CallCtx::for_window("main"), "view.open", json!({"spec":{"kind":"resources","urlIds":[1],"filter":{"id":"Nope"},"search":null,"sort":[]},"columns":[]})).await;
    assert!(unknown.is_err());
}
