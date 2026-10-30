use os_model::{CrawlConfig, Value};
use os_parse::{canonical, directives, dom::Document, ParseConfig};

// spec: TECH-021, TECH-023, TECH-086
#[test]
fn problem_evidence_survives_independently_of_validation() {
    let url = url::Url::parse("http://site.test/body.html").unwrap();
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/n09-technical/body.html"
    ));
    let refs = canonical::extract(&doc, &url);
    assert_eq!(refs.len(), 1);
    assert!(!refs[0].in_head);
    let facts = canonical::annotation_facts(&refs);
    assert!(facts
        .iter()
        .any(|(k, v)| k.as_str() == "canonical.outside_head" && *v == Value::Bool(true)));
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let (refs, _) = os_parse::hreflang::extract(&doc, &url, &cfg);
    assert_eq!(refs.len(), 1);
    assert!(!refs[0].in_head);
}

// spec: TECH-063
#[test]
fn stored_directive_values_preserve_dates_with_commas_and_choose_earliest() {
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/n09-technical/expired.html"
    ));
    let refs = directives::extract(&doc, &["googlebot".into()]);
    assert_eq!(
        directives::unavailable_after(&refs[0].raw).unwrap().to_db(),
        "2000-01-01T00:00:00.000Z"
    );
    assert_eq!(
        directives::unavailable_after(
            "noarchive, unavailable_after: Saturday, 01-Jan-00 00:00:00 GMT, nofollow"
        )
        .unwrap()
        .to_db(),
        "2000-01-01T00:00:00.000Z"
    );
    assert!(directives::unavailable_after("noindex, unavailable_after: invalid").is_none());
}

// spec: ONPAGE-094
#[test]
fn malformed_head_keeps_body_title_for_the_length_filter() {
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/n09-technical/malformed.html"
    ));
    let sets = os_parse::elements::extract(&doc);
    assert_eq!(sets.titles.len(), 1);
    assert_eq!(sets.titles[0].text, "Late html");
    assert_eq!(sets.titles[0].len, 9);
    assert!(!sets.titles[0].in_head);
}

// spec: TECH-283
// spec: TECH-284
#[test]
fn missing_head_is_also_not_first() {
    let html = include_str!("../../../tests/fixtures/n09-technical/head-missing.html");
    let facts = os_parse::validation::analyze(html);
    for code in ["missing-head", "head-not-first"] {
        assert!(facts.issues.iter().any(|i| i.code == code), "{code}");
    }
    let valid = os_parse::validation::analyze(
        "<!doctype html><!-- comment --><html> \n<head></head><body></body></html>",
    );
    assert!(!valid.issues.iter().any(|i| i.code == "head-not-first"));
}
