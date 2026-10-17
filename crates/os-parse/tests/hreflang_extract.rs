//! TECH-070: hreflang-Extraktion aus HTML und HTTP-Header (Sitemap: `sitemap_hreflang.rs`).

use os_model::{CrawlConfig, HintSource, HreflangOverLimit, HreflangRef};
use os_parse::dom::Document;
use os_parse::{hreflang, parse_headers, ParseConfig};

fn cfg() -> ParseConfig {
    ParseConfig::from_crawl_config(&CrawlConfig::default())
}

fn base() -> url::Url {
    url::Url::parse("https://site.test/en/").unwrap()
}

fn extract(html: &str) -> (Vec<HreflangRef>, u32) {
    hreflang::extract(&Document::parse(html), &base(), &cfg())
}

// spec: TECH-070
#[test]
fn html_annotations_with_raw_normalized_and_resolved_values() {
    let (refs, cut) = extract(
        r#"<head>
        <link rel="alternate" hreflang="en" href="/en/">
        <link rel="alternate" hreflang="DE-at" href="https://site.test/de-at/">
        <link rel="alternate" hreflang="x-default" href="../">
        </head>"#,
    );
    assert_eq!(cut, 0);
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[1].code_raw, "DE-at");
    assert_eq!(refs[1].code_norm, "de-at");
    assert!(refs[1].code_valid);
    assert_eq!(refs[1].invalid_reason, None);
    assert_eq!(refs[0].resolved.as_deref(), Some("https://site.test/en/"));
    assert_eq!(refs[2].href_raw, "../");
    assert_eq!(refs[2].resolved.as_deref(), Some("https://site.test/"));
    assert!(refs
        .iter()
        .all(|r| r.source == HintSource::Html && r.in_head));
    let orders: Vec<u32> = refs.iter().map(|r| r.order).collect();
    assert_eq!(orders, [0, 1, 2]);
}

// spec: TECH-070
#[test]
fn only_alternate_with_non_empty_hreflang_counts() {
    let (refs, _) = extract(
        r#"<head>
        <link rel="alternate" type="application/rss+xml" href="/feed.xml">
        <link rel="alternate" media="only screen and (max-width: 640px)" href="/m/">
        <link rel="alternate" hreflang="" href="/leer">
        <link rel="alternate" hreflang="   " href="/blank">
        <link rel="stylesheet" hreflang="de" href="/kein-alternate">
        <link rel="alternate" hreflang="de">
        <link rel="Alternate  Home" hreflang="fr" href="/fr/">
        </head>"#,
    );
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].code_raw, "fr");
}

// spec: TECH-070
#[test]
fn outside_head_is_marked_template_is_ignored() {
    let (refs, _) = extract(
        r#"<html><head><title>x</title></head><body>
        <link rel="alternate" hreflang="de" href="/de/">
        <template><link rel="alternate" hreflang="fr" href="/fr/"></template>
        </body></html>"#,
    );
    assert_eq!(refs.len(), 1);
    assert!(!refs[0].in_head);
}

// spec: TECH-070
#[test]
fn http_header_annotations_for_non_html() {
    let headers = vec![
        (
            "Link".to_string(),
            r#"<https://site.test/pdf-en.pdf>; rel="alternate"; hreflang="en", </pdf-de.pdf>; rel=alternate; hreflang=de"#
                .to_string(),
        ),
        (
            "Link".to_string(),
            r#"</no-code.pdf>; rel="alternate"; type="application/pdf""#.to_string(),
        ),
    ];
    let h = parse_headers(
        &url::Url::parse("https://site.test/pdf-en.pdf").unwrap(),
        &headers,
        &cfg(),
    );
    assert_eq!(h.hreflang.len(), 2);
    assert_eq!(h.hreflang[0].source, HintSource::Http);
    assert_eq!(h.hreflang[0].code_raw, "en");
    assert_eq!(h.hreflang[1].code_raw, "de");
    assert_eq!(
        h.hreflang[1].resolved.as_deref(),
        Some("https://site.test/pdf-de.pdf")
    );
    assert!(h.hreflang.iter().all(|r| r.code_valid));
}

// spec: TECH-070
#[test]
fn html_and_header_annotations_are_merged_by_parse_html() {
    let headers = vec![(
        "Link".to_string(),
        r#"</fr/>; rel="alternate"; hreflang="fr""#.to_string(),
    )];
    let c = cfg();
    let u = base();
    let page = os_parse::parse_html(&os_parse::ParseInput {
        status: 200,
        url: &u,
        body: br#"<head><link rel="alternate" hreflang="de" href="/de/"></head>"#,
        content_type: Some("text/html"),
        headers: &headers,
        cfg: &c,
    });
    let sources: Vec<(HintSource, &str)> = page
        .hreflang
        .iter()
        .map(|r| (r.source, r.code_raw.as_str()))
        .collect();
    assert_eq!(
        sources,
        [(HintSource::Html, "de"), (HintSource::Http, "fr")]
    );
    assert_eq!(page.hreflang_truncated, 0);
}

fn many(n: usize) -> String {
    let mut s = String::from("<head>");
    for i in 0..n {
        s.push_str(&format!(
            r#"<link rel="alternate" hreflang="de" href="/p/{i}">"#
        ));
    }
    s.push_str("</head>");
    s
}

// spec: TECH-070
#[test]
fn up_to_500_annotations_are_kept_more_are_truncated_and_counted() {
    let (refs, cut) = extract(&many(500));
    assert_eq!((refs.len(), cut), (500, 0));

    let (refs, cut) = extract(&many(510));
    assert_eq!((refs.len(), cut), (500, 10));
    assert_eq!(refs[499].href_raw, "/p/499");
    assert_eq!(refs[499].order, 499);
}

// spec: TECH-070
#[test]
fn skip_mode_drops_everything_over_the_limit() {
    let mut c = cfg();
    c.hreflang_over_limit = HreflangOverLimit::Skip;
    let (refs, cut) = hreflang::extract(&Document::parse(&many(510)), &base(), &c);
    assert_eq!((refs.len(), cut), (0, 510));
    let (refs, cut) = hreflang::extract(&Document::parse(&many(500)), &base(), &c);
    assert_eq!((refs.len(), cut), (500, 0));
}

// spec: TECH-070
#[test]
fn parse_html_reports_the_truncated_counter() {
    let c = cfg();
    let u = base();
    let body = many(503);
    let page = os_parse::parse_html(&os_parse::ParseInput {
        status: 200,
        url: &u,
        body: body.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &c,
    });
    assert_eq!(page.hreflang.len(), 500);
    assert_eq!(page.hreflang_truncated, 3);
}
