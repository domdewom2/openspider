//! CRAWL-041 und CRAWL-052 gegen die Fixture `crawl-resources`: Ressourcenlinks der Startseite.

use os_model::{CrawlConfig, ExtractedLink, LinkType};
use os_parse::dom::Document;
use os_parse::{base, css, links, ParseConfig};

fn page() -> (Document, url::Url) {
    let path = os_testsite::fixtures_root().join("crawl-resources/index.html");
    let html = std::fs::read_to_string(&path).unwrap();
    let doc = Document::parse(&html);
    let url = url::Url::parse("https://site.test/index.html").unwrap();
    let base_url = base::resolve_base(&doc, &url);
    (doc, base_url)
}

fn all_links() -> Vec<ExtractedLink> {
    let (doc, base_url) = page();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    links::extract(&doc, &base_url, &cfg)
}

// spec: CRAWL-041
#[test]
fn script_module_and_modulepreload_are_javascript_links() {
    let links = all_links();
    let js: Vec<(&str, Option<&str>)> = links
        .iter()
        .filter(|l| l.link_type == LinkType::JavaScript)
        .map(|l| (l.href_raw.as_str(), l.resolved.as_deref()))
        .collect();
    assert_eq!(
        js,
        [
            ("module.mjs", Some("https://site.test/module.mjs")), // modulepreload
            ("app.js", Some("https://site.test/app.js")),
            ("module.mjs", Some("https://site.test/module.mjs")), // type=module
        ]
    );
}

// spec: CRAWL-041
#[test]
fn javascript_files_are_not_searched_for_urls_in_text_mode() {
    // `app.js` enthält `import("lazy.js")` und einen Bildpfad: parse_css/extract_sheet ist nur für CSS da;
    // die HTML-Seite verweist weder auf lazy.js noch auf den Pfad im Skript.
    let links = all_links();
    assert!(!links.iter().any(|l| l.href_raw.contains("lazy")));
    assert!(!links.iter().any(|l| l.href_raw.contains("not-a-link")));
}

// spec: CRAWL-052
#[test]
fn page_resources_in_document_order() {
    use LinkType::*;
    let links = all_links();
    let actual: Vec<(LinkType, &str)> = links
        .iter()
        .map(|l| (l.link_type, l.href_raw.as_str()))
        .collect();
    assert_eq!(
        actual,
        [
            (Css, "style.css"),
            (JavaScript, "module.mjs"),
            (JavaScript, "app.js"),
            (JavaScript, "module.mjs"),
            (Image, "img/pic.png"),
            (Hyperlink, "foto.jpg"), // verlinktes Bild ist ein Hyperlink, kein eingebettetes Bild
            (Image, "img/send.png"), // input type=image
            // <img data-src> wird im Textmodus nicht erkannt
            (Image, "https://cdn.other.test/x.png"),
            (Hyperlink, "sources.html"),
        ]
    );
    let external = links
        .iter()
        .find(|l| l.href_raw.starts_with("https://cdn.other.test"))
        .unwrap();
    assert_eq!(
        external.resolved.as_deref(),
        Some("https://cdn.other.test/x.png")
    );
}

// spec: CRAWL-052
#[test]
fn css_in_style_blocks_and_attributes_comes_from_extract_inline() {
    let (doc, base_url) = page();
    let inline = css::extract_inline(&doc, &base_url, true);
    let actual: Vec<(LinkType, &str, Option<&str>)> = inline
        .iter()
        .map(|l| (l.link_type, l.href_raw.as_str(), l.resolved.as_deref()))
        .collect();
    assert_eq!(
        actual,
        [
            (
                LinkType::Css,
                "inline-import.css",
                Some("https://site.test/inline-import.css")
            ),
            (
                LinkType::Image,
                "img/hero.png",
                Some("https://site.test/img/hero.png")
            ),
            (
                LinkType::Image,
                "img/logo.png",
                Some("https://site.test/img/logo.png")
            ),
            (
                LinkType::Image,
                "img/bg.png",
                Some("https://site.test/img/bg.png")
            ),
        ]
    );
    assert_eq!(inline[0].xpath, "//head/style[1]");
    assert_eq!(inline[0].position, os_model::LinkPosition::Head);
    assert!(inline[0].in_head);
    assert_eq!(inline[3].xpath, "/html[1]/body[1]/div[1]");
    assert!(!inline[3].in_head);
}
