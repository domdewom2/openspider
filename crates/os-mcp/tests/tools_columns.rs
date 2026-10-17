//! Spaltennamen von `query_urls` (M1-N17): nummerierte Mehrfachspalten und Schlüsselnamen prüft das Werkzeug
//! mit derselben Regel wie die Ansicht in `os-analyze` (`views::column_name_resolves`).
//!
//! Die Vorabprüfung läuft vor jeder Abfrage; die Tests laufen gegen einen gespeicherten Crawl von
//! `tech/hreflang`, damit auch die Ansicht den Namen wirklich auflösen muss.

#[path = "tools_support.rs"]
mod support;

use serde_json::json;
use support::*;

const ACCEPTED: [(&str, &str); 10] = [
    ("Internal", "Title 1"),
    ("Internal", "title 1"),
    ("Internal", "H1-2"),
    ("Internal", "h1-2"),
    ("Internal", "mEtA rObOtS 2"),
    ("Hreflang", "HTML hreflang 1"),
    ("Hreflang", "HTML hreflang 1 URL"),
    ("Hreflang", "html HREFLANG 2 url"),
    ("Hreflang", "Html Hreflang 1 Url"),
    ("Hreflang", "HTML hreflang 12 URL"),
];

const REJECTED: [(&str, &str); 6] = [
    ("Internal", "Title 0"),
    ("Internal", "Title x"),
    ("Hreflang", "HTML hreflang 1 URLs"),
    ("Hreflang", "HTML hreflang 0 URL"),
    ("Hreflang", "HTML hreflang 1 Foo"),
    ("Hreflang", "HTML hreflang URL"),
];

// spec: AGENT-011
#[tokio::test]
async fn numbered_and_mixed_case_column_names_are_accepted() {
    let world = World::new();
    let stored = world.add("tech/hreflang", "/en/", base_config(), 1).await;
    let client = world.client().await;
    for (tab, name) in ACCEPTED {
        let reply = client
            .call(
                "query_urls",
                json!({"crawl_id": stored.id, "tab": tab,
                       "columns": ["Address", name], "limit": 5}),
            )
            .await;
        assert!(!reply.is_error, "{tab} / {name}: {:?}", reply.blocks);
        // Die Ausgabe nennt die Spalte in kanonischer Schreibweise.
        let columns = reply.summary()["columns"].as_array().unwrap().clone();
        assert_eq!(columns.len(), 2, "{tab} / {name}");
        assert!(
            columns[1].as_str().unwrap().eq_ignore_ascii_case(name),
            "{tab} / {name}: {columns:?}"
        );
    }
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn invalid_numbered_names_are_rejected_with_the_list_of_valid_forms() {
    let world = World::new();
    let stored = world.add("tech/hreflang", "/en/", base_config(), 1).await;
    let client = world.client().await;
    for (tab, name) in REJECTED {
        let reply = client
            .call(
                "query_urls",
                json!({"crawl_id": stored.id, "tab": tab, "columns": [name]}),
            )
            .await;
        assert!(reply.is_error, "{tab} / {name}");
        let text = reply.error_text();
        assert!(
            text.contains(&format!("unbekannte Spalte \"{name}\" im Tab {tab}")),
            "{text}"
        );
        assert!(text.contains("Gültige Spalten:"), "{text}");
        let forms = if tab == "Hreflang" {
            "HTML hreflang 1…n URL"
        } else {
            "Meta Robots 1…n"
        };
        assert!(text.contains(forms), "{text}");
    }
    // Beim Sortieren gilt dieselbe Regel.
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Hreflang",
                   "sort": ["HTML hreflang 1 URLs"], "limit": 5}),
        )
        .await;
    assert!(reply.is_error);
    client.cancel().await;
}

// spec: INTEG-020, AGENT-011
// Infrastruktur (M2-F05a) für INTEG-020; Fachprüfung M2-D14.
#[tokio::test]
async fn mcp_queries_filters_from_a_configured_column_source() {
    use os_mcp::tool::ToolContext;
    let world = World::new();
    let stored = world.add("out-basic-site", "/", base_config(), 7).await;
    let ctx = ToolContext::new(world.dir.path().to_path_buf());
    let tools = os_mcp::tools::all();
    let tool = tools.iter().find(|t| t.name() == "query_urls").unwrap();
    os_analyze::tabs::testing::with_fixture_source(|| {
        let result = tool
            .call(
                &ctx,
                json!({"crawl_id": stored.id, "tab": "Internal", "filter": "Fixture Filter"}),
            )
            .unwrap();
        assert_eq!(result.total, Some(1));
        assert_eq!(result.summary["filter"], "Fixture Filter");
    });
}
