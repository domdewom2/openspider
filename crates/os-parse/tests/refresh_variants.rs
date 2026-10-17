//! TECH-050/TECH-064: Meta-Refresh und `Refresh`-Header.

use os_model::{CrawlConfig, RefreshRef, RefreshSource};
use os_parse::dom::Document;
use os_parse::{parse_headers, refresh, ParseConfig};

fn base() -> url::Url {
    url::Url::parse("https://site.test/dir/page.html").unwrap()
}

fn extract(html: &str) -> Vec<RefreshRef> {
    refresh::extract(&Document::parse(html), &base())
}

fn one(content: &str) -> RefreshRef {
    let r = extract(&format!(
        r#"<head><meta http-equiv="refresh" content="{content}"></head>"#
    ));
    assert_eq!(r.len(), 1, "{content}");
    r.into_iter().next().unwrap()
}

// spec: TECH-050
#[test]
fn delay_and_target_are_resolved() {
    let r = one("5; url=/neu");
    assert_eq!(r.source, RefreshSource::Meta);
    assert_eq!(r.delay, Some(5));
    assert_eq!(r.target_raw.as_deref(), Some("/neu"));
    assert_eq!(r.target.as_deref(), Some("https://site.test/neu"));
    assert_eq!(r.raw, "5; url=/neu");
    assert!(r.in_head);
}

// spec: TECH-050
#[test]
fn four_spellings_of_the_target() {
    for content in [
        "0;url=/x",
        "0; URL='/x'",
        r#"0; url="/x""#,
        "0, url=/x",
        "0 ; Url = /x",
        "0;/x",
    ] {
        let r = one(&content.replace('"', "&quot;"));
        assert_eq!(r.delay, Some(0), "{content}");
        assert_eq!(r.target_raw.as_deref(), Some("/x"), "{content}");
        assert_eq!(
            r.target.as_deref(),
            Some("https://site.test/x"),
            "{content}"
        );
    }
}

// spec: TECH-050
#[test]
fn content_without_target_is_a_self_refresh() {
    for content in ["0", "10", " 30 ", "5;", "5; url="] {
        let r = one(content);
        assert_eq!(r.target_raw, None, "{content}");
        assert_eq!(r.target, None, "{content}");
        assert!(r.delay.is_some());
    }
}

// spec: TECH-064
#[test]
fn all_occurrences_are_kept_in_source_order_also_outside_the_head() {
    let r = extract(
        r#"<html><head>
        <meta http-equiv="Refresh" content="0; url=/first">
        <META HTTP-EQUIV="REFRESH" CONTENT="5;url=/second">
        </head><body><meta http-equiv="refresh" content="9; url=/third"></body></html>"#,
    );
    let targets: Vec<&str> = r.iter().map(|x| x.target_raw.as_deref().unwrap()).collect();
    assert_eq!(targets, ["/first", "/second", "/third"]);
    let in_head: Vec<bool> = r.iter().map(|x| x.in_head).collect();
    assert_eq!(in_head, [true, true, false]);
    let orders: Vec<u32> = r.iter().map(|x| x.order).collect();
    assert_eq!(orders, [0, 1, 2]);
}

// spec: TECH-050
#[test]
fn other_http_equiv_values_template_and_garbage_are_ignored() {
    let r = extract(
        r#"<head>
        <meta http-equiv="content-type" content="0; url=/x">
        <template><meta http-equiv="refresh" content="0; url=/t"></template>
        <meta http-equiv="refresh" content="url=/ohne-zahl">
        <meta http-equiv="refresh" content="">
        <meta http-equiv="refresh">
        </head>"#,
    );
    assert!(r.is_empty(), "{r:?}");
}

// spec: TECH-064
#[test]
fn query_and_fragment_targets_resolve_distinctly() {
    let r = one("0; url=https://site.test/dir/page.html?x=1");
    assert_eq!(
        r.target.as_deref(),
        Some("https://site.test/dir/page.html?x=1")
    );
    let r = one("0; url=https://site.test/dir/page.html#top");
    assert_eq!(
        r.target.as_deref(),
        Some("https://site.test/dir/page.html#top")
    );
}

// spec: TECH-050
#[test]
fn refresh_header_uses_the_same_syntax() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let headers = vec![
        ("Refresh".to_string(), "0; url=/target".to_string()),
        ("refresh".to_string(), "30".to_string()),
        ("Refresh".to_string(), "nonsense".to_string()),
    ];
    let h = parse_headers(&base(), &headers, &cfg);
    assert_eq!(h.refresh.len(), 2);
    assert_eq!(h.refresh[0].source, RefreshSource::Header);
    assert_eq!(h.refresh[0].delay, Some(0));
    assert_eq!(
        h.refresh[0].target.as_deref(),
        Some("https://site.test/target")
    );
    assert_eq!(h.refresh[1].target_raw, None);
    assert_eq!(h.refresh[1].delay, Some(30));
    assert_eq!((h.refresh[0].order, h.refresh[1].order), (0, 1));
}

// spec: TECH-064
#[test]
fn parse_html_puts_header_refresh_after_meta_refresh() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let headers = vec![("Refresh".to_string(), "0; url=/h".to_string())];
    let u = base();
    let page = os_parse::parse_html(&os_parse::ParseInput {
        status: 200,
        url: &u,
        body: br#"<head><meta http-equiv="refresh" content="3; url=/m"></head>"#,
        content_type: Some("text/html"),
        headers: &headers,
        cfg: &cfg,
    });
    assert_eq!(page.refresh.len(), 2);
    assert_eq!(page.refresh[0].source, RefreshSource::Meta);
    assert_eq!(page.refresh[1].source, RefreshSource::Header);
    assert!(page.refresh[1].order > page.refresh[0].order);
}
