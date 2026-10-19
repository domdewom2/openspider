//! TECH-030/031/035: eigener Fixture-Baum und Header, unabhängig von anderen Fachpaketen.
use os_model::{CrawlConfig, ExtKey, FetchPurpose, HintSource, Value};
use os_parse::{pagination, parse_html, ParseConfig, ParseInput, ResponseInput};

const HTML: &[u8] = include_bytes!("../../../tests/fixtures/tech-m2-pagination/extraction.html");
fn headers() -> Vec<(String, String)> {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/tech-m2-pagination/headers.json"
    ))
    .unwrap()
}
fn parse(body: &[u8], store: bool) -> os_model::ParsedPage {
    let mut cfg = CrawlConfig::default();
    cfg.spider.pagination.store = store;
    let cfg = ParseConfig::from_crawl_config(&cfg);
    parse_html(&ParseInput {
        status: 200,
        url: &url::Url::parse("https://site.test/start").unwrap(),
        body,
        content_type: Some("text/html"),
        headers: &headers(),
        cfg: &cfg,
    })
}
fn fact(page: &os_model::ParsedPage, key: &str) -> Value {
    page.ext
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .unwrap()
        .1
        .clone()
}

// spec: TECH-030
#[test]
fn html_and_headers_keep_direction_source_order_and_duplicate_occurrences() {
    let page = parse(HTML, true);
    assert_eq!(page.pagination.len(), 6);
    assert_eq!(
        page.pagination.iter().map(|p| p.order).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        page.pagination
            .iter()
            .map(|p| p.rel_next)
            .collect::<Vec<_>>(),
        [true, false, true, false, true, false]
    );
    assert!(page.pagination[..4]
        .iter()
        .all(|p| p.source == HintSource::Html && p.in_head));
    assert!(page.pagination[4..]
        .iter()
        .all(|p| p.source == HintSource::Http));
    assert_eq!(page.pagination[0].raw, "two");
    assert_eq!(
        page.pagination[0].resolved.as_deref(),
        Some("https://site.test/series/two")
    );
}

// spec: TECH-035
#[test]
fn same_page_anchor_comparison_accepts_nofollow_and_normalizes_urls() {
    assert_eq!(
        fact(&parse(HTML, true), "pagination.not_in_anchor"),
        Value::Bool(false)
    );
    let missing = include_bytes!("../../../tests/fixtures/tech-m2-pagination/missing.html");
    assert_eq!(
        fact(&parse(missing, true), "pagination.not_in_anchor"),
        Value::Bool(true)
    );
}

// spec: TECH-031
#[test]
fn has_prev_survives_the_store_switch_in_html_and_header_only_responses() {
    for store in [true, false] {
        let page = parse(HTML, store);
        assert_eq!(fact(&page, "pagination.has_prev"), Value::Bool(true));
        // HTML only: previous is accepted even without a Link header.
        let doc = os_parse::dom::Document::parse(std::str::from_utf8(HTML).unwrap());
        let refs = pagination::extract(&doc, &url::Url::parse("https://site.test/").unwrap());
        assert!(refs.iter().any(|p| !p.rel_next));
        let mut cfg = CrawlConfig::default();
        cfg.spider.pagination.store = store;
        let cfg = ParseConfig::from_crawl_config(&cfg);
        let url = url::Url::parse("https://site.test/document.pdf").unwrap();
        let h = headers();
        let mut input = ResponseInput {
            url: &url,
            internal: true,
            purpose: FetchPurpose::Page,
            status: 301,
            headers: &h,
            body: &[],
            content_length: None,
            untruncated_bytes: None,
            truncated: false,
            cfg: &cfg,
        };
        assert_eq!(
            pagination::response_facts(&input),
            [(
                ExtKey::from_static("pagination.has_prev"),
                Value::Bool(true)
            )]
        );
        input.headers = &[];
        assert_eq!(pagination::response_facts(&input)[0].1, Value::Bool(false));
    }
}

// spec: TECH-035
#[test]
fn header_only_pagination_without_anchors_has_one_authoritative_fact() {
    let body = include_bytes!("../../../tests/fixtures/tech-m2-pagination/header-only.html");
    for store in [true, false] {
        let page = parse(body, store);
        assert_eq!(fact(&page, "pagination.not_in_anchor"), Value::Bool(true));
        assert_eq!(page.pagination.len(), 2);
        assert!(page.pagination.iter().all(|p| p.source == HintSource::Http));
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        let url = url::Url::parse("https://site.test/start").unwrap();
        let h = headers();
        let response = pagination::response_facts(&ResponseInput {
            url: &url,
            internal: true,
            purpose: FetchPurpose::Page,
            status: 200,
            headers: &h,
            body,
            content_length: None,
            untruncated_bytes: None,
            truncated: false,
            cfg: &cfg,
        });
        // The writer sees exactly one value in either hook order, also when the anchor exists.
        for parsed in [page, parse(HTML, store)] {
            for facts in [
                response.iter().chain(&parsed.ext).collect::<Vec<_>>(),
                parsed.ext.iter().chain(&response).collect::<Vec<_>>(),
            ] {
                let matching: Vec<_> = facts
                    .into_iter()
                    .filter(|(k, _)| k.as_str() == "pagination.not_in_anchor")
                    .collect();
                assert_eq!(matching.len(), 1);
                assert_eq!(matching[0].1, fact(&parsed, "pagination.not_in_anchor"));
            }
        }
    }
}
