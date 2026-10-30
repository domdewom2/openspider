use os_model::{CrawlConfig, FetchPurpose, Value};
use os_parse::{ParseConfig, ResponseInput};
use url::Url;

// spec: ONPAGE-043
#[test]
fn missing_mime_and_generic_mime_respect_sq13c_without_disabling_other_rules() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/n10-security/cases.json"
    ))
    .unwrap();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    for c in cases {
        let url = Url::parse(&format!("https://site.test{}", c["path"].as_str().unwrap())).unwrap();
        let headers = c["mime"]
            .as_str()
            .map(|m| vec![("Content-Type".into(), m.into())])
            .unwrap_or_default();
        let body = if c["body"] == "PNG" {
            &b"\x89PNG\r\n\x1a\n"[..]
        } else {
            c["body"].as_str().unwrap().as_bytes()
        };
        let facts = os_parse::security::response_facts(&ResponseInput {
            url: &url,
            internal: true,
            purpose: FetchPurpose::Page,
            status: 200,
            headers: &headers,
            body,
            content_length: None,
            untruncated_bytes: None,
            truncated: false,
            cfg: &cfg,
        });
        assert!(
            facts
                .iter()
                .any(|(k, v)| k.as_str() == "security.bad_content_type"
                    && *v == Value::Bool(c["bad"].as_bool().unwrap())),
            "{c}"
        );
    }
}
