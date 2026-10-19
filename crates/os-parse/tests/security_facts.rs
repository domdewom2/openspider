use os_model::{CrawlConfig, Value};
use os_parse::{parse_html, response_ext, ParseConfig, ParseInput, ResponseInput};
use std::path::PathBuf;
use url::Url;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/onpage-m2-security")
            .join(name),
    )
    .unwrap()
}
fn page(scheme: &str, name: &str) -> os_model::ParsedPage {
    let url = Url::parse(&format!("{scheme}://site.test/https/{name}")).unwrap();
    let body = fixture(&format!("https/{name}"));
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: &body,
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    })
}
fn flag(page: &os_model::ParsedPage, key: &str) -> bool {
    page.ext
        .iter()
        .find(|(k, _)| k.as_str() == format!("security.{key}"))
        .unwrap()
        .1
        == Value::Bool(true)
}

// spec: ONPAGE-033, ONPAGE-037
#[test]
fn resource_element_set_and_source_scheme_are_exact() {
    for name in ["mixed.html", "resources.html"] {
        assert!(flag(&page("https", name), "mixed_content"), "{name}");
        assert!(!flag(&page("http", name), "mixed_content"));
    }
    for name in ["relative.html", "link-only.html", "not-resources.html"] {
        assert!(!flag(&page("https", name), "mixed_content"), "{name}");
    }
    for name in ["relative.html", "resources.html"] {
        for scheme in ["http", "https"] {
            assert!(flag(&page(scheme, name), "protocol_relative"));
        }
    }
    assert!(!flag(
        &page("http", "not-resources.html"),
        "protocol_relative"
    ));
}

// spec: ONPAGE-034, ONPAGE-035
#[test]
fn forms_resolve_against_base_except_empty_actions_and_ignore_non_http_targets() {
    let p = page("https", "form-insecure.html");
    assert_eq!(p.forms.len(), 6);
    assert_eq!(p.forms[0].method, "post");
    assert_eq!(
        p.forms[1].action_resolved.as_deref(),
        Some("https://site.test/send")
    );
    assert_eq!(
        p.forms[2].action_resolved.as_deref(),
        Some("https://site.test/https/form-insecure.html")
    );
    assert_eq!(p.forms[2].action_resolved, p.forms[3].action_resolved);
    assert!(p.forms[4].action_resolved.is_none() && p.forms[5].action_resolved.is_none());
    assert!(flag(&p, "form_insecure"));
    assert!(!flag(&p, "form_on_http"));
    for name in ["form-relative.html", "form-secure.html"] {
        assert!(flag(&page("http", name), "form_on_http"));
    }
    assert!(!flag(&page("https", "form-relative.html"), "form_insecure"));
    assert!(!flag(&page("http", "form-secure.html"), "form_insecure"));
    assert!(!flag(&page("http", "safe.html"), "form_on_http"));
    let p = page("https", "form-base.html");
    assert_eq!(
        p.forms[0].action_resolved.as_deref(),
        Some("http://site.test/base/send")
    );
    assert_eq!(
        p.forms[1].action_resolved.as_deref(),
        Some("https://site.test/https/form-base.html")
    );
}

// spec: ONPAGE-038, ONPAGE-039, ONPAGE-040, ONPAGE-041, ONPAGE-042, ONPAGE-043
#[test]
fn response_fixture_covers_headers_mime_extensions_magic_and_non_html_redirects() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(&fixture("responses/cases.json")).unwrap();
    for c in cases {
        let url = Url::parse(c["url"].as_str().unwrap_or("https://site.test/resource")).unwrap();
        let mut headers: Vec<(String, String)> = c["headers"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
            .collect();
        headers.push((
            "Content-Type".into(),
            c["mime"].as_str().unwrap_or("text/plain").into(),
        ));
        let body = if c["body"] == "PNG" {
            b"\x89PNG\r\n\x1a\n".to_vec()
        } else {
            c["body"].as_str().unwrap_or("plain").as_bytes().to_vec()
        };
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        let input = ResponseInput {
            url: &url,
            internal: true,
            purpose: os_model::FetchPurpose::Page,
            status: c["status"].as_u64().unwrap_or(200) as u16,
            headers: &headers,
            body: &body,
            content_length: None,
            untruncated_bytes: None,
            truncated: false,
            cfg: &cfg,
        };
        let facts = response_ext(&input);
        for (key, want) in c["expect"].as_object().unwrap() {
            let got = &facts
                .iter()
                .find(|(k, _)| k.as_str() == format!("security.{key}"))
                .unwrap()
                .1;
            assert_eq!(got, &Value::Bool(want.as_bool().unwrap()), "{c}");
        }
        let mut input = input;
        input.internal = false;
        assert!(os_parse::security::response_facts(&input).is_empty());
        input.internal = true;
        input.status = 0;
        assert!(os_parse::security::response_facts(&input).is_empty());
    }
}

// spec: ONPAGE-043
#[test]
fn obsolete_mime_list_can_be_disabled_and_expected_type_is_recorded() {
    let url = Url::parse("https://site.test/script.js").unwrap();
    let mut crawl = CrawlConfig::default();
    crawl.security.obsolete_mime_types.clear();
    let cfg = ParseConfig::from_crawl_config(&crawl);
    let headers = vec![("Content-Type".into(), "text/javascript".into())];
    let input = ResponseInput {
        url: &url,
        internal: true,
        purpose: os_model::FetchPurpose::Page,
        status: 200,
        headers: &headers,
        body: b"let x=1;",
        content_length: None,
        untruncated_bytes: None,
        truncated: false,
        cfg: &cfg,
    };
    let facts = response_ext(&input);
    assert!(facts
        .iter()
        .any(|(k, v)| k.as_str() == "security.bad_content_type" && *v == Value::Bool(false)));
    assert!(facts
        .iter()
        .any(|(k, v)| k.as_str() == "security.expected_content_type"
            && *v == Value::from("application/javascript")));
}
