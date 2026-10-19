use os_model::{CrawlConfig, ValidationFacts, Value};
use os_parse::{parse_html, validation, ParseConfig, ParseInput, ResponseInput};

const EXPECTED: &str = include_str!("../../../tests/fixtures/tech-m2-validation/expectations.json");

fn source(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/tech-m2-validation")
            .join(file),
    )
    .unwrap()
}

fn parse(file: &str, enabled: bool) -> os_model::ParsedPage {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let cfg = ParseConfig {
        html_validation: enabled,
        ..cfg
    };
    let url = url::Url::parse("https://site.test/page").unwrap();
    let body = source(file);
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: &body,
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    })
}

fn facts(file: &str) -> ValidationFacts {
    parse(file, true).validation.unwrap()
}

fn has(file: &str, code: &str) -> bool {
    facts(file).issues.iter().any(|i| i.code == code)
}

// spec: TECH-280
#[test]
fn source_token_facts_match_the_fixture_golden() {
    let cases: serde_json::Value = serde_json::from_str(EXPECTED).unwrap();
    for case in cases.as_array().unwrap() {
        let name = case["file"].as_str().unwrap();
        let f = facts(name);
        assert_eq!(
            u64::from(f.tokens.html_count),
            case["html"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            u64::from(f.tokens.head_count),
            case["head"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            u64::from(f.tokens.body_count),
            case["body"].as_u64().unwrap(),
            "{name}"
        );
        let mut codes: Vec<_> = f.issues.iter().map(|i| i.code.as_str()).collect();
        codes.sort_unstable();
        codes.dedup();
        let want: Vec<_> = case["codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(codes, want, "{name}");
    }
    let f = validation::analyze("<html><head></head><body></body></html>");
    assert_eq!(
        (
            f.tokens.first_html,
            f.tokens.first_head,
            f.tokens.first_body
        ),
        (Some(0), Some(1), Some(3))
    );
    assert_eq!(f.tokens.first_element_after_html.as_deref(), Some("head"));
}

// spec: TECH-008
// spec: CRAWL-138
#[test]
fn disabled_validation_collects_no_structure_and_keeps_extracted_metadata() {
    for file in [
        "ok.html",
        "invalid-head-img.html",
        "invalid-head-div.html",
        "invalid-head-text.html",
    ] {
        let on = parse(file, true);
        let off = parse(file, false);
        assert!(off.validation.is_none());
        assert_eq!(on.canonicals, off.canonicals);
        assert_eq!(on.directives, off.directives);
        assert_eq!(on.hreflang, off.hreflang);
        assert_eq!(on.effective_index, off.effective_index);
        assert_eq!(on.effective_follow, off.effective_follow);
        assert_eq!(on.titles, off.titles);
        let ext = validation::to_ext(on.validation.as_ref().unwrap());
        assert!(ext
            .iter()
            .any(|(k, v)| k.as_str() == "validation.checked" && *v == Value::Bool(true)));
    }
}

// spec: TECH-003 [M2]
#[test]
fn invalid_head_tokens_explain_tree_builder_head_membership() {
    for file in [
        "invalid-head-img.html",
        "invalid-head-div.html",
        "invalid-head-text.html",
    ] {
        let page = parse(file, true);
        assert!(page
            .validation
            .unwrap()
            .issues
            .iter()
            .any(|i| i.code == "invalid-head-element"));
        assert!(page.canonicals.iter().all(|c| !c.in_head));
    }
    assert!(parse("ok.html", true).canonicals[0].in_head);
}

// spec: TECH-282
#[test]
fn invalid_head_elements_and_noscript_details_are_recorded() {
    for (file, element) in [
        ("invalid-head-img.html", "img"),
        ("invalid-head-div.html", "div"),
        ("invalid-head-text.html", "#text"),
        ("noscript-img.html", "img"),
        ("custom-head.html", "my-widget"),
    ] {
        let f = facts(file);
        assert!(
            f.issues
                .iter()
                .any(|i| i.code == "invalid-head-element" && i.element.as_deref() == Some(element)),
            "{file}"
        );
    }
    assert!(!has("noscript-link.html", "invalid-head-element"));
}

// spec: TECH-283
#[test]
fn content_before_head_excludes_missing_head() {
    assert!(has("p-before-head.html", "head-not-first"));
    assert!(has("img-before-head.html", "head-not-first"));
    assert!(has("text-before-head.html", "head-not-first"));
    assert!(!has("no-head.html", "head-not-first"));
}
// spec: TECH-284
#[test]
fn absent_source_head_is_reported_despite_dom_repair() {
    assert!(has("no-head.html", "missing-head"));
}
// spec: TECH-285
#[test]
fn multiple_head_excludes_header_and_script_text() {
    assert!(has("two-heads.html", "multiple-head"));
    assert!(!has("header-element.html", "multiple-head"));
    assert!(!has("head-in-script.html", "multiple-head"));
}
// spec: TECH-286
#[test]
fn frameset_without_body_is_missing_body() {
    assert!(has("no-body.html", "missing-body"));
}
// spec: TECH-287
#[test]
fn source_body_tags_are_counted_before_tree_repair() {
    assert!(has("two-bodies.html", "multiple-body"));
}
// spec: TECH-288
#[test]
fn body_content_before_html_excludes_metadata_comments_and_absent_html() {
    for file in [
        "body-before-html.html",
        "div-before-html.html",
        "img-before-html.html",
        "text-before-html.html",
    ] {
        assert!(has(file, "body-before-html"), "{file}");
    }
    for file in [
        "meta-before-html.html",
        "comment-before-html.html",
        "no-html.html",
    ] {
        assert!(!has(file, "body-before-html"), "{file}");
    }
}

// spec: TECH-289
// spec: TECH-290
#[test]
fn response_size_uses_decoded_bytes_and_marks_truncation_even_without_validation() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/large").unwrap();
    let body = source("ok.html");
    let headers = [("content-encoding".into(), "gzip".into())];
    let mut input = ResponseInput {
        url: &url,
        internal: true,
        purpose: os_model::FetchPurpose::Page,
        status: 200,
        headers: &headers,
        body: &body,
        content_length: Some(42),
        untruncated_bytes: Some(2_097_153),
        truncated: false,
        cfg: &cfg,
    };
    let bytes = |i: &ResponseInput<'_>| {
        validation::response_facts(i)
            .into_iter()
            .find(|(k, _)| k.as_str() == "validation.body_bytes")
            .unwrap()
            .1
    };
    assert_eq!(bytes(&input), Value::Int(2_097_153));
    input.untruncated_bytes = None;
    input.truncated = true;
    input.content_length = Some(9_999_999);
    assert_eq!(
        bytes(&input),
        Value::Int(body.len() as i64),
        "gzip length is compressed"
    );
    assert!(validation::response_facts(&input)
        .iter()
        .any(|(k, v)| k.as_str() == "validation.truncated" && *v == Value::Bool(true)));
    input.headers = &[];
    assert_eq!(bytes(&input), Value::Int(9_999_999));
    input.truncated = false;
    input.body = b"\xe4";
    assert_eq!(
        bytes(&input),
        Value::Int(1),
        "original bytes, not decoded UTF-8 length"
    );
}

// spec: TECH-280
#[test]
fn source_structure_wins_over_a_repaired_or_rendered_dom() {
    let src = source("two-heads.html");
    let html = std::str::from_utf8(&src).unwrap();
    let doc = os_parse::dom::Document::parse("<html><head></head><body></body></html>");
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/").unwrap();
    let ctx = os_parse::PageCtx {
        url: &url,
        base: &url,
        doc: &doc,
        html,
        headers: &[],
        cfg: &cfg,
    };
    let mut page = os_model::ParsedPage::default();
    validation::run(&ctx, &mut page);
    assert_eq!(page.validation.unwrap().tokens.head_count, 2);
    assert_eq!(
        facts("comment-before-html.html").document_bytes,
        source("comment-before-html.html").len() as u64
    );
}
