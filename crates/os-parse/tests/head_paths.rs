//! Elemente im `<head>`: Position `Head` und Pfad in der Form der Referenz `//head/<element>[n]` für Links,
//! Canonical, hreflang, Paginierung und Meta Refresh (OUT-031, TECH-131, K-63, Paritätsbefund N-09).

use os_model::{CrawlConfig, HintSource, LinkPosition, LinkType};
use os_parse::{parse_headers, parse_html, ParseConfig, ParseInput};

const PAGE: &str = r#"<!doctype html>
<html><head>
<meta charset="utf-8">
<link rel="canonical" href="/p">
<link rel="stylesheet" href="/site.css">
<link rel="alternate" hreflang="de" href="/de">
<link rel="next" href="/p2">
<link rel="prev" href="/p0">
<meta http-equiv="refresh" content="5; url=/ziel">
<style>h1 { background: url(/bg.png) }</style>
<script src="/app.js"></script>
<title>t</title>
</head><body>
<nav><a href="/n">n</a></nav>
<p><a href="/c">c</a></p>
<link rel="canonical" href="/body-canonical">
<script src="/body.js"></script>
</body></html>"#;

fn parse() -> os_model::ParsedPage {
    let url = url::Url::parse("https://site.test/p").unwrap();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    parse_html(&ParseInput {
        url: &url,
        body: PAGE.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    })
}

// spec: OUT-031, TECH-131
#[test]
fn links_in_the_head_have_position_head_and_head_relative_paths() {
    let page = parse();
    let get = |t: &str| {
        page.links
            .iter()
            .find(|l| l.href_raw == t)
            .unwrap_or_else(|| panic!("{t}"))
    };
    let css = get("/site.css");
    assert_eq!(css.link_type, LinkType::Css);
    assert_eq!(css.position, LinkPosition::Head);
    assert_eq!(css.xpath, "//head/link[2]", "zweites <link> im Kopf");
    let js = get("/app.js");
    assert_eq!(js.position, LinkPosition::Head);
    assert_eq!(js.xpath, "//head/script[1]");
    let bg = get("/bg.png");
    assert_eq!(
        bg.position,
        LinkPosition::Head,
        "url() im <style> des Kopfes"
    );
    assert_eq!(bg.xpath, "//head/style[1]");
    // Body bleibt wie bisher
    assert_eq!(get("/n").position, LinkPosition::Navigation);
    assert_eq!(get("/n").xpath, "/html[1]/body[1]/nav[1]/a[1]");
    assert_eq!(get("/c").position, LinkPosition::Content);
    assert_eq!(get("/body.js").position, LinkPosition::Content);
    assert_eq!(get("/body.js").xpath, "/html[1]/body[1]/script[1]");
}

// spec: OUT-031, TECH-131
#[test]
fn head_references_carry_the_path_of_their_element() {
    let page = parse();
    // <meta charset> ist das erste <meta>, der Refresh das zweite
    assert_eq!(page.refresh.len(), 1);
    assert_eq!(page.refresh[0].xpath.as_deref(), Some("//head/meta[2]"));
    // <link>: canonical [1], stylesheet [2], alternate [3], next [4], prev [5]
    assert_eq!(page.canonicals.len(), 2);
    assert_eq!(page.canonicals[0].xpath.as_deref(), Some("//head/link[1]"));
    assert!(page.canonicals[0].in_head);
    assert_eq!(page.hreflang[0].xpath.as_deref(), Some("//head/link[3]"));
    assert_eq!(page.pagination.len(), 2);
    assert_eq!(page.pagination[0].xpath.as_deref(), Some("//head/link[4]"));
    assert_eq!(page.pagination[1].xpath.as_deref(), Some("//head/link[5]"));
    // ein <link rel=canonical> im Body behält den absoluten Pfad
    assert!(!page.canonicals[1].in_head);
    assert_eq!(
        page.canonicals[1].xpath.as_deref(),
        Some("/html[1]/body[1]/link[1]")
    );
}

// spec: OUT-031, TECH-131
#[test]
fn header_sources_have_no_path() {
    let url = url::Url::parse("https://site.test/p").unwrap();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let headers = vec![
        (
            "Link".to_string(),
            "</c>; rel=\"canonical\", </de>; rel=\"alternate\"; hreflang=\"de\", </n>; rel=\"next\""
                .to_string(),
        ),
        ("Refresh".to_string(), "5; url=/ziel".to_string()),
    ];
    let hints = parse_headers(&url, &headers, &cfg);
    assert!(hints
        .canonicals
        .iter()
        .all(|c| c.source == HintSource::Http && c.xpath.is_none()));
    assert!(hints.hreflang.iter().all(|h| h.xpath.is_none()));
    assert!(hints.pagination.iter().all(|p| p.xpath.is_none()));
    assert!(hints.refresh.iter().all(|r| r.xpath.is_none()));
    assert!(!hints.canonicals.is_empty() && !hints.refresh.is_empty());
}
