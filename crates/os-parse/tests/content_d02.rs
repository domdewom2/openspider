use os_model::{CrawlConfig, Value};
use os_parse::{ParseConfig, ParseInput};
fn parse(html: &str) -> os_model::ParsedPage {
    let url = url::Url::parse("https://site.test/").unwrap();
    os_parse::parse_html(&ParseInput {
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        status: 200,
        cfg: &ParseConfig::from_crawl_config(&CrawlConfig::default()),
    })
}
// spec: ONPAGE-147
#[test]
fn soft404_uses_title_h1_and_area_not_footer() {
    let page = parse(include_str!(
        "../../../tests/fixtures/onpage-m2-content/area.html"
    ));
    let text = page
        .ext
        .iter()
        .find(|(k, _)| k.as_str() == "content.soft404_text")
        .unwrap();
    assert!(
        matches!(&text.1,Value::Text(t) if t.contains("seite nicht gefunden") && !t.contains("ipsum"))
    );
}
// spec: ONPAGE-148
#[test]
fn lorem_in_footer_counts_but_comments_and_scripts_do_not() {
    for (html, expected) in [
        (
            include_str!("../../../tests/fixtures/onpage-m2-content/area.html"),
            true,
        ),
        (
            "<body><!--Lorem ipsum--><script>Lorem ipsum</script>lorem</body>",
            false,
        ),
        ("<meta name='description' content='LOREM   IPSUM'>", true),
    ] {
        assert_eq!(
            parse(html)
                .ext
                .iter()
                .find(|(k, _)| k.as_str() == "content.lorem")
                .unwrap()
                .1,
            Value::Bool(expected)
        );
    }
}
