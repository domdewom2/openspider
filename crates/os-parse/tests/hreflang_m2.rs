use os_model::CrawlConfig;
use os_parse::{dom::Document, hreflang, ParseConfig};

// spec: TECH-086
// spec: TECH-008 [M2]
#[test]
fn outside_head_annotations_survive_with_validation_disabled() {
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/tech-m2-hreflang/outside.html"
    ));
    let base = "https://site.test/".parse().unwrap();
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let (refs, _) = hreflang::extract(&doc, &base, &cfg);
    assert_eq!(
        refs.iter()
            .map(|r| (r.code_raw.as_str(), r.in_head))
            .collect::<Vec<_>>(),
        [
            ("en", true),
            ("de", true),
            ("fr", false),
            ("x-default", false)
        ]
    );
    cfg.html_validation = false;
    let (off, _) = hreflang::extract(&doc, &base, &cfg);
    assert_eq!(refs, off);
}
