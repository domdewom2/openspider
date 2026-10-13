//! ONPAGE-003: Textnormalisierung N(x) und Zeichenlänge.

use os_parse::dom::{text_content, Document};
use os_parse::text::{char_len, normalize, normalize_ws};

// spec: ONPAGE-003
#[test]
fn collapses_whitespace_and_trims() {
    let n = normalize("  Hallo\n\t Welt  ");
    assert_eq!(n, "Hallo Welt");
    assert_eq!(char_len(&n), 10);
}

// spec: ONPAGE-003
#[test]
fn decodes_named_entities() {
    let n = normalize("A&amp;B");
    assert_eq!(n, "A&B");
    assert_eq!(char_len(&n), 3);
    assert_eq!(
        normalize("&lt;b&gt; &quot;x&quot; &copy; &auml;"),
        "<b> \"x\" © ä"
    );
}

// spec: ONPAGE-003
#[test]
fn nbsp_counts_as_whitespace() {
    assert_eq!(normalize("&nbsp;Test&nbsp;"), "Test");
    assert_eq!(normalize("\u{00A0}Test\u{00A0}"), "Test");
    assert_eq!(normalize("a&nbsp;&nbsp; \n b"), "a b");
    // weitere Unicode-White_Space-Zeichen
    assert_eq!(normalize("a\u{2003}b\u{3000}c\u{0085}d"), "a b c d");
}

// spec: ONPAGE-003
#[test]
fn length_counts_codepoints_not_bytes() {
    let n = normalize("Über");
    assert_eq!(n.len(), 5);
    assert_eq!(char_len(&n), 4);
    // Astralzeichen = ein Codepoint (A-2)
    assert_eq!(char_len(&normalize("a😀b")), 3);
}

// spec: ONPAGE-003
#[test]
fn empty_and_whitespace_only_values() {
    for s in ["", " ", "\n\t \r\n", "&nbsp;", "&nbsp; &nbsp;", "\u{00A0}"] {
        let n = normalize(s);
        assert_eq!(n, "", "{s:?}");
        assert_eq!(char_len(&n), 0);
    }
}

// spec: ONPAGE-003
#[test]
fn invalid_entities_are_kept() {
    assert_eq!(normalize("&foo;"), "&foo;");
    assert_eq!(normalize("Tom &foo; Jerry"), "Tom &foo; Jerry");
    assert_eq!(normalize("a & b"), "a & b");
    assert_eq!(normalize("100%&"), "100%&");
}

// spec: ONPAGE-003
#[test]
fn numeric_entities_and_case_are_handled() {
    assert_eq!(normalize("&#65;&#x42;&#X43;"), "ABC");
    assert_eq!(normalize("&#8364;"), "€");
    // Groß-/Kleinschreibung bleibt (A-1), NFC wird nicht erzwungen
    assert_eq!(normalize("  ÄbC "), "ÄbC");
    let decomposed = "A\u{0308}";
    assert_eq!(normalize(decomposed), decomposed);
}

// spec: ONPAGE-003
#[test]
fn markup_like_text_is_not_interpreted() {
    // `<` bleibt Text; es entsteht kein Element und nichts geht verloren.
    assert_eq!(normalize("a <b> c & d"), "a <b> c & d");
    assert_eq!(normalize("x<script>y</script>"), "x<script>y</script>");
}

// spec: ONPAGE-003
#[test]
fn title_example_from_fixture_spec() {
    // onpage-titles/entities.html: <title>  Shop &amp; Mehr&nbsp; </title>
    let n = normalize("  Shop &amp; Mehr&nbsp; ");
    assert_eq!(n, "Shop & Mehr");
    assert_eq!(char_len(&n), 11);
}

// spec: ONPAGE-003
#[test]
fn normalize_ws_skips_entity_decoding_for_dom_text() {
    // Rohstring: Entities werden dekodiert, DOM-Text ist bereits dekodiert.
    assert_eq!(normalize_ws("  A&amp;B\u{00A0}\n c "), "A&amp;B c");
    assert_eq!(normalize_ws("\u{2003}a\tb\u{3000}"), "a b");
    assert_eq!(normalize_ws(""), "");
    assert_eq!(normalize_ws(" \n\t "), "");
    // ohne Entities identisch mit normalize
    for s in [
        "  Hallo\n\t Welt  ",
        "a\u{2003}b\u{3000}c\u{0085}d",
        "Über",
        "",
    ] {
        assert_eq!(normalize_ws(s), normalize(s), "{s:?}");
    }
}

// spec: ONPAGE-003
#[test]
fn dom_text_is_not_decoded_twice() {
    let doc =
        Document::parse("<body><p id=p title=\"x&amp;amp;y\"> a &amp;amp; b &lt;i&gt; </p></body>");
    let sel = scraper::Selector::parse("#p").unwrap();
    let p = doc.html().select(&sel).next().unwrap();
    // Quelltext `&amp;amp;` ist im DOM `&amp;` und bleibt nach normalize_ws genau so.
    assert_eq!(normalize_ws(&text_content(p, true)), "a &amp; b <i>");
    assert_eq!(normalize_ws(p.value().attr("title").unwrap()), "x&amp;y");
    // normalize würde ein zweites Mal dekodieren.
    assert_eq!(normalize(&text_content(p, true)), "a & b <i>");
}
