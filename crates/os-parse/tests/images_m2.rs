use os_model::{CrawlConfig, FetchPurpose, ImageSource, LinkPosition, LinkType, Value};
use os_parse::{dom::Document, images, ParseConfig, ResponseInput};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        os_testsite::fixtures_root()
            .join("onpage-m2-images/parse")
            .join(name),
    )
    .unwrap()
}
fn extract(cfg: &CrawlConfig) -> (Vec<os_model::ImageInstance>, Vec<os_model::ExtractedLink>) {
    let doc = Document::parse(std::str::from_utf8(&fixture("sources.html")).unwrap());
    images::extract_with(
        &doc,
        &url::Url::parse("https://site.test/").unwrap(),
        &ParseConfig::from_crawl_config(cfg),
    )
}
// spec: CRAWL-055
#[test]
fn srcset_discovers_each_candidate_only_when_enabled() {
    let mut cfg = CrawlConfig::default();
    let (instances, links) = extract(&cfg);
    assert!(links.is_empty());
    let candidates: Vec<_> = instances
        .iter()
        .filter(|i| i.source == ImageSource::Srcset)
        .collect();
    assert_eq!(candidates.len(), 7);
    cfg.extraction.srcset = true;
    let (enabled, links) = extract(&cfg);
    assert_eq!(enabled, instances);
    assert_eq!(links.len(), 7);
    assert!(links
        .iter()
        .all(|l| l.link_type == LinkType::Image && l.resolved.is_some()));
    assert!(links.iter().any(|l| l.href_raw == "query.png?x=1,2"));
    assert_eq!(links.iter().filter(|l| l.href_raw == "same.png").count(), 2);
    assert_eq!(
        links
            .iter()
            .find(|l| l.href_raw == "p.webp")
            .unwrap()
            .alt_text
            .as_deref(),
        Some("P")
    );
}
// spec: ONPAGE-172
#[test]
fn instance_metadata_preserves_paths_positions_origins_and_dimensions() {
    let (instances, _) = extract(&CrawlConfig::default());
    let find = |s: &str| instances.iter().find(|i| i.src_raw == s).unwrap();
    let a = find("/a.png");
    assert_eq!(a.xpath, "/html[1]/body[1]/main[1]/div[2]/img[1]");
    assert_eq!(a.width_attr.as_deref(), Some("400"));
    assert_eq!(a.height_attr.as_deref(), Some("300"));
    assert_eq!(a.link_position, Some(LinkPosition::Content));
    assert_eq!(a.origin.as_str(), "HTML");
    assert!(a.render.is_none());
    assert_eq!(
        find("//cdn.test/nav.png").link_position,
        Some(LinkPosition::Navigation)
    );
    assert_eq!(
        find("https://cdn.test/footer.png").link_position,
        Some(LinkPosition::Footer)
    );
}
// spec: ONPAGE-178
#[test]
fn inline_css_extension_adds_instances_without_duplicating_existing_css_links() {
    let mut cfg = CrawlConfig::default();
    assert!(!extract(&cfg).0.iter().any(|i| i.source == ImageSource::Css));
    cfg.images.parse_inline_css_backgrounds = true;
    let (instances, links) = extract(&cfg);
    assert!(links.is_empty());
    let css: Vec<_> = instances
        .iter()
        .filter(|i| i.source == ImageSource::Css)
        .map(|i| i.src_raw.as_str())
        .collect();
    assert_eq!(css, ["/hero.png", "/bg.png", "content.png"]);
    assert!(!instances
        .iter()
        .any(|i| ["lazy.png", "icon.png", "og.png", "font.woff"].contains(&i.src_raw.as_str())));
}
fn facts(body: &[u8], enabled: bool, status: u16) -> Vec<(os_model::ExtKey, Value)> {
    let mut cfg = CrawlConfig::default();
    cfg.images.real_dimensions_from_header = enabled;
    let parse = ParseConfig::from_crawl_config(&cfg);
    os_parse::response_ext(&ResponseInput {
        url: &url::Url::parse("https://cdn.test/image").unwrap(),
        internal: false,
        purpose: FetchPurpose::External,
        status,
        headers: &[],
        body,
        content_length: None,
        untruncated_bytes: None,
        truncated: false,
        cfg: &parse,
    })
}
// spec: ONPAGE-172
#[test]
fn header_extension_reads_five_formats_and_primary_avif_item() {
    for name in [
        "header.png",
        "header.gif",
        "header.jpg",
        "extended.webp",
        "lossless.webp",
        "lossy.webp",
        "primary.avif",
    ] {
        let body = fixture(name);
        assert!(facts(&body, false, 200).is_empty(), "{name}");
        let got = facts(&body, true, 200);
        assert_eq!(
            got.iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>(),
            [
                ("image.real_w", Value::Int(800)),
                ("image.real_h", Value::Int(600))
            ],
            "{name}"
        );
        assert!(facts(&body, true, 404).is_empty());
        for length in 0..body.len().min(64) {
            let _ = facts(&body[..length], true, 200);
        }
    }
    assert!(facts(b"<svg width='800' height='600'></svg>", true, 200).is_empty());
    assert!(facts(&[0; 32], true, 200).is_empty());
}
