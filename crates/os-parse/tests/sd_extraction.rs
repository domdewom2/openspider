use os_model::{SdItem, UrlId};
use os_parse::{parse_html, ParseConfig, ParseInput};
use serde_json::Value;
const JSONLD: &str = include_str!("../../../tests/fixtures/tech-m2-sd/jsonld.html");
const MICRO: &str = include_str!("../../../tests/fixtures/tech-m2-sd/microdata.html");
const RDFA: &str = include_str!("../../../tests/fixtures/tech-m2-sd/rdfa.html");
const ERRORS: &str = include_str!("../../../tests/fixtures/tech-m2-sd/errors.html");
fn parse(html: &str, formats: [bool; 3]) -> Vec<SdItem> {
    let mut cfg = ParseConfig::from_crawl_config(&os_model::CrawlConfig::default());
    cfg.structured_data.json_ld = formats[0];
    cfg.structured_data.microdata = formats[1];
    cfg.structured_data.rdfa = formats[2];
    let url = "https://site.test/page".parse().unwrap();
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    })
    .structured_data
}
fn json(item: &SdItem) -> Value {
    serde_json::from_str(&item.json).unwrap()
}
// spec: TECH-180
#[test]
fn jsonld_shapes_contexts_references_and_source_order() {
    let items = parse(JSONLD, [true, false, false]);
    assert_eq!(items.len(), 5);
    assert!(items.iter().all(|i| i.parse_error.is_none()));
    let product = json(&items[0]);
    assert_eq!(product["@type"][0], "https://schema.org/Product");
    assert_eq!(product["offers"]["@type"], "https://schema.org/Offer");
    assert_eq!(
        product["_os_order"],
        serde_json::json!(["@context", "@type", "name", "offers"])
    );
    assert_eq!(product["_os_source"]["block"], 1);
    assert_eq!(items[0].type_path, "Product|Thing");
    assert_eq!(json(&items[2])["publisher"]["name"], "O");
    assert_eq!(items[3].type_path, "Product");
    assert_eq!(items[4].type_path, "Event");
    let custom = parse(
        r#"<script type="application/ld+json">{"@context":{"@vocab":"https://vocab.test/"},"@type":"Custom"}</script>"#,
        [true, false, false],
    );
    assert_eq!(json(&custom[0])["@type"], "https://vocab.test/Custom");
    assert_eq!(json(&custom[0])["_os_types"], serde_json::json!(["Custom"]));
}
// spec: TECH-180
#[test]
fn references_across_blocks_and_cycles_are_bounded() {
    let html = r##"<script type="application/ld+json">{"@id":"#a","@type":"Thing","related":{"@id":"#b"}}</script><script type="application/ld+json">{"@id":"#b","@type":"Thing","name":"B","related":{"@id":"#a"}}</script>"##;
    let items = parse(html, [true, false, false]);
    assert_eq!(items.len(), 2);
    assert_eq!(json(&items[0])["related"]["name"], "B");
    assert!(items[0].json.len() < 4000);
}
// spec: TECH-181
#[test]
fn microdata_values_nested_items_and_itemref() {
    let items = parse(MICRO, [false, true, false]);
    assert_eq!(items.len(), 1);
    let v = json(&items[0]);
    assert_eq!(v["@type"].as_array().unwrap().len(), 2);
    for (key, value) in [
        ("name", "Soup"),
        ("datePublished", "2026-10-18"),
        ("image", "https://site.test/soup.png"),
        ("url", "https://site.test/soup"),
        ("subjectOf", "https://site.test/menu"),
        ("recipeYield", "2"),
        ("ratingValue", "5"),
        ("recipeCuisine", "Local"),
    ] {
        assert_eq!(v[key], value, "{key}");
    }
    assert_eq!(v["nutrition"]["calories"], "5 kcal");
    assert_eq!(v["@id"], "https://site.test/recipe");
    assert_eq!(v["related"]["recipeCuisine"], "Local");
}
// spec: TECH-182
#[test]
fn rdfa_vocab_prefix_nested_items_and_other_vocabularies() {
    let items = parse(RDFA, [false, false, true]);
    assert_eq!(items.len(), 3);
    assert_eq!(
        items
            .iter()
            .map(|i| i.type_path.as_str())
            .collect::<Vec<_>>(),
        ["Organization", "Product", "Event"]
    );
    assert_eq!(json(&items[0])["@id"], "https://site.test/org");
    assert_eq!(json(&items[1])["url"], "https://site.test/product");
    assert_eq!(
        json(&items[2])["location"]["@type"][0],
        "https://schema.org/Place"
    );
}
// spec: TECH-186
#[test]
fn errors_keep_positions_snippets_and_do_not_stop_extraction() {
    let items = parse(ERRORS, [true, false, false]);
    assert_eq!(items.len(), 5);
    assert_eq!(items.iter().filter(|i| i.parse_error.is_some()).count(), 4);
    let error = json(&items[0]);
    assert_eq!(error["_os_parse_error"]["line"], 2);
    assert!(error["_os_parse_error"]["column"].as_u64().unwrap() > 1);
    assert!(error["_os_parse_error"]["snippet"]
        .as_str()
        .unwrap()
        .contains("💚"));
    assert_eq!(json(&items[4])["name"], "still extracted");
    let (writes, issues) = os_parse::sd::to_writes(UrlId(1), &items);
    assert_eq!(writes.len(), 5);
    assert_eq!(issues.len(), 4);
    assert_eq!(issues[2].item, Some(2));
    for html in [
        "<div itemscope itemtype='relative'></div>",
        "<div typeof='Unresolved'></div>",
    ] {
        let items = parse(html, [false, true, true]);
        assert_eq!(items.len(), 1);
        assert!(items[0].parse_error.is_some());
    }
}
// spec: TECH-184
// spec: TECH-185
#[test]
fn formats_are_opt_in_and_ordered() {
    let html = include_str!("../../../tests/fixtures/tech-m2-sd/mixed.html");
    assert!(parse(html, [false, false, false]).is_empty());
    assert_eq!(
        parse(html, [true, true, true])
            .iter()
            .map(|i| i.format.as_str())
            .collect::<Vec<_>>(),
        ["json-ld", "microdata", "rdfa"]
    );
    assert!(parse(MICRO, [true, false, false]).is_empty());
    assert!(parse(
        include_str!("../../../tests/fixtures/tech-m2-sd/none.html"),
        [true, true, true]
    )
    .is_empty());
}

// spec: TECH-180
// spec: TECH-181
// spec: TECH-182
// spec: TECH-189
#[test]
fn extraction_golden_retains_full_trees_and_source_metadata() {
    let actual: Vec<_> = [("jsonld",JSONLD),("microdata",MICRO),("rdfa",RDFA),("errors",ERRORS)].into_iter().map(|(name,html)| {
        serde_json::json!({"fixture":name,"items":parse(html,[true,true,true]).iter().map(|i|serde_json::json!({"format":i.format,"type_path":i.type_path,"item":json(i),"parse_error":i.parse_error})).collect::<Vec<_>>()})
    }).collect();
    let text = serde_json::to_string_pretty(&actual).unwrap() + "\n";
    os_testsite::golden::assert_golden(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/tech-m2-sd/extraction.json"),
        &text,
    );
}
