use os_model::{ContentAreaConfig, Value};
use os_parse::{
    content,
    dom::Document,
    readability::{facts, label, syllables},
};

// spec: ONPAGE-140, ONPAGE-141
#[test]
fn anchor_boundaries_and_list_items_share_the_content_area() {
    let doc = Document::parse(include_str!(
        "../../../tests/fixtures/n08-content/boundaries.html"
    ));
    let area = ContentAreaConfig::default();
    assert_eq!(content::word_count(&doc, &area), 13);
    let text = content::area_text_flat(&doc, &area);
    assert!(text.contains("click here read more River guide"));
    for excluded in ["Header", "Aside", "Menu", "Footer"] {
        assert!(!text.contains(excluded));
    }
}

// spec: ONPAGE-013
#[test]
fn text_ratio_preserves_three_decimal_places() {
    let doc = Document::parse("<body>abc</body>");
    assert_eq!(
        content::analyze_with_source(&doc, &Default::default(), 7).text_ratio,
        42.857
    );
}

// spec: ONPAGE-149
#[test]
fn readability_retains_three_decimal_places() {
    let values = facts("River garden");
    assert_eq!(values[1].1, Value::Float(2.0));
    assert_eq!(values[2].1, Value::Float(35.605));
    assert_eq!(syllables("linked"), 1);
    assert_eq!(syllables("wordae"), 1);
    assert_eq!(syllables("tree"), 1);
    assert_eq!(syllables("images"), 3);
    assert_eq!(syllables("kategorie"), 3);
    assert_eq!(syllables("variante"), 3);
    assert_eq!(
        facts(&format!("{}ééé", "a ".repeat(38)))[0].1,
        Value::Int(1)
    );
}

// spec: ONPAGE-150, ONPAGE-151
#[test]
fn labels_match_the_data_at_filter_boundaries() {
    for (score, expected) in [
        (0.0, "Very Hard"),
        (29.999, "Very Hard"),
        (30.0, "Hard"),
        (49.999, "Hard"),
        (50.0, "Fairly Hard"),
        (60.0, "Normal"),
        (70.0, "Fairly Easy"),
        (80.0, "Easy"),
        (90.0, "Very Easy"),
    ] {
        assert_eq!(label(score), expected);
    }
}

// spec: ONPAGE-149
#[test]
fn calibrated_counts_on_long_paragraphs_and_lists() {
    let url = url::Url::parse("https://site.test/").unwrap();
    for (name, html) in [
        (
            "services",
            include_str!("../../../tests/fixtures/out-basic-site/services.html"),
        ),
        (
            "about",
            include_str!("../../../tests/fixtures/out-basic-site/about.html"),
        ),
        (
            "root",
            include_str!("../../../tests/fixtures/parity-m2-onpage/index.html"),
        ),
        (
            "near-base",
            include_str!("../../../tests/fixtures/parity-m2-onpage/near-base.html"),
        ),
        (
            "near-95",
            include_str!("../../../tests/fixtures/parity-m2-onpage/near-95.html"),
        ),
        (
            "near-85",
            include_str!("../../../tests/fixtures/parity-m2-onpage/near-85.html"),
        ),
        (
            "images",
            include_str!("../../../tests/fixtures/parity-m2-onpage/images.html"),
        ),
        (
            "list",
            include_str!("../../../tests/fixtures/parity-m2-onpage/list-item-text.html"),
        ),
        (
            "images",
            include_str!("../../../tests/fixtures/parity-m2-onpage/images.html"),
        ),
        (
            "difficult",
            include_str!("../../../tests/fixtures/parity-m2-onpage/readability-difficult.html"),
        ),
    ] {
        let page = os_parse::parse_html(&os_parse::ParseInput {
            url: &url,
            body: html.as_bytes(),
            content_type: Some("text/html"),
            headers: &[],
            status: 200,
            cfg: &os_parse::ParseConfig::from_crawl_config(&Default::default()),
        });
        let (words, sentences, average, score) = match name {
            "services" => (8, 3, 2.667, 45.503),
            "about" => (9, 3, 3.0, 81.590),
            "root" => (110, 111, 0.991, 36.629),
            "near-base" => (302, 28, 10.786, 77.672),
            "near-95" => (287, 27, 10.630, 76.957),
            "near-85" => (257, 24, 10.708, 75.156),
            "list" => (10, 3, 3.333, 68.092),
            "images" => (3, 2, 1.5, 36.113),
            "difficult" => (20, 2, 10.0, 78.245),
            _ => unreachable!(),
        };
        assert_eq!(page.word_count, Some(words), "{name}");
        for (key, expected) in [
            ("readability.sentences", Value::Int(sentences)),
            ("readability.average", Value::Float(average)),
            ("readability.flesch", Value::Float(score)),
        ] {
            assert_eq!(
                page.ext.iter().find(|(k, _)| k.as_str() == key).unwrap().1,
                expected,
                "{name}: {key}"
            );
        }
    }
}
