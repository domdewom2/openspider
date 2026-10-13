//! CRAWL-052 (Link-Quellen), CRAWL-041 (JavaScript-Dateien) und Linkposition gegen die Fixture
//! `crawl-link-sources`.

use os_model::{CrawlConfig, ExtractedLink, LinkPosition, LinkType};
use os_parse::dom::Document;
use os_parse::{base, links, ParseConfig};

const URL: &str = "https://site.test/index.html";

fn extract(file: &str) -> Vec<ExtractedLink> {
    let full = os_testsite::fixtures_root()
        .join("crawl-link-sources")
        .join(file);
    let html = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()));
    let doc = Document::parse(&html);
    let url = url::Url::parse(URL).unwrap();
    let base_url = base::resolve_base(&doc, &url);
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    links::extract(&doc, &base_url, &cfg)
}

fn find<'a>(links: &'a [ExtractedLink], href: &str) -> &'a ExtractedLink {
    links
        .iter()
        .find(|l| l.href_raw == href)
        .unwrap_or_else(|| panic!("kein Link {href}"))
}

// spec: CRAWL-052
#[test]
fn golden_link_list_of_the_fixture_page() {
    use LinkType::*;
    // Golden-Liste aus CRAWL-052 abgeleitet: je Zeile der Quellenliste ein Element, in Dokumentreihenfolge.
    let expected: Vec<(LinkType, &str)> = vec![
        // <head>
        (Css, "/css/main.css"),
        (Other, "/fonts/f.woff2"),   // preload as=font -> Sonstige
        (Css, "/css/pre.css"),       // preload as=style
        (JavaScript, "/js/pre.js"),  // prefetch as=script
        (Other, "/next-page.html"),  // prefetch ohne as -> Sonstige
        (JavaScript, "/js/mod.mjs"), // modulepreload
        (JavaScript, "/js/app.js"),
        (JavaScript, "/js/module.mjs"),
        // <body>: Hyperlinks (auch in Groß-/Mischschreibung) und area
        (Hyperlink, "/header-link"),
        (Hyperlink, "/nav-link"),
        (Hyperlink, "/hyperlink"),
        (Hyperlink, "/UPPER"),
        (Hyperlink, "/mixed"),
        (Hyperlink, "/area"),
        // Bilder (srcset und data: erzeugen keinen Link)
        (Image, "/img/pic.png"),
        (Image, "/img/upper.png"),
        (Image, "/img/with-srcset.png"),
        (Image, "/img/p.jpg"),
        (Image, "/img/btn.png"),
        (Image, "/img/poster.png"),
        // Medien
        (Media, "/media/v.mp4"),
        (Media, "/media/v.webm"),
        (Media, "/media/v.vtt"),
        (Media, "/media/a.mp3"),
        // Frames, Plugins
        (Iframe, "/frame.html"),
        (Other, "/flash/x.swf"),
        (Other, "/flash/y.swf"),
        // verlinktes Bild: Hyperlink UND eingebettetes Bild
        (Hyperlink, "/bild-link.jpg"),
        (Image, "/img/linked.png"),
        (Hyperlink, "/rel-sponsored"),
        (Hyperlink, "/rel-ugc"),
        (Hyperlink, "/rel-nofollow"),
        // Kommentar, <template> und <script>-Inhalt fehlen; <noscript> ist im Textmodus Markup
        (Hyperlink, "/in-noscript"),
        (Hyperlink, "/aside-link"),
        (Hyperlink, "/footer-link"),
        // kaputtes Markup (nicht geschlossene p/a/b/div) wird wie im Browser repariert
        (Hyperlink, "/broken-1"),
        (Hyperlink, "/broken-2"),
        (Hyperlink, "/broken-3"),
    ];
    let links = extract("index.html");
    let actual: Vec<(LinkType, &str)> = links
        .iter()
        .map(|l| (l.link_type, l.href_raw.as_str()))
        .collect();
    assert_eq!(actual, expected);

    // Die aufgelösten Ziele sind absolut gegen die Dokument-URL.
    assert_eq!(
        find(&links, "/hyperlink").resolved.as_deref(),
        Some("https://site.test/hyperlink")
    );
    // Dokumentreihenfolge
    let orders: Vec<u32> = links.iter().map(|l| l.doc_order).collect();
    assert!(orders.windows(2).all(|w| w[0] <= w[1]), "{orders:?}");
}

// spec: CRAWL-052
#[test]
fn excluded_sources_are_not_extracted() {
    let links = extract("index.html");
    for gone in [
        "/in-comment",
        "/in-template",
        "/in-template.png",
        "/in-script",
        "/img/s1.png",
        "/img/s2.png",
        "/img/p.webp",
        "/img/lazy.png",
        "/index.html",
        "/favicon.ico",
    ] {
        assert!(
            !links.iter().any(|l| l.href_raw == gone),
            "{gone} darf nicht extrahiert werden"
        );
    }
    // data:-URI als src und data-src liefern nichts
    assert!(!links
        .iter()
        .any(|l| l.href_raw.starts_with("data:") || l.href_raw.contains("lazy")));
}

// spec: CRAWL-052
#[test]
fn rel_attribute_is_normalised_and_drives_nofollow() {
    let links = extract("index.html");
    let sponsored = find(&links, "/rel-sponsored");
    assert_eq!(sponsored.rel.as_deref(), Some("sponsored"));
    assert!(sponsored.nofollow_rel);
    let ugc = find(&links, "/rel-ugc");
    assert_eq!(ugc.rel.as_deref(), Some("ugc"));
    assert!(ugc.nofollow_rel);
    let both = find(&links, "/rel-nofollow");
    assert_eq!(both.rel.as_deref(), Some("nofollow noopener"));
    assert!(both.nofollow_rel);
    // `Rel="NoFollow"` in Mischschreibung -> klein
    let mixed = find(&links, "/mixed");
    assert_eq!(mixed.rel.as_deref(), Some("nofollow"));
    assert!(mixed.nofollow_rel);
    // ohne rel
    let plain = find(&links, "/hyperlink");
    assert_eq!(plain.rel, None);
    assert!(!plain.nofollow_rel);
}

// spec: CRAWL-052
#[test]
fn link_position_follows_the_nearest_landmark_ancestor() {
    let links = extract("index.html");
    let pos = |href: &str| find(&links, href).position;
    assert_eq!(pos("/header-link"), LinkPosition::Header);
    assert_eq!(pos("/nav-link"), LinkPosition::Navigation);
    assert_eq!(pos("/hyperlink"), LinkPosition::Content);
    assert_eq!(pos("/img/pic.png"), LinkPosition::Content);
    assert_eq!(pos("/aside-link"), LinkPosition::Aside);
    assert_eq!(pos("/footer-link"), LinkPosition::Footer);
    // Ressourcen im Head: kein Landmark -> Content, aber in_head
    let css = find(&links, "/css/main.css");
    assert_eq!(css.position, LinkPosition::Content);
    assert!(css.in_head);
    assert!(!find(&links, "/hyperlink").in_head);
}

// spec: CRAWL-052
#[test]
fn xpath_is_absolute_with_position_indices() {
    let links = extract("index.html");
    assert_eq!(
        find(&links, "/header-link").xpath,
        "/html[1]/body[1]/header[1]/a[1]"
    );
    assert_eq!(
        find(&links, "/nav-link").xpath,
        "/html[1]/body[1]/nav[1]/a[1]"
    );
    assert_eq!(
        find(&links, "/hyperlink").xpath,
        "/html[1]/body[1]/main[1]/p[1]/a[1]"
    );
    assert_eq!(
        find(&links, "/UPPER").xpath,
        "/html[1]/body[1]/main[1]/p[1]/a[2]"
    );
    assert_eq!(
        find(&links, "/css/main.css").xpath,
        "/html[1]/head[1]/link[1]"
    );
}

// spec: CRAWL-052
#[test]
fn anchor_text_and_alt_text() {
    let links = extract("index.html");
    let a = find(&links, "/hyperlink");
    assert_eq!(a.anchor.as_deref(), Some("Hyperlink"));
    assert_eq!(a.alt_text, None);
    // area: kein Text, alt als Alt-Text
    let area = find(&links, "/area");
    assert_eq!(area.anchor, None);
    assert_eq!(area.alt_text.as_deref(), Some("Bereich"));
    // Bildlink ohne Text: alt des Bildes in Alt-Text
    let image_link = find(&links, "/bild-link.jpg");
    assert_eq!(image_link.anchor, None);
    assert_eq!(image_link.alt_text.as_deref(), Some("Verlinktes Bild"));
    // das eingebettete Bild trägt sein alt ebenfalls
    assert_eq!(
        find(&links, "/img/linked.png").alt_text.as_deref(),
        Some("Verlinktes Bild")
    );
    assert_eq!(find(&links, "/img/upper.png").alt_text, None);
}

// spec: CRAWL-052
#[test]
fn broken_markup_is_repaired_like_a_browser() {
    let links = extract("index.html");
    // Das zweite <a> beendet das erste (HTML5-Adoption-Agency); jedes Element erscheint genau einmal.
    assert_eq!(
        find(&links, "/broken-1").anchor.as_deref(),
        Some("nicht geschlossen fett")
    );
    assert_eq!(
        find(&links, "/broken-2").anchor.as_deref(),
        Some("verschachtelt")
    );
    assert_eq!(find(&links, "/broken-3").anchor.as_deref(), Some("im div"));
    assert_eq!(
        links
            .iter()
            .filter(|l| l.href_raw.starts_with("/broken-"))
            .count(),
        3
    );
}

// spec: CRAWL-041
#[test]
fn javascript_sources_script_module_and_modulepreload() {
    let links = extract("index.html");
    let js: Vec<&str> = links
        .iter()
        .filter(|l| l.link_type == LinkType::JavaScript)
        .map(|l| l.href_raw.as_str())
        .collect();
    // <script src>, <script type="module" src>, <link rel="modulepreload">, preload/prefetch as=script
    assert_eq!(
        js,
        ["/js/pre.js", "/js/mod.mjs", "/js/app.js", "/js/module.mjs"]
    );
    let module = find(&links, "/js/module.mjs");
    assert_eq!(
        module.resolved.as_deref(),
        Some("https://site.test/js/module.mjs")
    );
    assert!(module.in_head);
}
