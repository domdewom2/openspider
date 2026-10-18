use os_model::{CrawlConfig, Value};
use os_parse::{parse_html, ParseConfig, ParseInput};

// spec: ONPAGE-079
#[test]
fn html5_bookmarks_are_exact_deduplicated_and_exclude_inert_content() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/page").unwrap();
    let html = include_bytes!("../../../tests/fixtures/onpage-m2-url/bookmarks.html");
    let page = parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: html,
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    });
    let value = &page
        .ext
        .iter()
        .find(|(key, _)| key.as_str() == "urlfilter.bookmark_targets")
        .unwrap()
        .1;
    let Value::Text(value) = value else {
        panic!("JSON text expected")
    };
    let actual: Vec<String> = serde_json::from_str(value).unwrap();
    assert_eq!(
        actual,
        [
            "a&b",
            "anker-alt",
            "café",
            "dupe",
            "id-and-name",
            "kontakt",
            "old-name"
        ]
    );
}

// spec: ONPAGE-079
#[test]
fn empty_html_records_known_empty_targets() {
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let url = url::Url::parse("https://site.test/page").unwrap();
    let page = parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: b"<p>No targets</p>",
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    assert!(page
        .ext
        .iter()
        .any(|(key, v)| key.as_str() == "urlfilter.bookmark_targets"
            && v == &Value::Text("[]".into())));
}
