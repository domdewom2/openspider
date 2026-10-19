use os_parse::{directives, dom::Document};

// spec: TECH-063
#[test]
fn unavailable_after_parses_iso_rfc822_rfc1123_and_rfc850_without_clock() {
    let expected = "2020-12-31T23:59:59.000Z";
    for date in [
        "2020-12-31T23:59:59Z",
        "Thu, 31 Dec 2020 23:59:59 GMT",
        "Thu, 31 Dec 20 23:59:59 +0000",
        "Thursday, 31-Dec-20 23:59:59 GMT",
    ] {
        assert_eq!(
            directives::parse_unavailable_after(date).unwrap().to_db(),
            expected
        );
    }
    let fixture = include_str!("../../../tests/fixtures/tech-m2-directives/unavailable-past.html");
    let parsed = directives::extract(&Document::parse(fixture), &["googlebot".into()]);
    assert_eq!(
        directives::parse_unavailable_after(&parsed[0].tokens[0])
            .unwrap()
            .to_db(),
        expected
    );
    let iso = directives::extract(
        &Document::parse(include_str!(
            "../../../tests/fixtures/tech-m2-directives/unavailable.html"
        )),
        &["googlebot".into()],
    );
    assert!(directives::parse_unavailable_after(&iso[0].tokens[0]).is_some());
    assert!(directives::parse_unavailable_after("nonsense").is_none());
    assert!(directives::parse_unavailable_after("Thu, 31 Feb 2020 25:00:00 GMT").is_none());
    assert!(directives::parse_unavailable_after("2030-06-25").is_some());
}

// spec: TECH-065
#[test]
fn html_tree_marks_robots_after_img_as_body_even_with_head_source_spelling() {
    let html = include_str!("../../../tests/fixtures/tech-m2-directives/after-img.html");
    let parsed = directives::extract(&Document::parse(html), &["googlebot".into()]);
    assert_eq!(parsed.len(), 1);
    assert!(!parsed[0].in_head);
    assert_eq!(parsed[0].tokens, ["noindex"]);
}
