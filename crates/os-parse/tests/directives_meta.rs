//! TECH-050: Meta-Robots-Extraktion (generisch, agentenspezifisch, wertbehaftet, Head/Body).

use os_model::{CrawlConfig, Directive, DirectiveSource};
use os_parse::dom::Document;
use os_parse::{directives, ParseConfig};

fn tokens() -> Vec<String> {
    vec!["openspider".to_string(), "googlebot".to_string()]
}

fn extract(html: &str) -> Vec<Directive> {
    directives::extract(&Document::parse(html), &tokens())
}

// spec: TECH-050
#[test]
fn name_and_content_are_case_insensitive_and_trimmed() {
    let d = extract(
        r#"<html><head><META NAME="Robots" CONTENT="NoIndex , FOLLOW"></head><body></body></html>"#,
    );
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].source, DirectiveSource::MetaRobots);
    assert_eq!(d[0].agent, None);
    assert_eq!(d[0].tokens, ["noindex", "follow"]);
    assert_eq!(d[0].raw, "NoIndex , FOLLOW");
    assert!(d[0].applies);
    assert!(d[0].in_head);
}

// spec: TECH-050
#[test]
fn valued_directives_are_stored_with_their_value() {
    let d = extract(
        r#"<head><meta name="robots" content="max-snippet:50, max-image-preview:large, max-video-preview:-1, unavailable_after: 2026-12-31"></head>"#,
    );
    assert_eq!(
        d[0].tokens,
        [
            "max-snippet:50",
            "max-image-preview:large",
            "max-video-preview:-1",
            "unavailable_after:2026-12-31"
        ]
    );
}

// spec: TECH-050
#[test]
fn unavailable_after_date_with_commas_is_not_split() {
    let d = extract(
        r#"<head><meta name="robots" content="noindex, unavailable_after: Thursday, 31-Dec-26 23:59:59 GMT, nofollow"></head>"#,
    );
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].tokens.len(), 3, "{:?}", d[0].tokens);
    assert_eq!(d[0].tokens[0], "noindex");
    assert!(d[0].tokens[1].starts_with("unavailable_after:thursday,31-dec-26"));
    assert_eq!(d[0].tokens[2], "nofollow");

    let d = extract(
        r#"<head><meta name="robots" content="unavailable_after: Thu, 31 Dec 2026 23:59:59 GMT"></head>"#,
    );
    assert_eq!(d[0].tokens.len(), 1, "{:?}", d[0].tokens);
}

// spec: TECH-050
#[test]
fn generic_and_token_agents_apply_other_agents_are_only_shown() {
    let d = extract(
        r#"<head>
        <meta name="robots" content="index">
        <meta name="googlebot" content="noindex">
        <meta name="OpenSpider" content="nofollow">
        <meta name="bingbot" content="noindex">
        <meta name="description" content="noindex wird hier ignoriert">
        </head>"#,
    );
    let view: Vec<(Option<&str>, bool)> =
        d.iter().map(|x| (x.agent.as_deref(), x.applies)).collect();
    assert_eq!(
        view,
        [
            (None, true),
            (Some("googlebot"), true),
            (Some("openspider"), true),
            (Some("bingbot"), false)
        ]
    );
    let orders: Vec<u32> = d.iter().map(|x| x.order).collect();
    assert_eq!(orders, [0, 1, 2, 3]);
}

// spec: TECH-050
#[test]
fn the_configured_token_decides_which_agent_applies() {
    let html = r#"<head><meta name="mybot" content="noindex"><meta name="googlebot" content="nofollow"></head>"#;
    let doc = Document::parse(html);
    let d = directives::extract(&doc, &["MyBot".to_string()]);
    assert_eq!(d.len(), 2);
    assert!(d[0].applies, "mybot passt zum Token MyBot");
    assert!(
        !d[1].applies,
        "googlebot ist hier kein konfiguriertes Token"
    );
}

// spec: TECH-050
#[test]
fn meta_robots_in_the_body_is_extracted_and_marked_outside_head() {
    let d = extract(
        r#"<html><head><title>t</title></head><body><p>x</p><meta name="robots" content="noindex"></body></html>"#,
    );
    assert_eq!(d.len(), 1);
    assert!(!d[0].in_head);
    let d = extract(
        r#"<html><head><img src="a.png"><meta name="robots" content="noindex"></head><body></body></html>"#,
    );
    assert_eq!(d.len(), 1);
    assert!(!d[0].in_head, "ein img beendet den Head implizit");
}

// spec: TECH-050
#[test]
fn template_comments_and_empty_content_are_ignored() {
    let d = extract(
        r#"<head><template><meta name="robots" content="noindex"></template>
        <!-- <meta name="robots" content="noindex"> -->
        <meta name="robots" content="">
        <meta name="robots"></head>"#,
    );
    assert!(d.is_empty(), "{d:?}");
}

// spec: TECH-050
#[test]
fn parse_html_uses_the_robots_token_plus_googlebot() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/").unwrap();
    let body = br#"<head><meta name="openspider" content="noindex"><meta name="googlebot" content="nofollow"><meta name="yandex" content="noindex"></head>"#;
    let page = os_parse::parse_html(&os_parse::ParseInput {
        url: &url,
        body,
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    let applied: Vec<&str> = page
        .directives
        .iter()
        .filter(|d| d.applies)
        .filter_map(|d| d.agent.as_deref())
        .collect();
    assert_eq!(applied, ["openspider", "googlebot"]);
    assert_eq!(page.directives.len(), 3);
    assert!(!page.effective_index);
    assert!(!page.effective_follow);
}
