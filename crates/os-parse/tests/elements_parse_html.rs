//! `parse_html` (P00b) ruft die P05-Module auf: Elemente, Wortzahl und Pixelbreiten landen in `ParsedPage`.

use os_model::CrawlConfig;
use os_parse::{parse_html, ParseConfig, ParseInput};

fn parse(html: &str, pixel: bool) -> os_model::ParsedPage {
    let url = url::Url::parse("https://site.test/seite").unwrap();
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.pixel = pixel;
    parse_html(&ParseInput {
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    })
}

const PAGE: &str = "<html><head><title>Schuhe kaufen</title>\
    <meta name=\"description\" content=\"Beschreibung der Seite\">\
    <meta name=\"keywords\" content=\"schuhe, kaufen\"></head>\
    <body><nav>Menü Start</nav><h1>Willkommen</h1><h2>Eins</h2><h2>Zwei</h2>\
    <p>eins zwei drei</p><footer>Impressum</footer></body></html>";

// spec: ONPAGE-008, ONPAGE-141, UI-038
#[test]
fn parse_html_fills_elements_word_count_and_pixels() {
    let page = parse(PAGE, true);
    assert_eq!(page.titles.len(), 1);
    assert_eq!(page.titles[0].text, "Schuhe kaufen");
    assert_eq!(page.meta_descriptions[0].text, "Beschreibung der Seite");
    assert_eq!(page.meta_keywords[0].text, "schuhe, kaufen");
    assert_eq!(page.h1[0].text, "Willkommen");
    assert_eq!(page.h2.len(), 2);
    // Willkommen Eins Zwei eins zwei drei (nav/footer ausgeschlossen)
    assert_eq!(page.word_count, Some(6));
    let ratio = page.text_ratio.unwrap();
    assert!(ratio > 0.0 && ratio <= 100.0, "{ratio}");
    // Titel 20 px, Meta-Description 14 px, jeweils Regular.
    assert_eq!(
        page.titles[0].px,
        Some(os_parse::pixel::width_px("Schuhe kaufen", 20.0, false))
    );
    assert_eq!(
        page.meta_descriptions[0].px,
        Some(os_parse::pixel::width_px(
            "Beschreibung der Seite",
            14.0,
            false
        ))
    );
    assert!(page.titles[0].px.unwrap() > 0);
    // Überschriften und Keywords tragen keine Pixelbreite.
    assert_eq!(page.h1[0].px, None);
    assert_eq!(page.meta_keywords[0].px, None);
}

// spec: UI-038
#[test]
fn parse_html_without_pixel_flag_leaves_px_empty() {
    let page = parse(PAGE, false);
    assert_eq!(page.titles[0].px, None);
    assert_eq!(page.meta_descriptions[0].px, None);
}

// spec: ONPAGE-013
#[test]
fn parse_html_text_ratio_uses_source_length_in_chars() {
    // Sichtbarer Body-Text „Grüße aus Köln“ = 14 Zeichen (Script ausgenommen). Der Quelltext hat 120 Zeichen
    // (bei 124 Bytes) und das unquotierte Attribut `class=intro`, das der Baum anders (mit Anführungszeichen)
    // serialisiert: nur die echte Quelltextlänge ergibt 14 / 120 × 100 = 11,7 % (die Baum-Serialisierung mit 122 Zeichen ergäbe 11,5 %).
    let html = "<html><head><title>Müller</title></head><body><p class=intro>Grüße aus Köln</p>\
                <script>var a = 1;</script></body></html>";
    assert_eq!(html.chars().count(), 120);
    let page = parse(html, false);
    assert_eq!(page.text_ratio, Some(11.7));
}
