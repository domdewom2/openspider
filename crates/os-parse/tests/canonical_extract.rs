//! TECH-010: Canonical-Extraktion aus `<link>` und `Link`-Header.

use os_model::{CanonicalRef, CrawlConfig, HintSource};
use os_parse::dom::Document;
use os_parse::{canonical, parse_headers, ParseConfig};

fn base() -> url::Url {
    url::Url::parse("https://site.test/dir/page.html").unwrap()
}

fn extract(html: &str) -> Vec<CanonicalRef> {
    canonical::extract(&Document::parse(html), &base())
}

fn header_hints(hdrs: &[(&str, &str)], url: &str) -> os_model::HeaderHints {
    let headers: Vec<(String, String)> = hdrs
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    parse_headers(&url::Url::parse(url).unwrap(), &headers, &cfg)
}

// spec: TECH-010
#[test]
fn html_canonical_raw_resolved_and_in_head() {
    let c =
        extract(r#"<html><head><link rel="canonical" href="/canon/a"></head><body></body></html>"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].source, HintSource::Html);
    assert_eq!(c[0].raw, "/canon/a");
    assert_eq!(c[0].resolved.as_deref(), Some("https://site.test/canon/a"));
    assert!(c[0].in_head);
    assert_eq!(c[0].order, 0);
}

// spec: TECH-010
#[test]
fn rel_is_a_case_insensitive_token_list() {
    let c = extract(
        r#"<head>
        <link rel="Canonical" href="/a">
        <link rel="canonical alternate" href="/b">
        <link rel="ALTERNATE   CANONICAL" href="/c">
        <link rel="canonicalx" href="/nein">
        <link rel="notcanonical" href="/nein2">
        </head>"#,
    );
    let raws: Vec<&str> = c.iter().map(|x| x.raw.as_str()).collect();
    assert_eq!(raws, ["/a", "/b", "/c"]);
    let orders: Vec<u32> = c.iter().map(|x| x.order).collect();
    assert_eq!(orders, [0, 1, 2]);
}

// spec: TECH-010
#[test]
fn link_without_href_is_ignored_empty_href_resolves_to_the_page() {
    let c = extract(r#"<head><link rel="canonical"><link rel="canonical" href=""></head>"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].raw, "");
    assert_eq!(
        c[0].resolved.as_deref(),
        Some("https://site.test/dir/page.html")
    );
}

// spec: TECH-010
#[test]
fn unresolvable_values_keep_the_raw_value_only() {
    let c = extract(r#"<head><link rel="canonical" href="http://[bad"></head>"#);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].raw, "http://[bad");
    assert_eq!(c[0].resolved, None);
}

// spec: TECH-010
#[test]
fn template_and_comments_are_ignored_noscript_in_head_is_kept() {
    let c = extract(
        r#"<html><head>
        <template><link rel="canonical" href="/im-template"></template>
        <!-- <link rel="canonical" href="/im-kommentar"> -->
        <noscript><link rel="canonical" href="/im-noscript"></noscript>
        </head><body></body></html>"#,
    );
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].raw, "/im-noscript");
    assert!(c[0].in_head, "noscript mit link beendet den Head nicht");
}

// spec: TECH-010
#[test]
fn canonical_outside_the_head_is_marked() {
    let c = extract(
        r#"<html><head><title>t</title></head><body><link rel="canonical" href="/b"></body></html>"#,
    );
    assert_eq!(c.len(), 1);
    assert!(!c[0].in_head);
    let c = extract(
        r#"<html><head><img src="a.png"><link rel="canonical" href="/c"></head><body></body></html>"#,
    );
    assert!(!c[0].in_head, "ein img im Head beendet ihn implizit");
}

// spec: TECH-010
#[test]
fn base_href_is_honoured() {
    let c = extract(
        r#"<head><base href="https://cdn.test/x/"><link rel="canonical" href="../y"></head>"#,
    );
    // extract bekommt die bereits aufgelöste Basis; hier die Dokument-URL als Basis
    assert_eq!(c[0].resolved.as_deref(), Some("https://site.test/y"));
    let doc = Document::parse(
        r#"<head><base href="https://cdn.test/x/"><link rel="canonical" href="../y"></head>"#,
    );
    let resolved_base = os_parse::base::resolve_base(&doc, &base());
    let c = canonical::extract(&doc, &resolved_base);
    assert_eq!(c[0].resolved.as_deref(), Some("https://cdn.test/y"));
}

// spec: TECH-010
#[test]
fn two_link_headers_and_one_header_with_two_entries() {
    let h = header_hints(
        &[
            ("Link", r#"<https://site.test/c1>; rel="canonical""#),
            ("link", r#"</c2>; rel=canonical, </other>; rel="prefetch""#),
            (
                "Link",
                r#"<https://site.test/c3>; rel="canonical"; title="x, y", <https://site.test/c4>;rel=canonical"#,
            ),
        ],
        "https://site.test/doc",
    );
    let resolved: Vec<&str> = h
        .canonicals
        .iter()
        .map(|c| c.resolved.as_deref().unwrap())
        .collect();
    assert_eq!(
        resolved,
        [
            "https://site.test/c1",
            "https://site.test/c2",
            "https://site.test/c3",
            "https://site.test/c4"
        ]
    );
    assert!(h.canonicals.iter().all(|c| c.source == HintSource::Http));
    assert_eq!(h.canonicals[1].raw, "/c2");
    let orders: Vec<u32> = h.canonicals.iter().map(|c| c.order).collect();
    assert_eq!(orders, [0, 1, 2, 3]);
}

// spec: TECH-010
#[test]
fn parameters_with_and_without_quotes() {
    let h = header_hints(
        &[("Link", r#"<https://site.test/a>; rel=canonical"#)],
        "https://site.test/doc",
    );
    assert_eq!(h.canonicals.len(), 1);
    let h = header_hints(
        &[("Link", r#"<https://site.test/a>; rel="canonical""#)],
        "https://site.test/doc",
    );
    assert_eq!(h.canonicals.len(), 1);
    let h = header_hints(
        &[(
            "Link",
            r#"<https://site.test/a>; REL="Alternate Canonical""#,
        )],
        "https://site.test/doc",
    );
    assert_eq!(h.canonicals.len(), 1);
}

// spec: TECH-010
#[test]
fn http_canonical_is_captured_for_non_html_responses() {
    let h = header_hints(
        &[
            ("Content-Type", "application/pdf"),
            ("Link", r#"<https://site.test/landing>; rel="canonical""#),
        ],
        "https://site.test/files/doc.pdf",
    );
    assert_eq!(h.canonicals.len(), 1);
    assert_eq!(
        h.canonicals[0].resolved.as_deref(),
        Some("https://site.test/landing")
    );
}

// spec: TECH-010
#[test]
fn parse_html_appends_header_canonicals_after_html_ones() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/p").unwrap();
    let headers = vec![(
        "Link".to_string(),
        r#"<https://site.test/http>; rel="canonical""#.to_string(),
    )];
    let page = os_parse::parse_html(&os_parse::ParseInput {
        url: &url,
        body: br#"<head><link rel="canonical" href="/html"></head>"#,
        content_type: Some("text/html"),
        headers: &headers,
        cfg: &cfg,
    });
    assert_eq!(page.canonicals.len(), 2);
    assert_eq!(page.canonicals[0].source, HintSource::Html);
    assert_eq!(page.canonicals[1].source, HintSource::Http);
    assert!(page.canonicals[1].order > page.canonicals[0].order);
}
