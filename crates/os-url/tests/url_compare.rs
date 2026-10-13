//! Tests zu TECH-002 (URL-Vergleich und Auflösung), Fixture `tech/url-compare`.

use os_url::{compare_key, eq_for_compare, resolve};
use url::Url;

// spec: TECH-002
#[test]
fn equality_normalizes_case_port_fragment() {
    assert!(eq_for_compare("https://A.test:443/x#y", "https://a.test/x"));
    assert!(eq_for_compare("HTTP://a.test:80/x", "http://a.test/x"));
    assert!(eq_for_compare("https://a.test/x%7e", "https://a.test/x~"));
    assert!(eq_for_compare(
        "https://a.test/a%2fb",
        "https://a.test/a%2Fb"
    ));
    assert!(eq_for_compare(
        "https://a.test/caf%c3%a9",
        "https://a.test/café"
    ));
    assert!(eq_for_compare(
        "https://müller.test/",
        "https://xn--mller-kva.test/"
    ));
}

// spec: TECH-002
#[test]
fn equality_keeps_path_case_trailing_slash_scheme_and_query_order() {
    assert!(!eq_for_compare("https://a.test/x", "https://a.test/x/"));
    assert!(!eq_for_compare("http://a.test/x", "https://a.test/x"));
    assert!(!eq_for_compare("https://a.test/X", "https://a.test/x"));
    assert!(!eq_for_compare(
        "https://a.test/x?a=1&b=2",
        "https://a.test/x?b=2&a=1"
    ));
    assert!(!eq_for_compare("https://a.test/x?", "https://a.test/x"));
    // reservierte Zeichen werden nicht dekodiert
    assert!(!eq_for_compare(
        "https://a.test/a%2Fb",
        "https://a.test/a/b"
    ));
}

// spec: TECH-002
#[test]
fn invalid_urls_never_match() {
    assert_eq!(compare_key("http://[bad"), None);
    assert!(!eq_for_compare("http://[bad", "http://[bad"));
    assert!(!eq_for_compare("http://[bad", "https://a.test/"));
    assert!(!eq_for_compare("", ""));
    assert!(!eq_for_compare("/relativ", "/relativ"));
    assert!(compare_key("https://a.test/x#frag").is_some());
}

// spec: TECH-002
#[test]
fn compare_key_is_the_normalized_encoded_form() {
    assert_eq!(
        compare_key("  https://A.test:443/x?b=2&a=1#y ").as_deref(),
        Some("https://a.test/x?b=2&a=1")
    );
}

// spec: TECH-002
#[test]
fn resolve_follows_whatwg_against_base() {
    let doc = Url::parse("https://site.test/dir/sub/page.html").unwrap();
    let r = |h: &str| resolve(&doc, h).map(|u| u.to_string());
    assert_eq!(
        r("a.html").as_deref(),
        Some("https://site.test/dir/sub/a.html")
    );
    assert_eq!(r("../b/").as_deref(), Some("https://site.test/dir/b/"));
    assert_eq!(r("./c").as_deref(), Some("https://site.test/dir/sub/c"));
    assert_eq!(r("/d").as_deref(), Some("https://site.test/d"));
    assert_eq!(
        r("//cdn.test/x.js").as_deref(),
        Some("https://cdn.test/x.js")
    );
    assert_eq!(
        r("?p=2").as_deref(),
        Some("https://site.test/dir/sub/page.html?p=2")
    );
    assert_eq!(r("../../../../x").as_deref(), Some("https://site.test/x"));
    // Leerer oder nur aus Leerraum bestehender href = Dokument-URL (Festlegung, Anhang A Punkt 2)
    assert_eq!(
        r("").as_deref(),
        Some("https://site.test/dir/sub/page.html")
    );
    assert_eq!(
        r("   ").as_deref(),
        Some("https://site.test/dir/sub/page.html")
    );
    assert_eq!(r(" /spaces ").as_deref(), Some("https://site.test/spaces"));
    assert_eq!(
        r("a\n.html").as_deref(),
        Some("https://site.test/dir/sub/a.html")
    );
    assert_eq!(r("http://[bad"), None);
}

// spec: TECH-002
#[test]
fn resolve_against_base_href_and_compare_with_document_url() {
    let doc = Url::parse("https://site.test/dir/page.html").unwrap();
    let base = resolve(&doc, "/other/").unwrap();
    let canonical = resolve(&base, "page.html").unwrap();
    assert_eq!(canonical.as_str(), "https://site.test/other/page.html");
    assert!(!eq_for_compare(canonical.as_str(), doc.as_str()));
    // Leerer Canonical-href → eigene URL (Self Referencing)
    let own = resolve(&doc, "").unwrap();
    assert!(eq_for_compare(own.as_str(), doc.as_str()));
    // Canonical mit Großbuchstaben-Host, Port und Fragment gilt als gleich
    let c = resolve(&doc, "HTTPS://SITE.TEST:443/dir/page.html#top").unwrap();
    assert!(eq_for_compare(c.as_str(), doc.as_str()));
}
