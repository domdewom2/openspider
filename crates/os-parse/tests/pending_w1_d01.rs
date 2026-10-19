use os_model::{CrawlConfig, Value};
use os_parse::{parse_html, ParseConfig, ParseInput};
// spec: TECH-008 [P1]
#[test]
#[ignore = "M2: wartet auf M2-D10"]
fn validation_marker_accompanies_outside_facts_in_real_parser() {
    let mut crawl = CrawlConfig::default();
    crawl.advanced.html_validation = true;
    let cfg = ParseConfig::from_crawl_config(&crawl);
    let url = url::Url::parse("https://site.test/").unwrap();
    let page = parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: include_bytes!("../../../tests/fixtures/onpage-m2-elements/facts.html"),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    for key in [
        "validation.checked",
        "head.title_outside",
        "head.description_outside",
    ] {
        assert!(
            page.ext
                .iter()
                .any(|(k, v)| k.as_str() == key && *v == Value::Bool(true)),
            "{key}"
        );
    }
}
