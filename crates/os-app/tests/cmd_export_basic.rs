#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use os_app::dto::{ErrorCode, OpHandle, OpState};
use serde_json::json;
// spec: OUT-020 [M2]
#[tokio::test]
async fn view_export_is_a_background_snapshot_with_visible_columns_search_and_order() {
    let fx = Fx::new();
    let mut spec = tab();
    spec["sort"] = json!([{"column":"Word Count","dir":"desc"}]);
    spec["search"] = json!({"kind":"simple","text":"blog"});
    let h = fx.open(spec, json!(["Address", "Word Count"])).await;
    let path = fx.dir.path().join("result.csv");
    let result=fx.op("export.view",json!({"viewId":h["viewId"],"version":42,"path":path,"format":"csv","allColumns":false})).await.unwrap();
    assert!(result.bytes > 0);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Address,Word Count"));
    assert!(text.contains("/blog/a,200"));
    assert!(!text.contains("/missing"));
    let suggestion = fx
        .call(
            "export.suggestName",
            json!({"target":{"kind":"view","tab":"Internal","filter":"All","format":"csv"}}),
        )
        .await;
    assert!(suggestion
        .as_str()
        .unwrap()
        .starts_with("internal_all_2026-10-13"));
}
// spec: OUT-020 [M2]
#[tokio::test]
async fn stale_export_never_replaces_existing_file_and_all_columns_includes_hidden_fields() {
    let fx = Fx::new();
    let h = fx.open(tab(), json!(["Address"])).await;
    let path = fx.dir.path().join("result.csv");
    std::fs::write(&path, "keep").unwrap();
    fx.write("UPDATE crawl_meta SET value='43' WHERE key='change_seq'");
    let err = fx
        .op(
            "export.view",
            json!({"viewId":h["viewId"],"version":42,"path":path,"format":"csv","allColumns":true}),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Stale);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    let h = fx
        .call("view.update", json!({"viewId":h["viewId"],"spec":tab()}))
        .await;
    fx.op(
        "export.view",
        json!({"viewId":h["viewId"],"version":43,"path":path,"format":"csv","allColumns":true}),
    )
    .await
    .unwrap();
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("Title 1 Pixel Width"));
}
// spec: OUT-021 [P1]
// spec: UI-042
#[tokio::test]
async fn header_detail_export_streams_all_pages_with_canonical_names_and_redaction() {
    let fx = Fx::new();
    fx.write("WITH RECURSIVE n(x) AS (SELECT 3 UNION ALL SELECT x+1 FROM n WHERE x<610) INSERT INTO http_headers SELECT 1,'response',x,'X-Page',CAST(x AS TEXT) FROM n");
    let path = fx.dir.path().join("headers.csv");
    fx.op("export.detail",json!({"spec":{"kind":"detail","detail":"headers","urlIds":[3],"params":{},"sort":[]},"selection":{"kind":"ids","ids":[1]},"path":path,"format":"csv"})).await.unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Address,Direction,Header Name,Header Value"));
    assert!(text.contains("Response,X-Page,610"));
    assert_eq!(text.matches("Response,Set-Cookie,").count(), 2);
    assert!(!text.contains("should-never-escape"));
    assert_eq!(text.lines().count(), 613);
}
// spec: OUT-021 [M2]
// spec: INTEG-084
// spec: ONPAGE-036
#[tokio::test]
async fn link_exports_honor_selection_position_filter_and_standard_columns() {
    let fx = Fx::new();
    let path = fx.dir.path().join("inlinks.csv");
    let mut spec = links(true);
    spec["filter"] = json!({"id":"Hyperlinks","position":"Content","excludeSelf":true});
    fx.op(
        "export.detail",
        json!({"spec":spec,"selection":{"kind":"ids","ids":[2,2]},"path":path,"format":"csv"}),
    )
    .await
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(text.contains("Link Path,Link Position,Link Origin"));
    assert!(text.contains("_blank"));
    assert!(!text.contains(",Nav,"));
}
// spec: OUT-099
// spec: OUT-023
#[tokio::test]
async fn export_catalog_uses_configured_entries_and_prerequisites_and_fach_dispatch() {
    let fx = Fx::new();
    let catalog = fx.call("export.catalog", json!(null)).await;
    let reports = catalog["reports"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["items"].as_array().unwrap())
        .collect::<Vec<_>>();
    let orphan = reports
        .iter()
        .find(|r| r["name"] == "Orphan Pages")
        .unwrap();
    assert_eq!(orphan["enabled"], false);
    assert!(orphan["disabledReason"].is_string());
    let name = reports
        .iter()
        .find(|r| r["name"] == "Redirects:All Redirects")
        .unwrap()["name"]
        .as_str()
        .unwrap();
    let result=fx.op("export.report",json!({"name":name,"dir":fx.dir.path().join("reports"),"format":"csv","singleWorkbook":false})).await.unwrap();
    assert!(std::fs::read_to_string(result.path)
        .unwrap()
        .contains("https://site.test/old"));
    let e = fx.op("export.detail",json!({"spec":{"kind":"detail","detail":"crawlPath","urlIds":[2],"params":{},"sort":[]},"selection":{"kind":"ids","ids":[2]},"path":fx.dir.path().join("path.csv"),"format":"csv"})).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert_eq!(e.args["owner"], "M2-D17");
}
// spec: OUT-020
// spec: OUT-021 [P1]
#[tokio::test]
async fn bulk_issues_overview_and_sitemap_commands_produce_files() {
    let fx = Fx::new();
    let bulk=fx.op("export.bulk",json!({"name":"Links:All Inlinks","dir":fx.dir.path().join("bulk"),"format":"csv","singleWorkbook":false})).await.unwrap();
    let text = std::fs::read_to_string(bulk.path).unwrap();
    assert!(text.contains("Link Position"));
    assert!(text.contains("https://site.test/blog/a"));
    for command in ["export.issues", "export.sidebar"] {
        let result=fx.op(command,json!({"which":if command=="export.issues"{"issues"}else{"overview"},"path":fx.dir.path().join(format!("{command}.csv")),"format":"csv"})).await.unwrap();
        assert!(result.bytes > 50);
    }
    let sitemap = fx
        .op(
            "export.sitemap",
            json!({"sitemapKind":"pages","options":null,"dir":fx.dir.path().join("sitemap")}),
        )
        .await
        .unwrap();
    assert!(sitemap.bytes > 0);
    assert!(
        std::fs::read_to_string(std::path::Path::new(&sitemap.path).join("sitemap.xml"))
            .unwrap()
            .contains("https://site.test/blog/a")
    );
}
// spec: OUT-010
#[tokio::test]
async fn cancelled_exports_leave_existing_files_and_no_staging_directories() {
    let fx = Fx::new();
    fx.write("WITH RECURSIVE n(x) AS (SELECT 10 UNION ALL SELECT x+1 FROM n WHERE x<80000) INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code) SELECT x,'https://site.test/'||x,'https://site.test/'||x,'https://site.test',1,1,'done','link',0,25,'HTML',200 FROM n");
    let h = fx.open(tab(), json!(["Address"])).await;
    let path = fx.dir.path().join("cancel.csv");
    std::fs::write(&path, "keep").unwrap();
    let op:OpHandle=serde_json::from_value(fx.call("export.view",json!({"viewId":h["viewId"],"version":42,"path":path,"format":"csv","allColumns":false})).await).unwrap();
    assert!(fx.app.ops().cancel(&op.op_id));
    let start = std::time::Instant::now();
    loop {
        let status = fx.app.ops().status(&op.op_id).unwrap();
        if status.state != OpState::Running {
            assert_eq!(status.state, OpState::Cancelled);
            break;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep");
    assert!(!std::fs::read_dir(fx.dir.path())
        .unwrap()
        .flatten()
        .any(|p| p
            .file_name()
            .to_string_lossy()
            .starts_with(".openspider-export-")));
}
// spec: OUT-006
#[tokio::test]
async fn unsupported_formats_fail_before_creating_files() {
    let fx = Fx::new();
    let e=fx.app.dispatch(os_app::CallCtx::for_window("main"),"export.bulk",json!({"name":"Links:All Inlinks","dir":fx.dir.path().join("json"),"format":"json","singleWorkbook":false})).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::Unsupported);
    assert!(!fx.dir.path().join("json").exists());
}

// spec: OUT-021 [P1]
#[tokio::test]
async fn url_details_export_contains_every_main_table_property() {
    let fx = Fx::new();
    let columns = fx.call("catalog.columns", json!({"tab":"Internal"})).await;
    let path = fx.dir.path().join("properties.csv");
    fx.op(
        "export.detail",
        json!({"spec":tab(),"selection":{"kind":"ids","ids":[1]},"path":path,"format":"csv"}),
    )
    .await
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text
        .trim_start_matches('\u{feff}')
        .starts_with("Name,Value"));
    assert_eq!(text.lines().count(), columns.as_array().unwrap().len() + 1);
    assert!(text.contains("Title 1,Same title"));
}

// spec: OUT-010
#[tokio::test]
async fn snapshot_keeps_sequence_and_data_together_and_releases_failed_reads() {
    let fx = Fx::new();
    let pool = fx.app.read_pool().clone();
    let key = os_app::pool::CrawlKey("fx-ui-basic".into());
    let (started, wait) = tokio::sync::oneshot::channel();
    let (resume, continue_read) = std::sync::mpsc::channel();
    let run_key = key.clone();
    let reader = tokio::spawn(async move {
        pool.run_snapshot(run_key, move |file, _| {
            let before = os_store::queries::change_seq(file.conn())?;
            started.send(()).unwrap();
            continue_read.recv().unwrap();
            let after = os_store::queries::change_seq(file.conn())?;
            Ok((before, after))
        })
        .await
    });
    wait.await.unwrap();
    fx.write("UPDATE crawl_meta SET value='43' WHERE key='change_seq'");
    resume.send(()).unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), (42, 42));
    let error = fx
        .app
        .read_pool()
        .run_snapshot::<()>(key.clone(), |_, _| {
            Err(os_store::StoreError::InvalidInput("fixture failure".into()))
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert_eq!(
        fx.app
            .read_pool()
            .run_snapshot(key, |file, _| os_store::queries::change_seq(file.conn()))
            .await
            .unwrap(),
        43
    );
}

// spec: OUT-015 [M2]
#[tokio::test]
async fn list_export_preserves_upload_duplicates_original_input_and_invalid_rows() {
    let fx = Fx::new();
    fx.write("UPDATE crawl_meta SET value='list' WHERE key='mode'; INSERT INTO list_input(position,original,url_id,outcome) VALUES(0,'B',2,'ok'),(1,'A',1,'ok'),(2,'B',2,'duplicate'),(3,'not a URL',NULL,'invalid')");
    let h = fx.open(tab(), json!(["Address", "Status"])).await;
    let path = fx.dir.path().join("list.csv");
    fx.op(
        "export.view",
        json!({"viewId":h["viewId"],"version":42,"path":path,"format":"csv","allColumns":false}),
    )
    .await
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 5);
    assert!(lines[0].contains("Original Url,Address,Status"));
    assert!(lines[1].starts_with("B,https://site.test/blog/a"));
    assert!(lines[2].starts_with("A,https://site.test/"));
    assert_eq!(lines[1], lines[3]);
    assert_eq!(lines[4], "not a URL,,Invalid");
    fx.op("export.view",json!({"viewId":h["viewId"],"version":42,"path":path,"format":"csv","allColumns":false,"preserveUploadOrder":false})).await.unwrap();
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("Original Url"));
}

// spec: OUT-021 [P1]
#[tokio::test]
async fn detail_names_are_canonical_and_url_details_use_the_tab_descriptor() {
    let fx = Fx::new();
    for (spec, expected) in [
        (
            json!({"kind":"detail","detail":"headers","urlIds":[1],"params":{},"sort":[]}),
            "http_headers",
        ),
        (
            json!({"kind":"detail","detail":"structuredDataItems","urlIds":[1],"params":{},"sort":[]}),
            "structured_data_details",
        ),
        (
            json!({"kind":"detail","detail":"structuredDataIssues","urlIds":[1],"params":{},"sort":[]}),
            "structured_data_details",
        ),
        (tab(), "url_details"),
    ] {
        let name = fx
            .call(
                "export.suggestName",
                json!({"target":{"kind":"detail","spec":spec,"format":"csv"}}),
            )
            .await;
        assert_eq!(name, format!("detail_{expected}_2026-10-13.csv"));
    }
}

// spec: OUT-021 [P1]
#[tokio::test]
async fn header_workbook_uses_the_same_canonical_detail_title() {
    use std::io::Read;
    let fx = Fx::new();
    let result = fx.op("export.detail", json!({"spec":{"kind":"detail","detail":"headers","urlIds":[1],"params":{},"sort":[]},"selection":{"kind":"ids","ids":[1]},"path":fx.dir.path().join("headers.xlsx"),"format":"xlsx"})).await.unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(result.path).unwrap()).unwrap();
    let mut workbook = String::new();
    archive
        .by_name("xl/workbook.xml")
        .unwrap()
        .read_to_string(&mut workbook)
        .unwrap();
    assert!(workbook.contains("name=\"HTTP Headers\""));
}

// spec: UI-061
#[tokio::test]
async fn overview_export_matches_every_summary_percentage() {
    let fx = Fx::new();
    let overview = fx.call("sidebar.overview", json!({})).await;
    let result = fx
        .op(
            "export.sidebar",
            json!({"which":"overview","path":fx.dir.path().join("overview.csv"),"format":"csv"}),
        )
        .await
        .unwrap();
    let text = std::fs::read_to_string(result.path).unwrap();
    for (record, summary) in text
        .lines()
        .skip(1)
        .zip(overview["summary"].as_array().unwrap())
    {
        let record = record.split(',').collect::<Vec<_>>();
        assert_eq!(record[0], summary["key"].as_str().unwrap());
        assert_eq!(
            record[1].parse::<u64>().unwrap(),
            summary["count"].as_u64().unwrap()
        );
        assert_eq!(
            record[2].parse::<f64>().unwrap(),
            summary["pct"].as_f64().unwrap()
        );
    }
}

// Fixture-only SQL fault injection through the read pool.
// spec: OUT-010
#[tokio::test]
#[allow(clippy::disallowed_methods)]
async fn failed_rollback_discards_the_connection_even_when_the_callback_also_failed() {
    let fx = Fx::new();
    let key = os_app::pool::CrawlKey("fx-ui-basic".into());
    fx.app
        .read_pool()
        .run_snapshot::<()>(key.clone(), |file, _| {
            // End the transaction early so the pool's ROLLBACK deterministically fails.
            file.conn()
                .execute_batch("CREATE TEMP TABLE poisoned_snapshot(x); COMMIT")?;
            Err(os_store::StoreError::InvalidInput("callback failed".into()))
        })
        .await
        .unwrap_err();
    let leftover = fx
        .app
        .read_pool()
        .run_snapshot(key, |file, _| {
            Ok(file.conn().query_row(
                "SELECT count(*) FROM sqlite_temp_master WHERE name='poisoned_snapshot'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(
        leftover, 0,
        "a rollback failure must discard the connection"
    );
}
