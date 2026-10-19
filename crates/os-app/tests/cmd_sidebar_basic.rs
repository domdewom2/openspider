#[path = "cmd_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::json;
// spec: UI-062
// spec: UI-065 [M2]
#[tokio::test]
async fn issues_match_filter_counts_and_explanations_use_shared_translated_resources() {
    let fx = Fx::new();
    let rows = fx.call("sidebar.issues", json!({})).await;
    assert!(!rows.as_array().unwrap().is_empty());
    golden(
        "IssueRows.json",
        &json!(rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["tab"] == "Page Titles" && r["filter"] == "Duplicate")
            .collect::<Vec<_>>()),
    );
    let counts = fx.call("catalog.filterCounts", json!({})).await;
    for issue in rows.as_array().unwrap() {
        assert!(issue["count"].as_u64().unwrap() > 0);
        let count = counts["filters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tab"] == issue["tab"] && r["filter"]["labelTemplate"] == issue["filter"])
            .unwrap();
        assert_eq!(issue["count"], count["filter"]["count"]);
    }
    for lang in ["de", "en", "fr"] {
        let text = fx
            .call(
                "sidebar.issueExplain",
                json!({"issueId":rows[0]["id"],"lang":lang}),
            )
            .await;
        assert!(!text["what"].as_str().unwrap().is_empty());
        assert!(!text["fix"].as_str().unwrap().is_empty());
        assert!(text["rule"].as_str().unwrap().contains('/'));
    }
    let filtered = fx
        .call(
            "sidebar.issues",
            json!({"type":"issue","priority":"high","search":"Response Codes"}),
        )
        .await;
    assert!(filtered
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["type"] == "issue" && r["priority"] == "high" && r["tab"] == "Response Codes"));
}
// spec: UI-065 [M2]
#[tokio::test]
async fn explanation_thresholds_come_from_current_configuration() {
    let mut cfg = os_model::CrawlConfig::default();
    cfg.title.max_chars = 65;
    let fx = Fx::config(cfg);
    let text = fx
        .call(
            "sidebar.issueExplain",
            json!({"issueId":"Page Titles/Over {title.max_chars} Characters","lang":"en"}),
        )
        .await;
    assert!(text.to_string().contains("65"));
    assert!(!text.to_string().contains("{max}"));
}
