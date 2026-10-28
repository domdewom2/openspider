use os_mcp::audit::redacted;
use serde_json::json;

// spec: AGENT-043
#[test]
fn audit_exposes_only_allowlisted_scalars_and_never_nested_secrets() {
    let fixture: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/agent-m2-http/requests.json"
    ))
    .unwrap();
    for headers in fixture {
        assert_eq!(
            redacted(&json!({"headers":headers})),
            json!({"headers":"[redacted]"})
        );
    }
    let safe = json!({"crawl_id":"fixture-crawl", "id":12, "tab":"internal",
        "filter":"all", "limit":50, "offset":0, "format":"csv", "page":2});
    assert_eq!(redacted(&safe), safe);
    let input = json!({"crawl_id":{"token":"secret"}, "id":["secret"],
        "tab":true, "page":null, "url":"secret", "urls":["secret"],
        "start_url":"secret", "sql":"secret", "path":"secret",
        "path_output":"secret", "overrides":{"token":"secret"},
        "headers":{"Authorization":"secret"}, "cookies":"secret",
        "cursor":"secret", "unknown":"secret", "confirm":true});
    let output = redacted(&input);
    assert!(!output.to_string().contains("secret"));
    assert_eq!(output["crawl_id"], "[redacted]");
    assert_eq!(output["id"], json!({"items":1}));
    assert_eq!(output["tab"], true);
    assert_eq!(output["page"], serde_json::Value::Null);
    assert_eq!(output["confirm"], true);
    for key in [
        "url",
        "start_url",
        "sql",
        "path",
        "overrides",
        "headers",
        "cookies",
        "cursor",
    ] {
        assert_eq!(output[key], "[redacted]");
    }
    assert!(output.get("path_output").is_none());
    assert!(output.get("unknown").is_none());
}
