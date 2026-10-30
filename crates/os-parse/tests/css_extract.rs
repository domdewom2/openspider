//! CRAWL-040: `url()` und `@import` in CSS-Dateien (`crawl-resources/style.css`).

use os_model::{ExtractedLink, LinkType};
use os_parse::css;

fn sheet() -> Vec<ExtractedLink> {
    let path = os_testsite::fixtures_root().join("crawl-resources/style.css");
    let body = std::fs::read(path).unwrap();
    let base = url::Url::parse("https://site.test/style.css").unwrap();
    css::extract_sheet(&base, &body)
}

// spec: CRAWL-040
#[test]
fn imports_and_urls_of_the_fixture_stylesheet() {
    use LinkType::*;
    let links = sheet();
    let actual: Vec<(LinkType, &str, Option<&str>)> = links
        .iter()
        .map(|l| (l.link_type, l.href_raw.as_str(), l.resolved.as_deref()))
        .collect();
    assert_eq!(
        actual,
        [
            // @import url("…"), @import "…", @import url(…) mit Medienliste: relativ zur CSS-Datei
            (Css, "b.css", Some("https://site.test/b.css")),
            (Css, "c.css", Some("https://site.test/c.css")),
            (Css, "d.css", Some("https://site.test/d.css")),
            // @font-face -> Schriftarten (LinkType hat keinen eigenen Wert: Other)
            (
                Other,
                "fonts/fixture.woff2",
                Some("https://site.test/fonts/fixture.woff2")
            ),
            (
                Other,
                "fonts/fixture.woff",
                Some("https://site.test/fonts/fixture.woff")
            ),
            // url() ohne, mit einfachen und mit doppelten Anführungszeichen
            (
                Image,
                "img/hero.png",
                Some("https://site.test/img/hero.png")
            ),
            (
                Image,
                "img/logo.png",
                Some("https://site.test/img/logo.png")
            ),
            (
                Image,
                "img/icon.png",
                Some("https://site.test/img/icon.png")
            ),
            // data:-URI, url(#clip) und der auskommentierte Verweis fehlen; `..` über die Wurzel bleibt bei `/`
            (Image, "../up.png", Some("https://site.test/up.png")),
        ]
    );
}

// spec: CRAWL-040
#[test]
fn relative_urls_resolve_against_the_stylesheet_location() {
    let base = url::Url::parse("https://site.test/assets/css/site.css").unwrap();
    let css_text = b"@import 'sub/x.css'; a{background:url(../img/a.png)} b{background:url(/root.png)} c{background:url(//cdn.test/c.png)}";
    let links = css::extract_sheet(&base, css_text);
    let resolved: Vec<&str> = links
        .iter()
        .map(|l| l.resolved.as_deref().unwrap())
        .collect();
    assert_eq!(
        resolved,
        [
            "https://site.test/assets/css/sub/x.css",
            "https://site.test/assets/img/a.png",
            "https://site.test/root.png",
            "https://cdn.test/c.png",
        ]
    );
}

// spec: CRAWL-040
#[test]
fn data_uris_and_fragment_references_are_not_links() {
    let base = url::Url::parse("https://site.test/s.css").unwrap();
    let links = css::extract_sheet(
        &base,
        b"a{background:url(data:image/png;base64,AAAA)} b{background:url('DATA:image/gif;base64,AAAA')} c{fill:url(#g)} d{background:url()}",
    );
    assert!(links.is_empty(), "{links:?}");
}

// spec: CRAWL-040
#[test]
fn css_decoding_handles_bom_and_legacy_bytes() {
    let base = url::Url::parse("https://site.test/s.css").unwrap();
    // UTF-8-BOM vor @charset
    let mut with_bom = b"\xEF\xBB\xBF@import 'k.css';".to_vec();
    assert_eq!(css::extract_sheet(&base, &with_bom)[0].href_raw, "k.css");
    // Latin-1-Bytes in einem Kommentar stören nicht
    with_bom = b"/* \xE4\xF6\xFC */ a{background:url(x.png)}".to_vec();
    assert_eq!(css::extract_sheet(&base, &with_bom)[0].href_raw, "x.png");
}

// spec: CRAWL-040
#[test]
fn style_blocks_and_style_attributes_in_html_are_scanned_too() {
    use os_parse::dom::Document;
    let doc = Document::parse(
        "<html><head><style>@import 'i.css'; .a{background:url(\"h.png\")}</style></head>\
         <body><p style=\"background:url('p.png')\">x</p><svg><style>.s{fill:url(s.svg#f)}</style></svg>\
         <template><div style=\"background:url(no.png)\"></div></template></body></html>",
    );
    let base = url::Url::parse("https://site.test/dir/page.html").unwrap();
    let links = css::extract_inline(&doc, &base, true);
    let resolved: Vec<&str> = links
        .iter()
        .map(|l| l.resolved.as_deref().unwrap())
        .collect();
    assert_eq!(
        resolved,
        [
            "https://site.test/dir/i.css",
            "https://site.test/dir/h.png",
            "https://site.test/dir/p.png",
            "https://site.test/dir/s.svg#f",
        ]
    );
    assert_eq!(links[0].link_type, LinkType::Css);
    assert_eq!(links[1].link_type, LinkType::Image);
}

// spec: CRAWL-040
#[test]
fn parse_css_delegates_to_the_sheet_scanner() {
    let base = url::Url::parse("https://site.test/s.css").unwrap();
    let links = os_parse::parse_css(&base, b"@import 'a.css';");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].link_type, LinkType::Css);
}
