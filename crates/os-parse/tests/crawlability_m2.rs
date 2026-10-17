use os_model::{CrawlConfig, UncrawlableReason};
use os_parse::{parse_html, ParseConfig, ParseInput};

// spec: CRAWL-053 [M2]
#[test]
fn catalog_replaces_pseudo_anchors_without_losing_real_links_or_templates() {
    let url = url::Url::parse("https://site.test/").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.spider.store_uncrawlable_links = true;
    let cfg = ParseConfig::from_crawl_config(&cfg);
    let body =
        include_bytes!("../../../tests/fixtures/crawl-m2-switches/crawl-resources/index.html");
    let page = parse_html(&ParseInput {
        status: 200,
        url: &url,
        body,
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    let pseudo: Vec<_> = page.links.iter().filter(|l| !l.crawlable).collect();
    assert_eq!(pseudo.len(), 7);
    assert!(pseudo.iter().any(|l| l.uncrawlable_reason
        == Some(UncrawlableReason::AnchorReturnFalse)
        && l.resolved.as_deref() == Some("https://site.test/blocked")));
    assert!(pseudo
        .iter()
        .any(|l| l.resolved.as_deref() == Some("https://site.test/onclick")));
    assert!(page
        .links
        .iter()
        .any(|l| l.href_raw == "/ok" && l.crawlable));
    assert_eq!(
        page.links
            .iter()
            .filter(|l| l.href_raw == "/blocked")
            .count(),
        1
    );
}

// spec: CRAWL-053
#[test]
fn missing_empty_fragment_and_javascript_without_literals_keep_explicit_reasons() {
    let url = url::Url::parse("https://site.test/").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.spider.store_uncrawlable_links = true;
    let cfg = ParseConfig::from_crawl_config(&cfg);
    let doc = os_parse::dom::Document::parse("<template><span href='/skip'></span></template><a>missing</a><a href=''>empty</a><a href='#top'>fragment</a><a href='javascript:void(0)'>script</a>");
    let links = os_parse::crawlability::inspect(&doc, &url, &cfg);
    assert_eq!(links.len(), 4);
    assert_eq!(
        links
            .iter()
            .map(|l| l.uncrawlable_reason.unwrap())
            .collect::<Vec<_>>(),
        [
            UncrawlableReason::AnchorMissingHref,
            UncrawlableReason::AnchorEmptyHref,
            UncrawlableReason::AnchorFragmentOnly,
            UncrawlableReason::JavascriptHref
        ]
    );
    assert!(links[3].resolved.is_none());
    assert_eq!(links[3].xpath, "javascript:void(0)");
}

// spec: CRAWL-053 [M2]
#[test]
fn catalog_includes_non_anchor_onclick_navigation_from_the_shared_rule_union() {
    let base = url::Url::parse("https://site.test/").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.spider.store_uncrawlable_links = true;
    let cfg = ParseConfig::from_crawl_config(&cfg);
    let doc = os_parse::dom::Document::parse(include_str!(
        "../../../tests/fixtures/crawl-m2-switches/crawl-uncrawlable/index.html"
    ));
    let links = os_parse::crawlability::inspect(&doc, &base, &cfg);
    assert_eq!(links.len(), 3);
    assert_eq!(
        links[0].resolved.as_deref(),
        Some("https://site.test/div-navigation")
    );
    assert_eq!(
        links[1].resolved.as_deref(),
        Some("https://site.test/button-navigation")
    );
    assert!(links[2].resolved.is_none());
    assert_eq!(links[2].xpath, "window.open(destination)");
    assert!(links.iter().all(|l| !l.crawlable));
}
