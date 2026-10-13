//! TECH-003: Head-Zugehörigkeit nach dem HTML5-Tree-Builder, plus XPath, Template und Textinhalt.

use os_parse::dom::{in_head, in_template, text_content, xpath, Document};
use scraper::{ElementRef, Selector};

fn find<'a>(doc: &'a Document, css: &str) -> ElementRef<'a> {
    let sel = Selector::parse(css).unwrap();
    doc.html()
        .select(&sel)
        .next()
        .unwrap_or_else(|| panic!("kein Treffer für {css}"))
}

fn canonical_in_head(src: &str) -> bool {
    let doc = Document::parse(src);
    in_head(find(&doc, "link[rel=canonical]"))
}

// spec: TECH-003
#[test]
fn canonical_in_head_and_in_body() {
    assert!(canonical_in_head(
        "<html><head><link rel=canonical href=/a></head><body></body></html>"
    ));
    assert!(!canonical_in_head(
        "<html><head></head><body><link rel=canonical href=/a></body></html>"
    ));
    // Dokument ohne explizites head/body
    assert!(canonical_in_head("<link rel=canonical href=/a><p>x</p>"));
    assert!(!canonical_in_head("<p>x</p><link rel=canonical href=/a>"));
}

// spec: TECH-003
#[test]
fn img_in_head_closes_head_implicitly() {
    let src = "<html><head><title>t</title><img src=x.png><link rel=canonical href=/a></head><body><p>x</p></body></html>";
    assert!(!canonical_in_head(src));
    // Das Element vor dem <img> liegt noch im Head.
    let doc = Document::parse(src);
    assert!(in_head(find(&doc, "title")));
    assert!(!in_head(find(&doc, "img")));
}

// spec: TECH-003
#[test]
fn div_and_text_in_head_also_close_it() {
    assert!(!canonical_in_head(
        "<html><head><div></div><link rel=canonical href=/a></head><body></body></html>"
    ));
    assert!(!canonical_in_head(
        "<html><head>Text<link rel=canonical href=/a></head><body></body></html>"
    ));
}

// spec: TECH-003
#[test]
fn noscript_with_allowed_content_does_not_close_head() {
    let src = "<html><head><noscript><link rel=stylesheet href=a.css><meta name=x content=y><style>p{}</style></noscript><link rel=canonical href=/a></head><body></body></html>";
    assert!(canonical_in_head(src));
    let doc = Document::parse(src);
    assert!(in_head(find(&doc, "noscript")));
    // Mit img im noscript (Scripting an) bleibt der Head ebenfalls offen: der Inhalt ist reiner Text.
    assert!(canonical_in_head(
        "<html><head><noscript><img src=x></noscript><link rel=canonical href=/a></head><body></body></html>"
    ));
}

// spec: TECH-003
#[test]
fn second_head_before_body_counts_as_head() {
    // zweites <head> direkt nach </head>, noch vor <body>
    assert!(canonical_in_head(
        "<html><head><title>t</title></head><head><link rel=canonical href=/a></head><body></body></html>"
    ));
    // zweites <head> mitten im ersten
    assert!(canonical_in_head(
        "<html><head><title>t</title><head><link rel=canonical href=/a></head><body></body></html>"
    ));
    // nach Body-Beginn: außerhalb
    assert!(!canonical_in_head(
        "<html><head></head><body><p>x</p><head><link rel=canonical href=/a></head></body></html>"
    ));
}

// spec: TECH-003
#[test]
fn title_meta_and_hreflang_use_the_same_rule() {
    let src = "<html><head><meta name=robots content=noindex><title>a</title></head><body><title>b</title><meta name=robots content=index><link rel=alternate hreflang=de href=/de></body></html>";
    let doc = Document::parse(src);
    let sel = Selector::parse("title").unwrap();
    let titles: Vec<_> = doc.html().select(&sel).map(in_head).collect();
    assert_eq!(titles, [true, false]);
    let sel = Selector::parse("meta[name=robots]").unwrap();
    let metas: Vec<_> = doc.html().select(&sel).map(in_head).collect();
    assert_eq!(metas, [true, false]);
    assert!(!in_head(find(&doc, "link[hreflang]")));
}

// spec: TECH-003
#[test]
fn head_element_itself_and_body_are_not_inside_head() {
    let doc = Document::parse("<html><head></head><body></body></html>");
    assert!(!in_head(find(&doc, "head")));
    assert!(!in_head(find(&doc, "body")));
    assert!(!in_head(find(&doc, "html")));
}

// spec: TECH-003
#[test]
fn template_content_is_flagged() {
    let src = "<html><head><template><link rel=canonical href=/t></template><link rel=canonical href=/real></head><body><template><a href=/x>x</a></template><a href=/y>y</a></body></html>";
    let doc = Document::parse(src);
    let sel = Selector::parse("link[rel=canonical]").unwrap();
    let flags: Vec<_> = doc.html().select(&sel).map(in_template).collect();
    assert_eq!(flags, [true, false]);
    let sel = Selector::parse("a").unwrap();
    let flags: Vec<_> = doc.html().select(&sel).map(in_template).collect();
    assert_eq!(flags, [true, false]);
    // das Element im Template im Head liegt (formal) im Head, wird aber über in_template ausgesondert
    assert!(in_head(find(&doc, "template")));
}

// spec: TECH-003
#[test]
fn base_href_is_first_base_with_href_outside_templates() {
    let doc = Document::parse(
        "<html><head><template><base href=/in-template/></template><base target=_blank><base href=\"  https://x.test/b/  \"><base href=/second/></head><body></body></html>",
    );
    assert_eq!(doc.base_href().as_deref(), Some("https://x.test/b/"));
    assert_eq!(Document::parse("<p>x</p>").base_href(), None);
    assert_eq!(
        Document::parse("<base href=\"\"><p>x</p>")
            .base_href()
            .as_deref(),
        Some("")
    );
}

// spec: OUT-031
#[test]
fn xpath_matches_spec_examples() {
    let doc = Document::parse("<body><main><div></div><div><img></div></main></body>");
    assert_eq!(xpath(find(&doc, "img")), "/html/body/main/div[2]/img");

    let doc = Document::parse(
        "<body><main><p>a</p><p>b <a href=1>x</a> <a href=2>y</a></p></main></body>",
    );
    let sel = Selector::parse("a").unwrap();
    let paths: Vec<_> = doc.html().select(&sel).map(xpath).collect();
    assert_eq!(
        paths,
        ["/html/body/main/p[2]/a[1]", "/html/body/main/p[2]/a[2]"]
    );
    assert_eq!(xpath(find(&doc, "html")), "/html");
}

// spec: OUT-031
#[test]
fn xpath_is_stable_and_counts_only_same_name_siblings() {
    let src = "<!doctype html><html><head><title>t</title></head><body><ul><li>1</li><li><a href=a>a</a></li><li>3</li></ul><span><a href=b>b</a></span></body></html>";
    let first: Vec<String> = {
        let doc = Document::parse(src);
        let sel = Selector::parse("a").unwrap();
        doc.html().select(&sel).map(xpath).collect()
    };
    let second: Vec<String> = {
        let doc = Document::parse(src);
        let sel = Selector::parse("a").unwrap();
        doc.html().select(&sel).map(xpath).collect()
    };
    assert_eq!(first, second);
    assert_eq!(first, ["/html/body/ul/li[2]/a", "/html/body/span/a"]);
    let doc = Document::parse(src);
    assert_eq!(xpath(find(&doc, "title")), "/html/head/title");
}

// spec: ONPAGE-140
#[test]
fn text_content_separates_blocks_and_skips_hidden_content() {
    let doc = Document::parse(
        "<body><div id=r><p>a</p><p>b</p><script>var x=1;</script><style>p{}</style><!-- c --><span>c</span><span>d</span><br>e<svg><text>s</text></svg><noscript>n</noscript></div></body>",
    );
    let root = find(&doc, "#r");
    assert_eq!(
        os_parse::text::normalize(&text_content(root, true)),
        "a b cd e"
    );
    // ohne Blockgrenzen werden Texte unmittelbar aneinandergehängt
    assert_eq!(text_content(root, false), "abcde");
    let p = find(&doc, "p");
    assert_eq!(text_content(p, true).trim(), "a");
    // Inline-Elemente trennen nie
    let doc = Document::parse("<body><p>fo<b>o</b>bar</p></body>");
    assert_eq!(text_content(find(&doc, "p"), true).trim(), "foobar");
}
