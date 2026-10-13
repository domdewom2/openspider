//! TECH-050: `X-Robots-Tag` (mehrere Header, mehrere Werte, Agent-Präfix, auch für Nicht-HTML).

use os_model::{CrawlConfig, DirectiveSource};
use os_parse::{parse_headers, ParseConfig};

fn cfg() -> ParseConfig {
    ParseConfig::from_crawl_config(&CrawlConfig::default())
}

fn url(s: &str) -> url::Url {
    url::Url::parse(s).unwrap()
}

fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect()
}

// spec: TECH-050
#[test]
fn several_headers_and_several_values_per_header() {
    let hints = parse_headers(
        &url("https://site.test/doc.pdf"),
        &h(&[
            ("Content-Type", "application/pdf"),
            ("X-Robots-Tag", "NoIndex, NoFollow"),
            ("x-robots-tag", "noarchive"),
        ]),
        &cfg(),
    );
    assert_eq!(hints.directives.len(), 2);
    assert_eq!(hints.directives[0].source, DirectiveSource::XRobotsTag);
    assert_eq!(hints.directives[0].tokens, ["noindex", "nofollow"]);
    assert_eq!(hints.directives[0].raw, "NoIndex, NoFollow");
    assert_eq!(hints.directives[1].tokens, ["noarchive"]);
    assert!(hints
        .directives
        .iter()
        .all(|d| d.agent.is_none() && d.applies));
    let orders: Vec<u32> = hints.directives.iter().map(|d| d.order).collect();
    assert_eq!(orders, [0, 1]);
}

// spec: TECH-050
#[test]
fn agent_prefix_binds_the_following_directives() {
    let hints = parse_headers(
        &url("https://site.test/a"),
        &h(&[("X-Robots-Tag", "googlebot: noindex, otherbot: nofollow")]),
        &cfg(),
    );
    assert_eq!(hints.directives.len(), 2);
    let g = &hints.directives[0];
    assert_eq!(g.agent.as_deref(), Some("googlebot"));
    assert_eq!(g.tokens, ["noindex"]);
    assert!(
        g.applies,
        "googlebot ist ein zusätzliches Direktiven-Token (K-12)"
    );
    let o = &hints.directives[1];
    assert_eq!(o.agent.as_deref(), Some("otherbot"));
    assert_eq!(o.tokens, ["nofollow"]);
    assert!(!o.applies);
}

// spec: TECH-050
#[test]
fn generic_part_comes_before_the_first_agent() {
    let hints = parse_headers(
        &url("https://site.test/a"),
        &h(&[("X-Robots-Tag", "noarchive, bingbot: noindex, nosnippet")]),
        &cfg(),
    );
    let view: Vec<(Option<&str>, Vec<&str>, bool)> = hints
        .directives
        .iter()
        .map(|d| {
            (
                d.agent.as_deref(),
                d.tokens.iter().map(String::as_str).collect(),
                d.applies,
            )
        })
        .collect();
    assert_eq!(
        view,
        [
            (None, vec!["noarchive"], true),
            (Some("bingbot"), vec!["noindex", "nosnippet"], false)
        ]
    );
}

// spec: TECH-050
#[test]
fn robots_token_of_the_configuration_applies() {
    let mut c = cfg();
    c.robots_token = "OpenSpider".to_string();
    let hints = parse_headers(
        &url("https://site.test/a"),
        &h(&[("X-Robots-Tag", "openspider: none")]),
        &c,
    );
    assert!(hints.directives[0].applies);
    assert_eq!(hints.directives[0].tokens, ["none"]);
}

// spec: TECH-050
#[test]
fn unavailable_after_with_commas_and_agent_in_the_header() {
    let hints = parse_headers(
        &url("https://site.test/a"),
        &h(&[(
            "X-Robots-Tag",
            "googlebot: unavailable_after: 25 Jun 2030 15:00:00 PST, noindex, bingbot: nofollow",
        )]),
        &cfg(),
    );
    assert_eq!(hints.directives.len(), 2);
    assert_eq!(hints.directives[0].tokens.len(), 2);
    assert!(hints.directives[0].tokens[0].starts_with("unavailable_after:25 jun 2030"));
    assert_eq!(hints.directives[0].tokens[1], "noindex");
    assert_eq!(hints.directives[1].agent.as_deref(), Some("bingbot"));
}
