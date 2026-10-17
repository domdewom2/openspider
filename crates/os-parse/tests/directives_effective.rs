//! TECH-066: Konfliktauflösung widersprüchlicher Direktiven (restriktivste gewinnt).

use os_model::{CrawlConfig, Directive};
use os_parse::dom::Document;
use os_parse::{directives, effective, headers, ParseConfig};

fn cfg() -> ParseConfig {
    ParseConfig::from_crawl_config(&CrawlConfig::default())
}

fn page(html: &str, hdrs: &[(&str, &str)]) -> os_model::ParsedPage {
    let url = url::Url::parse("https://site.test/p").unwrap();
    let headers: Vec<(String, String)> = hdrs
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
    let cfg = cfg();
    os_parse::parse_html(&os_parse::ParseInput {
        status: 200,
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html"),
        headers: &headers,
        cfg: &cfg,
    })
}

fn meta(html: &str) -> Vec<Directive> {
    directives::extract(
        &Document::parse(html),
        &["openspider".to_string(), "googlebot".to_string()],
    )
}

// spec: TECH-066
#[test]
fn index_and_noindex_in_two_tags_noindex_wins() {
    let p = page(
        r#"<head><meta name="robots" content="index"><meta name="robots" content="noindex"></head>"#,
        &[],
    );
    assert!(!p.effective_index);
    assert!(p.effective_follow);
}

// spec: TECH-066
#[test]
fn index_and_noindex_in_one_tag_noindex_wins() {
    let p = page(
        r#"<head><meta name="robots" content="index, noindex, follow"></head>"#,
        &[],
    );
    assert!(!p.effective_index);
    assert!(p.effective_follow);
}

// spec: TECH-066
#[test]
fn follow_and_nofollow_nofollow_wins() {
    let p = page(
        r#"<head><meta name="robots" content="follow"><meta name="robots" content="nofollow"></head>"#,
        &[],
    );
    assert!(p.effective_index);
    assert!(!p.effective_follow);
}

// spec: TECH-066
#[test]
fn meta_index_and_header_noindex_noindex_wins() {
    let p = page(
        r#"<head><meta name="robots" content="index,follow"></head>"#,
        &[("X-Robots-Tag", "noindex")],
    );
    assert!(!p.effective_index);
    assert!(p.effective_follow);
}

// spec: TECH-066
#[test]
fn none_means_noindex_and_nofollow() {
    let p = page(r#"<head><meta name="robots" content="none"></head>"#, &[]);
    assert!(!p.effective_index);
    assert!(!p.effective_follow);
    let p = page("<head></head>", &[("X-Robots-Tag", "NONE")]);
    assert!(!p.effective_index);
    assert!(!p.effective_follow);
}

// spec: TECH-066
#[test]
fn only_applying_directives_count() {
    let p = page(
        r#"<head><meta name="bingbot" content="noindex, nofollow"></head>"#,
        &[("X-Robots-Tag", "otherbot: noindex")],
    );
    assert!(p.effective_index);
    assert!(p.effective_follow);
    // googlebot ist ein Direktiven-Token (K-12)
    let p = page(
        r#"<head><meta name="googlebot" content="noindex"></head>"#,
        &[("X-Robots-Tag", "googlebot: nofollow")],
    );
    assert!(!p.effective_index);
    assert!(!p.effective_follow);
}

// spec: TECH-066
#[test]
fn noindex_in_the_body_still_applies() {
    let p = page(
        r#"<html><head></head><body><meta name="robots" content="noindex"></body></html>"#,
        &[],
    );
    assert!(!p.effective_index);
}

// spec: TECH-066
#[test]
fn compute_without_directives_is_index_follow() {
    assert_eq!(effective::compute(&[], &[]), (true, true));
    let html = meta(r#"<head><meta name="robots" content="all, max-snippet:10"></head>"#);
    assert_eq!(effective::compute(&html, &[]), (true, true));
}

// spec: TECH-066
#[test]
fn merge_keeps_header_directives_after_html_ones() {
    let p = page(
        r#"<head><meta name="robots" content="nofollow"></head>"#,
        &[("X-Robots-Tag", "noarchive")],
    );
    assert_eq!(p.directives.len(), 2);
    assert_eq!(p.directives[0].tokens, ["nofollow"]);
    assert_eq!(p.directives[1].tokens, ["noarchive"]);
    assert_eq!((p.directives[0].order, p.directives[1].order), (0, 1));

    let mut target = page("<head></head>", &[]);
    let hints = headers::hints(
        &url::Url::parse("https://site.test/p").unwrap(),
        &[("X-Robots-Tag".to_string(), "noindex".to_string())],
        &cfg(),
    );
    headers::merge(&mut target, hints);
    assert_eq!(target.directives.len(), 1);
}
