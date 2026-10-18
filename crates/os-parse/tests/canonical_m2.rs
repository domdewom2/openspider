use os_model::{CrawlConfig, Value};
use os_parse::{canonical, dom::Document, ParseConfig, ResponseInput};

const HTML: &str = include_str!("../../../tests/fixtures/tech-m2-canonicals/attributes.html");
const HEADERS: &str = include_str!("../../../tests/fixtures/tech-m2-canonicals/headers.txt");

// spec: TECH-021
#[test]
fn invalid_attributes_include_empty_values_and_exclude_other_attributes() {
    let url = url::Url::parse("https://site.test/dir/page").unwrap();
    let refs = canonical::extract(&Document::parse(HTML), &url);
    assert_eq!(
        refs.iter().map(|c| c.invalid_attribute).collect::<Vec<_>>(),
        [true, true, true, true, false, false, false]
    );
    assert_eq!(canonical::annotation_facts(&refs)[0].1, Value::Bool(true));
    let headers = vec![("lInK".into(), HEADERS.trim().into())];
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let hints = os_parse::parse_headers(&url, &headers, &cfg);
    assert_eq!(
        hints
            .canonicals
            .iter()
            .map(|c| c.invalid_attribute)
            .collect::<Vec<_>>(),
        [true, true, false]
    );
    let mut input = ResponseInput {
        url: &url,
        internal: false,
        purpose: os_model::FetchPurpose::External,
        status: 201,
        headers: &headers,
        body: b"%PDF",
        content_length: None,
        untruncated_bytes: None,
        truncated: false,
        cfg: &cfg,
    };
    assert_eq!(canonical::response_facts(&input)[0].1, Value::Bool(true));
    let mut off_cfg = CrawlConfig::default();
    off_cfg.spider.canonicals.store = false;
    let off = ParseConfig::from_crawl_config(&off_cfg);
    input.cfg = &off;
    assert!(canonical::response_facts(&input).is_empty());
}

// spec: TECH-019
// spec: TECH-022
#[test]
fn relative_and_fragment_raw_values_survive_resolution() {
    let url = url::Url::parse("https://site.test/dir/page").unwrap();
    let refs = canonical::extract(&Document::parse(HTML), &url);
    assert_eq!(
        refs.iter().map(|c| c.raw.as_str()).collect::<Vec<_>>(),
        [
            "/relative",
            "https://site.test/a",
            "//site.test/a",
            "../a#",
            "?p=2",
            "custom+v1:a#top",
            "/outside#top"
        ]
    );
    assert_eq!(refs[3].resolved.as_deref(), Some("https://site.test/a#"));
    assert_eq!(
        refs[6].resolved.as_deref(),
        Some("https://site.test/outside#top")
    );
}

// spec: TECH-023
// spec: TECH-008 [P1]
#[test]
fn extraction_keeps_body_canonicals_independent_of_validation() {
    let url = url::Url::parse("https://site.test/page").unwrap();
    let mut cfg = CrawlConfig::default();
    for enabled in [false, true] {
        cfg.advanced.html_validation = enabled;
        let page = os_parse::parse_html(&os_parse::ParseInput {
            status: 200,
            url: &url,
            headers: &[],
            body: HTML.as_bytes(),
            content_type: Some("text/html"),
            cfg: &ParseConfig::from_crawl_config(&cfg),
        });
        assert_eq!(page.canonicals.len(), 7);
        assert!(!page.canonicals[6].in_head);
    }
}
