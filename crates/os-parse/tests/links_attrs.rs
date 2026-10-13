//! TECH-131: Link-Attribute (Follow aus `rel`, Anchor, Alt-Text) gegen `crawl-link-sources/attrs.html`.

use os_model::{CrawlConfig, ExtractedLink, LinkType};
use os_parse::dom::Document;
use os_parse::{links, ParseConfig};

fn hyperlinks() -> Vec<ExtractedLink> {
    let path = os_testsite::fixtures_root().join("crawl-link-sources/attrs.html");
    let html = std::fs::read_to_string(&path).unwrap();
    let doc = Document::parse(&html);
    let base = url::Url::parse("https://site.test/attrs.html").unwrap();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    links::extract(&doc, &base, &cfg)
        .into_iter()
        .filter(|l| l.link_type == LinkType::Hyperlink)
        .collect()
}

fn find<'a>(links: &'a [ExtractedLink], href: &str) -> &'a ExtractedLink {
    links
        .iter()
        .find(|l| l.href_raw == href)
        .unwrap_or_else(|| panic!("kein Link {href}"))
}

// spec: TECH-131
#[test]
fn nofollow_ugc_sponsored_set_the_rel_flag_noopener_does_not() {
    let links = hyperlinks();
    // (href, rel, nofollow_rel)
    let expected = [
        ("/plain", None, false),
        ("/nofollow", Some("nofollow"), true),
        ("/ugc", Some("ugc"), true),
        ("/sponsored", Some("sponsored"), true),
        ("/noopener", Some("noopener noreferrer"), false),
        // Groß-/Kleinschreibung und Mehrfach-Leerraum: klein, Originalreihenfolge, ein Leerzeichen
        ("/mixed-rel", Some("noopener nofollow external"), true),
    ];
    for (href, rel, flag) in expected {
        let l = find(&links, href);
        assert_eq!(l.rel.as_deref(), rel, "{href}");
        assert_eq!(l.nofollow_rel, flag, "{href}");
    }
}

// spec: TECH-131
#[test]
fn anchor_text_is_the_normalised_text_content() {
    let links = hyperlinks();
    assert_eq!(find(&links, "/plain").anchor.as_deref(), Some("Normal"));
    // verschachtelte Elemente, Entities, geschütztes Leerzeichen -> N(x)
    assert_eq!(
        find(&links, "/nested-text").anchor.as_deref(),
        Some("Ein verschachtelter Text& mehr")
    );
    // leer bzw. nur Leerraum -> kein Anchor
    assert_eq!(find(&links, "/empty-text").anchor, None);
    assert_eq!(find(&links, "/spaces-only").anchor, None);
}

// spec: TECH-131
#[test]
fn image_alt_replaces_a_missing_anchor_only() {
    let links = hyperlinks();
    // leerer Text + Bild mit alt -> Alt Text (normalisiert)
    let l = find(&links, "/img-alt");
    assert_eq!(l.anchor, None);
    assert_eq!(l.alt_text.as_deref(), Some("Firmen Logo"));
    // Bild ohne alt oder mit leerem alt -> weder Anchor noch Alt Text
    for href in ["/img-no-alt", "/img-empty-alt"] {
        let l = find(&links, href);
        assert_eq!(l.anchor, None, "{href}");
        assert_eq!(l.alt_text, None, "{href}");
    }
    // vorhandener Text bleibt Anchor, das alt wird nicht übernommen
    let l = find(&links, "/text-and-img");
    assert_eq!(l.anchor.as_deref(), Some("Text"));
    assert_eq!(l.alt_text, None);
}

// spec: TECH-131
#[test]
fn aria_label_and_title_are_not_anchors() {
    let links = hyperlinks();
    let l = find(&links, "/aria");
    assert_eq!(l.anchor, None);
    assert_eq!(l.alt_text, None);
}

// spec: TECH-131
#[test]
fn target_attribute_is_kept() {
    let links = hyperlinks();
    assert_eq!(find(&links, "/blank").target.as_deref(), Some("_blank"));
    assert_eq!(find(&links, "/plain").target, None);
}
