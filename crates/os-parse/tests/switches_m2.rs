use os_model::*;
use os_parse::{switches, ParseConfig};

// spec: CRAWL-061
// spec: CRAWL-065
#[test]
fn switch_application_drops_only_disabled_facts_and_keeps_independent_directive_sources() {
    let mut cfg = CrawlConfig::default();
    cfg.extraction.forms = false;
    cfg.extraction.readability = false;
    cfg.extraction.word_count = false;
    cfg.extraction.text_ratio = false;
    cfg.extraction.page_titles = false;
    cfg.extraction.x_robots_tag = false;
    let cfg = ParseConfig::from_crawl_config(&cfg);
    let url = url::Url::parse("https://site.test/").unwrap();
    let mut page = os_parse::parse_html(&os_parse::ParseInput {
        status: 200,
        url: &url,
        body: include_bytes!(
            "../../../tests/fixtures/crawl-m2-switches/crawl-resources/index.html"
        ),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    page.forms.push(FormRef {
        idx: 0,
        action: Some("/submit".into()),
        action_resolved: Some("https://site.test/submit".into()),
        method: "post".into(),
    });
    page.ext
        .push((ExtKey::from_static("readability.flesch"), Value::Float(60.)));
    switches::apply(&mut page, &cfg);
    assert!(
        page.forms.is_empty()
            && page.word_count.is_none()
            && page.text_ratio.is_none()
            && page.titles.is_empty()
    );
    assert_eq!(page.h1.len(), 1);
    // Andere Pakete legen eigene ext-Fakten ab; hier zählt nur der abgeschaltete Namensraum.
    assert!(page.ext.iter().all(|(k, _)| k.namespace() != "readability"));
    let mut directives = os_parse::parse_headers(
        &url,
        &[("X-Robots-Tag".into(), "noindex,nofollow".into())],
        &cfg,
    )
    .directives;
    switches::filter_directives(&mut directives, &cfg);
    assert!(directives.is_empty());
}
