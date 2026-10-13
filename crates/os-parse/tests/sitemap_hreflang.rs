//! TECH-070 / CRAWL-060: hreflang-Annotationen (`xhtml:link`) je `<url>` einer XML-Sitemap.

use os_model::{HintSource, SitemapKind};
use os_parse::parse_sitemap;

fn url() -> url::Url {
    url::Url::parse("https://site.test/sitemap-hreflang.xml").unwrap()
}

// spec: TECH-070
#[test]
fn annotations_belong_to_the_url_of_their_block() {
    let xml = r#"<?xml version="1.0"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
  <url>
    <loc>https://site.test/en/</loc>
    <xhtml:link rel="alternate" hreflang="en" href="https://site.test/en/"/>
    <xhtml:link rel="alternate" hreflang="es" href="https://site.test/es/"/>
    <xhtml:link rel="alternate" hreflang="it" href="/it/"/>
  </url>
  <url>
    <loc>https://site.test/es/</loc>
    <xhtml:link rel="alternate" hreflang="en" href="https://site.test/en/"/>
  </url>
</urlset>"#;
    let doc = parse_sitemap(&url(), xml.as_bytes(), None);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 2);
    let first = &doc.locs[0].hreflang;
    let codes: Vec<&str> = first.iter().map(|h| h.code_raw.as_str()).collect();
    assert_eq!(codes, ["en", "es", "it"]);
    assert!(first
        .iter()
        .all(|h| h.source == HintSource::Sitemap && h.code_valid));
    assert_eq!(first[1].resolved.as_deref(), Some("https://site.test/es/"));
    assert_eq!(first[2].href_raw, "/it/");
    assert_eq!(first[2].resolved.as_deref(), Some("https://site.test/it/"));
    let orders: Vec<u32> = first.iter().map(|h| h.order).collect();
    assert_eq!(orders, [0, 1, 2]);
    assert_eq!(doc.locs[1].hreflang.len(), 1);
}

// spec: TECH-070
#[test]
fn the_namespace_matters_not_the_prefix() {
    let xml = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:x="http://www.w3.org/1999/xhtml" xmlns:fake="http://example.test/not-xhtml">
  <url>
    <loc>https://site.test/en/</loc>
    <x:link rel="alternate" hreflang="de" href="https://site.test/de/"/>
    <fake:link rel="alternate" hreflang="fr" href="https://site.test/fr/"/>
    <unbound:link rel="alternate" hreflang="nl" href="https://site.test/nl/"/>
    <link rel="alternate" hreflang="pl" href="https://site.test/pl/"/>
  </url>
</urlset>"#;
    let doc = parse_sitemap(&url(), xml.as_bytes(), None);
    let codes: Vec<&str> = doc.locs[0]
        .hreflang
        .iter()
        .map(|h| h.code_raw.as_str())
        .collect();
    assert_eq!(codes, ["de"]);
}

// spec: TECH-070
#[test]
fn default_xhtml_namespace_on_the_link_element_works() {
    let xml = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://site.test/en/</loc>
    <link xmlns="http://www.w3.org/1999/xhtml" rel="alternate" hreflang="de" href="https://site.test/de/"/>
  </url></urlset>"#;
    let doc = parse_sitemap(&url(), xml.as_bytes(), None);
    assert_eq!(doc.locs[0].hreflang.len(), 1);
}

// spec: TECH-070
#[test]
fn only_alternate_with_a_code_and_href_count_codes_are_validated() {
    let xml = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
  <url><loc>https://site.test/en/</loc>
    <xhtml:link rel="canonical" hreflang="de" href="https://site.test/de/"/>
    <xhtml:link rel="alternate" href="https://site.test/ohne-code/"/>
    <xhtml:link rel="alternate" hreflang="" href="https://site.test/leer/"/>
    <xhtml:link rel="alternate" hreflang="de"/>
    <xhtml:link rel="alternate" hreflang="en-UK" href="https://site.test/uk/"/>
    <xhtml:link rel="alternate" hreflang="x-default" href="https://site.test/"/>
  </url></urlset>"#;
    let doc = parse_sitemap(&url(), xml.as_bytes(), None);
    let h = &doc.locs[0].hreflang;
    assert_eq!(h.len(), 2);
    assert!(!h[0].code_valid);
    assert_eq!(h[0].invalid_reason.as_deref(), Some("invalid_region"));
    assert_eq!(h[1].code_norm, "x-default");
    assert!(h[1].code_valid);
}

// spec: TECH-070
#[test]
fn annotation_limit_applies_per_url() {
    let mut xml = String::from(
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml"><url><loc>https://site.test/en/</loc>"#,
    );
    for i in 0..510 {
        xml.push_str(&format!(
            r#"<xhtml:link rel="alternate" hreflang="de" href="https://site.test/de/{i}"/>"#
        ));
    }
    xml.push_str("</url></urlset>");
    let doc = parse_sitemap(&url(), xml.as_bytes(), None);
    assert_eq!(doc.locs[0].hreflang.len(), 500);
}
