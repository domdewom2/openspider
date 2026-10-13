//! TECH-030: rel=next/prev aus `<link>` und `Link`-Header.

use os_model::{CrawlConfig, HintSource};
use os_parse::dom::Document;
use os_parse::{pagination, parse_headers, ParseConfig};

fn base() -> url::Url {
    url::Url::parse("https://site.test/list?page=2").unwrap()
}

// spec: TECH-030
#[test]
fn next_and_prev_link_elements() {
    let p = pagination::extract(
        &Document::parse(
            r#"<head><link rel="prev" href="/list?page=1"><link rel="NEXT" href="?page=3"></head>"#,
        ),
        &base(),
    );
    assert_eq!(p.len(), 2);
    assert!(!p[0].rel_next);
    assert_eq!(
        p[0].resolved.as_deref(),
        Some("https://site.test/list?page=1")
    );
    assert!(p[1].rel_next);
    assert_eq!(p[1].raw, "?page=3");
    assert_eq!(
        p[1].resolved.as_deref(),
        Some("https://site.test/list?page=3")
    );
    assert!(p.iter().all(|x| x.source == HintSource::Html && x.in_head));
    assert_eq!((p[0].order, p[1].order), (0, 1));
}

// spec: TECH-030
#[test]
fn previous_counts_as_prev() {
    let p = pagination::extract(
        &Document::parse(r#"<head><link rel="previous" href="/list?page=1"></head>"#),
        &base(),
    );
    assert_eq!(p.len(), 1);
    assert!(!p[0].rel_next);
}

// spec: TECH-030
#[test]
fn anchors_do_not_count() {
    let p = pagination::extract(
        &Document::parse(r#"<body><a rel="next" href="/list?page=3">weiter</a></body>"#),
        &base(),
    );
    assert!(p.is_empty());
}

// spec: TECH-030
#[test]
fn link_header_next_and_prev_with_duplicates() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let headers = vec![
        (
            "Link".to_string(),
            r#"</list?page=3>; rel="next", </list?page=1>; rel="prev""#.to_string(),
        ),
        (
            "Link".to_string(),
            r#"</list?page=3>; rel=next"#.to_string(),
        ),
    ];
    let h = parse_headers(&base(), &headers, &cfg);
    let view: Vec<(bool, &str)> = h
        .pagination
        .iter()
        .map(|p| (p.rel_next, p.resolved.as_deref().unwrap()))
        .collect();
    assert_eq!(
        view,
        [
            (true, "https://site.test/list?page=3"),
            (false, "https://site.test/list?page=1"),
            (true, "https://site.test/list?page=3"),
        ]
    );
    assert!(h.pagination.iter().all(|p| p.source == HintSource::Http));
}
