//! Bildinstanzen (CRAWL-052 „Bilder“, ONPAGE-170) gegen `crawl-resources/sources.html` und die Startseite.

use os_model::{ImageInstance, ImageSource};
use os_parse::dom::Document;
use os_parse::{base, images};

fn instances(file: &str) -> Vec<ImageInstance> {
    let path = os_testsite::fixtures_root()
        .join("crawl-resources")
        .join(file);
    let html = std::fs::read_to_string(path).unwrap();
    let doc = Document::parse(&html);
    let url = url::Url::parse(&format!("https://site.test/{file}")).unwrap();
    let base_url = base::resolve_base(&doc, &url);
    images::extract(&doc, &base_url)
}

fn find<'a>(all: &'a [ImageInstance], src: &str, source: ImageSource) -> &'a ImageInstance {
    all.iter()
        .find(|i| i.src_raw == src && i.source == source)
        .unwrap_or_else(|| panic!("keine Instanz {src} ({source:?})"))
}

// spec: CRAWL-052
#[test]
fn golden_instance_list_of_the_sources_page() {
    use ImageSource::*;
    let actual: Vec<(ImageSource, String)> = instances("sources.html")
        .into_iter()
        .map(|i| (i.source, i.src_raw))
        .collect();
    let expected: Vec<(ImageSource, &str)> = vec![
        (Img, "img/a.png"),
        (Img, "img/b.png"),
        (Img, "img/c.png"),
        (Img, "img/d.png"),
        (Img, "img/e.png"),
        (Img, "img/f.png"),
        (Img, "img/g.png"),
        (Img, "img/h.png"),
        // srcset: nur als Instanz mit Quelle Srcset
        (Srcset, "img/s.png"),
        (Srcset, "img/l.png"),
        (Img, "img/m.png"),
        (Srcset, "img/m-480.png"),
        (Srcset, "img/m-800.png"),
        (Srcset, "img/m.png?x=1,2"), // Komma innerhalb der URL trennt nicht
        // <picture>: erst <source>, dann <img>
        (Srcset, "img/p.webp"),
        (Img, "img/p.jpg"),
        (Srcset, "img/q.webp"),
        // data:-URI, data-src, leeres src, Kommentar und <template> erzeugen nichts; <noscript> ist Markup
        (A, "img/big.jpg"),
        (A, "img/vector.SVG?v=2#x"),
        (InputImage, "img/button.png"),
        (Poster, "img/poster.png"),
        (Img, "img/in-noscript.png"),
        (Img, "img/upper.png"),
    ];
    let expected: Vec<(ImageSource, String)> = expected
        .into_iter()
        .map(|(s, u)| (s, u.to_string()))
        .collect();
    assert_eq!(actual, expected);
}

// spec: CRAWL-052
#[test]
fn alt_information_per_instance() {
    let all = instances("sources.html");
    let a = find(&all, "img/a.png", ImageSource::Img);
    assert!(a.alt_present);
    assert_eq!(a.alt.as_deref(), Some("A"));
    assert_eq!(a.alt_len, Some(1));
    // alt="" und `alt` ohne Wert: vorhanden, leer
    for src in ["img/b.png", "img/c.png"] {
        let i = find(&all, src, ImageSource::Img);
        assert!(i.alt_present, "{src}");
        assert_eq!(i.alt.as_deref(), Some(""), "{src}");
        assert_eq!(i.alt_len, Some(0), "{src}");
    }
    // kein alt-Attribut
    let d = find(&all, "img/d.png", ImageSource::Img);
    assert!(!d.alt_present);
    assert_eq!(d.alt, None);
    assert_eq!(d.alt_len, None);
    // normalisiert (N(x)), Länge in Codepoints, Entities dekodiert
    let e = find(&all, "img/e.png", ImageSource::Img);
    assert_eq!(e.alt.as_deref(), Some("E mit Leerraum"));
    assert_eq!(e.alt_len, Some(14));
    let h = find(&all, "img/h.png", ImageSource::Img);
    assert_eq!(h.alt.as_deref(), Some("Kleinbild & Ko"));
    // Großschreibung
    assert_eq!(
        find(&all, "img/upper.png", ImageSource::Img).alt.as_deref(),
        Some("Gross")
    );
}

// spec: CRAWL-052
#[test]
fn image_links_input_and_poster_carry_no_alt_information() {
    let all = instances("sources.html");
    for (src, source) in [
        ("img/big.jpg", ImageSource::A),
        ("img/vector.SVG?v=2#x", ImageSource::A),
        ("img/poster.png", ImageSource::Poster),
        ("img/button.png", ImageSource::InputImage),
    ] {
        let i = find(&all, src, source);
        assert!(!i.alt_present, "{src}");
        assert_eq!(i.alt, None, "{src}");
    }
    // Größenattribute des input type=image bleiben erhalten
    let input = find(&all, "img/button.png", ImageSource::InputImage);
    assert_eq!(input.width_attr.as_deref(), Some("10"));
    assert_eq!(input.height_attr.as_deref(), Some("10"));
}

// spec: CRAWL-052
#[test]
fn width_and_height_attributes_are_kept_raw() {
    let all = instances("sources.html");
    let get = |src: &str| {
        let i = find(&all, src, ImageSource::Img);
        (i.width_attr.clone(), i.height_attr.clone())
    };
    assert_eq!(get("img/a.png"), (None, None));
    assert_eq!(get("img/e.png"), (Some("400".into()), Some("300".into())));
    assert_eq!(get("img/f.png"), (Some("400".into()), None));
    assert_eq!(get("img/g.png"), (Some(String::new()), Some(String::new())));
    // CSS-Größen (style=…) zählen nicht als Attribut
    assert_eq!(get("img/h.png"), (None, None));
}

// spec: CRAWL-052
#[test]
fn srcset_instances_inherit_alt_from_their_img() {
    let all = instances("sources.html");
    let s = find(&all, "img/s.png", ImageSource::Srcset);
    assert!(s.alt_present);
    assert_eq!(s.alt.as_deref(), Some("Srcset"));
    // <picture><source> nimmt das alt des <img> im selben <picture>
    let webp = find(&all, "img/p.webp", ImageSource::Srcset);
    assert_eq!(webp.alt.as_deref(), Some("P"));
    // <picture> ohne <img>: keine Alt-Information
    let q = find(&all, "img/q.webp", ImageSource::Srcset);
    assert!(!q.alt_present);
    // XPath zeigt auf das <source>
    assert_eq!(q.xpath, "/html[1]/body[1]/picture[2]/source[1]");
}

// spec: CRAWL-052
#[test]
fn resolved_urls_and_xpaths() {
    let all = instances("sources.html");
    let a = find(&all, "img/a.png", ImageSource::Img);
    assert_eq!(a.resolved.as_deref(), Some("https://site.test/img/a.png"));
    assert_eq!(a.xpath, "/html[1]/body[1]/img[1]");
    let orders: Vec<u32> = all.iter().map(|i| i.doc_order).collect();
    assert!(orders.windows(2).all(|w| w[0] <= w[1]), "{orders:?}");
    // Query mit Komma bleibt in der aufgelösten URL erhalten
    assert_eq!(
        find(&all, "img/m.png?x=1,2", ImageSource::Srcset)
            .resolved
            .as_deref(),
        Some("https://site.test/img/m.png?x=1,2")
    );
}

// spec: CRAWL-052
#[test]
fn startpage_instances_follow_the_crawl_resources_fixture() {
    use ImageSource::*;
    let all = instances("index.html");
    let actual: Vec<(ImageSource, &str)> =
        all.iter().map(|i| (i.source, i.src_raw.as_str())).collect();
    assert_eq!(
        actual,
        [
            (Img, "img/pic.png"),
            (A, "foto.jpg"),
            (InputImage, "img/send.png"),
            // <img data-src> ohne Rendering nicht erkannt; das style=-Hintergrundbild ist ein CSS-Link, keine Instanz
            (Img, "https://cdn.other.test/x.png"),
        ]
    );
}
