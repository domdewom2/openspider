//! Kennzahlen: Folder Depth (Annahme A-8) und Länge der Address in Codepoints.

use os_url::{folder_depth, normalize, url_length};
use url::Url;

fn depth(u: &str) -> u32 {
    folder_depth(&Url::parse(u).unwrap())
}

#[test]
fn folder_depth_follows_assumption_a8() {
    assert_eq!(depth("https://site.test/"), 0);
    assert_eq!(depth("https://site.test"), 0);
    assert_eq!(depth("https://site.test/a/"), 1);
    assert_eq!(depth("https://site.test/a/b.html"), 1);
    assert_eq!(depth("https://site.test/a/b/"), 2);
    assert_eq!(depth("https://site.test/a/b"), 1);
    assert_eq!(depth("https://site.test/a"), 0);
    // Query und Fragment zählen nicht
    assert_eq!(depth("https://site.test/a/?x=1"), 1);
    assert_eq!(depth("https://site.test/a?x=/b/c/"), 0);
    assert_eq!(depth("https://site.test/a/#x/y/"), 1);
    // `//` bleibt erhalten und zählt
    assert_eq!(depth("https://site.test/a//b"), 2);
}

#[test]
fn url_length_counts_codepoints_of_the_address() {
    assert_eq!(url_length("a/ü"), 3);
    assert_eq!(url_length(""), 0);
    let n = normalize(&Url::parse("https://site.test/café").unwrap());
    assert_eq!(
        url_length(&n.display),
        "https://site.test/café".chars().count() as u32
    );
    assert_eq!(url_length(&n.display), 22);
    assert_eq!(url_length(&n.encoded), 27);
    assert_eq!(url_length("😀"), 1);
}
