//! Die Werkzeuge über den echten STDIO-Prozess (M1-P27): rmcp-Client gegen den Starter `openspider-mcp`
//! (derselbe Server wie `openspider mcp` aus `os-cli`) mit `--crawl-dir` auf einen Ordner mit einem
//! gespeicherten Crawl von `out-basic-site`.

#[path = "tools_support.rs"]
mod support;

use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::json;
use support::*;

const BIN: &str = env!("CARGO_BIN_EXE_openspider-mcp");

// spec: AGENT-010, AGENT-011, AGENT-013
#[tokio::test]
async fn the_five_reading_tools_work_over_stdio() {
    let world = World::new();
    let stored = world.add("out-basic-site", "/", base_config(), 1).await;
    let home = tempfile::tempdir().unwrap();
    let transport = TokioChildProcess::new(tokio::process::Command::new(BIN).configure(|cmd| {
        cmd.env("OPENSPIDER_HOME", home.path())
            .env_remove("OPENSPIDER_LOG")
            .arg("--crawl-dir")
            .arg(world.dir.path());
    }))
    .unwrap();
    let client = Client::from_running(().serve(transport).await.expect("initialize"));

    assert_eq!(
        client.tool_names().await,
        [
            "crawl_overview",
            "list_crawls",
            "list_issues",
            "query_urls",
            "url_details"
        ],
        "im Auslieferungszustand genau die fünf lesenden Werkzeuge"
    );
    let crawls = client.call("list_crawls", json!({})).await;
    assert_eq!(
        crawls.summary()["crawls"][0]["crawl_id"],
        stored.id.as_str()
    );

    let urls = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Response Codes", "filter": "All", "columns": ["Address", "Status Code"]}),
        )
        .await;
    assert!(urls.total().unwrap() > 3);
    assert!(urls.untrusted.is_some(), "Zeilen im Datenblock");

    let overview = client
        .call("crawl_overview", json!({"crawl_id": stored.id}))
        .await;
    assert!(overview.summary()["sections"].as_array().unwrap().len() > 5);
    let issues = client
        .call("list_issues", json!({"crawl_id": stored.id}))
        .await;
    assert!(issues.summary()["issues"].is_array());
    let details = client
        .call(
            "url_details",
            json!({"crawl_id": stored.id, "url": stored.fx.site.url("/"), "sections": ["overview"]}),
        )
        .await;
    assert_eq!(details.summary()["crawled"], true);

    // Ein unbekannter Name ist ein Werkzeugfehler mit der Liste, kein Abbruch des Servers.
    let bad = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal", "filter": "Foo"}),
        )
        .await;
    assert!(bad.is_error && bad.error_text().contains("Gültige Filter"));
    client.cancel().await;
}
