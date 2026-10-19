use os_model::CrawlConfig;
use os_parse::{alternates, dom::Document, ParseConfig};

// spec: CRAWL-051
#[test]
fn mobile_catalog_accepts_only_head_width_or_handheld_alternates_without_hreflang_or_type() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let base = url::Url::parse("https://site.test/dir/").unwrap();
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/crawl-m2-switches/crawl-resources/index.html"
    ));
    let links = alternates::extract(&doc, &base, &cfg);
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].resolved.as_deref(),
        Some("https://site.test/mobile")
    );
    let doc = Document::parse("<head><link rel=alternate media=handheld href=mobile><link rel=alternate media='(max-device-width: 800px)' href=/device><link rel=alternate media=handheld hreflang=en href=/lang><link rel=alternate media=handheld type=text/html href=/type></head><body><link rel=alternate media=handheld href=/body>");
    assert_eq!(alternates::extract(&doc, &base, &cfg).len(), 2);
    let links = alternates::from_headers(&base,&[("Link".into(),"<m>; rel=alternate; media=handheld, </lang>; rel=alternate; media=handheld; hreflang=en".into())],&cfg);
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].resolved.as_deref(),
        Some("https://site.test/dir/m")
    );
}
