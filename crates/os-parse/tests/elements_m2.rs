use os_model::{CrawlConfig, Value};
use os_parse::{parse_html, ParseConfig, ParseInput};
fn facts(html: &str, validation: bool) -> os_model::ParsedPage {
    let url = url::Url::parse("https://site.test/").unwrap();
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.html_validation = validation;
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    })
}
fn flag(p: &os_model::ParsedPage, key: &str) -> Option<bool> {
    p.ext
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| *v == Value::Bool(true))
}
const FIXTURE: &str = include_str!("../../../tests/fixtures/onpage-m2-elements/facts.html");
// spec: ONPAGE-099
// spec: ONPAGE-108
// spec: TECH-008 [P1]
#[test]
fn outside_uses_tree_and_respects_validation_switch() {
    let p = facts(FIXTURE, true);
    assert_eq!(flag(&p, "head.title_outside"), Some(true));
    assert_eq!(flag(&p, "head.description_outside"), Some(true));
    assert_eq!(flag(&facts(FIXTURE, false), "head.title_outside"), None);
    for html in [
        "<title>X</title><p>body",
        "<head><noscript><link href=x></noscript><title>X</title>",
    ] {
        assert_eq!(flag(&facts(html, true), "head.title_outside"), Some(false));
    }
    for html in [
        "<head><div></div><title>X</title>",
        "<head><img src=x><title>X</title>",
    ] {
        assert_eq!(flag(&facts(html, true), "head.title_outside"), Some(true));
    }
}
// spec: ONPAGE-125
// spec: ONPAGE-126
// spec: ONPAGE-135
#[test]
fn heading_facts_include_nested_alt_and_empty_headings() {
    let p = facts(FIXTURE, false);
    for key in [
        "head.h1_alt",
        "head.h1_non_sequential",
        "head.h2_non_sequential",
    ] {
        assert_eq!(flag(&p, key), Some(true));
    }
    for html in [
        "<h1>A</h1><h2>B</h2><h3>C</h3><h2>D</h2>",
        "<h2>B</h2>",
        "<template><h3>X</h3></template><h1><img alt=' '></h1><h2>B</h2>",
    ] {
        let p = facts(html, false);
        assert_eq!(flag(&p, "head.h2_non_sequential"), Some(false));
        assert_eq!(flag(&p, "head.h1_alt"), Some(false));
    }
}
// spec: ONPAGE-110
#[test]
fn keywords_normalize_name_and_value() {
    let s = os_parse::elements::extract(&os_parse::dom::Document::parse(FIXTURE));
    assert_eq!(s.meta_keywords[0].text, "a, b");
    assert_eq!(s.meta_keywords[0].len, 4);
}

// spec: ONPAGE-125
// spec: ONPAGE-126
// spec: ONPAGE-135
#[test]
fn heading_acceptance_matrix_uses_document_order() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/onpage-m2-elements/headings.json"
    ))
    .unwrap();
    for case in cases {
        let html = case["html"].as_str().unwrap();
        let page = facts(html, true);
        for (key, expected) in [
            ("head.h1_alt", "alt"),
            ("head.h1_non_sequential", "h1"),
            ("head.h2_non_sequential", "h2"),
        ] {
            assert_eq!(flag(&page, key), case[expected].as_bool(), "{html}: {key}");
        }
    }
}

// spec: ONPAGE-108
#[test]
fn descriptions_outside_head_follow_html5_tree_building() {
    for (html, outside) in [
        ("<head><meta name=description content=X></head>", false),
        ("<meta name=description content=X><p>body", false),
        ("<head><span></span><meta name=description content=X>", true),
        ("<body><meta name=description content=X>", true),
        (
            "<body><template><title>Ignored</title><meta name=description content=X></template>",
            false,
        ),
        ("<body><svg><title>SVG</title></svg>", false),
    ] {
        let page = facts(html, true);
        assert_eq!(
            flag(&page, "head.description_outside"),
            Some(outside),
            "{html}"
        );
        if !outside {
            assert_eq!(flag(&page, "head.title_outside"), Some(false), "{html}");
        }
    }
    let page = facts(FIXTURE, false);
    assert!(!page.meta_descriptions.is_empty());
    assert!(!page.titles.is_empty());
    assert_eq!(flag(&page, "head.description_outside"), None);
}
