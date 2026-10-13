//! Onpage-Elemente: Titel, Meta-Description, Meta-Keywords, H1, H2 (ONPAGE-008; Extraktionsregeln aus
//! ONPAGE-090, -100, -110, -120, -130 und ONPAGE-003 für die Normalisierung).
//!
//! Die HTML-Quellen sind den Fixtures `onpage-titles`, `onpage-meta`, `onpage-h1`, `onpage-h2` der Spec
//! nachgebaut und stehen inline, damit die Tests ohne Dateien auskommen.

use os_parse::dom::Document;
use os_parse::elements::{extract, ElementSets};

fn sets(html: &str) -> ElementSets {
    extract(&Document::parse(html))
}

fn texts(list: &[os_model::ExtractedText]) -> Vec<&str> {
    list.iter().map(|t| t.text.as_str()).collect()
}

// spec: ONPAGE-008
#[test]
fn title_basic_value_length_and_head_flag() {
    let s = sets("<html><head><title>Start</title></head><body></body></html>");
    assert_eq!(s.titles.len(), 1);
    assert_eq!(s.titles[0].text, "Start");
    assert_eq!(s.titles[0].len, 5);
    assert_eq!(s.titles[0].px, None);
    assert!(s.titles[0].in_head);
}

// spec: ONPAGE-008
#[test]
fn title_entities_and_whitespace_are_normalized_once() {
    // onpage-titles/entities.html
    let s = sets("<html><head><title>  Shop &amp; Mehr&nbsp; </title></head><body></body></html>");
    assert_eq!(texts(&s.titles), ["Shop & Mehr"]);
    assert_eq!(s.titles[0].len, 11);
    // Keine Doppeldekodierung: `&amp;amp;` im Quelltext ist im Titel das Literal `&amp;`.
    let s = sets("<title>A &amp;amp; B</title>");
    assert_eq!(texts(&s.titles), ["A &amp; B"]);
    // Ungültige Entity bleibt erhalten, Umlaute zählen als ein Zeichen.
    let s = sets("<title>&foo; Über</title>");
    assert_eq!(texts(&s.titles), ["&foo; Über"]);
    assert_eq!(s.titles[0].len, 10);
}

// spec: ONPAGE-008
#[test]
fn title_is_rcdata_tags_stay_literal() {
    // onpage-titles/rcdata.html
    let s = sets("<html><head><title>A <b>B</b></title></head><body></body></html>");
    assert_eq!(texts(&s.titles), ["A <b>B</b>"]);
}

// spec: ONPAGE-008
#[test]
fn svg_title_is_ignored() {
    // onpage-titles/svg-title.html: Titel + Inline-SVG mit <title>
    let s = sets(
        "<html><head><title>Seite</title></head><body>\
         <svg width=10 height=10><title>Icon-Beschriftung</title><circle r=4 /></svg></body></html>",
    );
    assert_eq!(texts(&s.titles), ["Seite"]);
    // Nur ein SVG-Titel: kein Vorkommen.
    let s = sets("<html><head></head><body><svg><title>x</title></svg></body></html>");
    assert!(s.titles.is_empty());
}

// spec: ONPAGE-008
#[test]
fn three_titles_are_all_collected_in_document_order() {
    // onpage-titles/multi-3.html: der Kern liefert alle, die Deckelung auf 2 (Occurrences) erfolgt später.
    let s = sets(
        "<html><head><title>Eins</title><title>Zwei</title></head>\
         <body><title>Drei</title></body></html>",
    );
    assert_eq!(texts(&s.titles), ["Eins", "Zwei", "Drei"]);
    assert_eq!(
        s.titles.iter().map(|t| t.in_head).collect::<Vec<_>>(),
        [true, true, false]
    );
}

// spec: ONPAGE-008
#[test]
fn empty_element_counts_as_occurrence_with_empty_value() {
    let s =
        sets("<head><title></title><title>  \n </title></head><body><h1></h1><h2> </h2></body>");
    assert_eq!(texts(&s.titles), ["", ""]);
    assert!(s.titles.iter().all(|t| t.len == 0));
    assert_eq!(texts(&s.h1), [""]);
    assert_eq!(texts(&s.h2), [""]);
}

// spec: ONPAGE-008
#[test]
fn page_without_elements_has_no_occurrences() {
    let s = sets("<html><head></head><body><p>Text</p></body></html>");
    assert_eq!(s, ElementSets::default());
    assert_eq!(extract(&Document::parse("")), ElementSets::default());
}

// spec: ONPAGE-008
#[test]
fn elements_in_template_are_ignored() {
    let s = sets(
        "<html><head><title>T</title></head><body>\
         <template><title>X</title><h1>X</h1><h2>X</h2><meta name=description content=X></template>\
         <h1>Echt</h1></body></html>",
    );
    assert_eq!(texts(&s.titles), ["T"]);
    assert_eq!(texts(&s.h1), ["Echt"]);
    assert!(s.h2.is_empty());
    assert!(s.meta_descriptions.is_empty());
}

// spec: ONPAGE-008
#[test]
fn noscript_content_is_markup_and_counts() {
    // Tree-Builder ohne Scripting (TECH-017): Elemente im <noscript> sind sichtbar.
    let s = sets(
        "<html><head><noscript><meta name=description content=\"ohne JS\"></noscript></head>\
         <body><noscript><h1>Ohne JavaScript</h1></noscript></body></html>",
    );
    assert_eq!(texts(&s.meta_descriptions), ["ohne JS"]);
    assert_eq!(texts(&s.h1), ["Ohne JavaScript"]);
}

// spec: ONPAGE-008
#[test]
fn meta_description_name_is_case_insensitive() {
    // onpage-meta/case.html
    let s = sets(
        "<head><meta name=\"Description\" content=\"Text\"><META NAME=\"DESCRIPTION\" content=\"Zwei\"></head>",
    );
    assert_eq!(texts(&s.meta_descriptions), ["Text", "Zwei"]);
    assert_eq!(s.meta_descriptions[0].len, 4);
    assert!(s.meta_descriptions[0].in_head);
}

// spec: ONPAGE-008
#[test]
fn og_and_twitter_descriptions_do_not_count() {
    // onpage-meta/og-only.html
    let s = sets(
        "<head><meta property=\"og:description\" content=\"X\">\
         <meta name=\"twitter:description\" content=\"Y\">\
         <meta name=\"og:description\" content=\"Z\"></head>",
    );
    assert!(s.meta_descriptions.is_empty());
    assert!(s.meta_keywords.is_empty());
}

// spec: ONPAGE-008
#[test]
fn meta_without_content_is_an_occurrence_with_empty_value() {
    // onpage-meta/no-content.html
    let s = sets("<head><meta name=\"description\"><meta name=\"keywords\"></head>");
    assert_eq!(texts(&s.meta_descriptions), [""]);
    assert_eq!(s.meta_descriptions[0].len, 0);
    assert_eq!(texts(&s.meta_keywords), [""]);
}

// spec: ONPAGE-008
#[test]
fn meta_content_entities_are_decoded_once() {
    // onpage-meta/entities.html
    let s = sets(
        "<head><meta name=\"description\" content=\"Er sagte &quot;Hallo&quot; &amp; ging&nbsp;\">\
         <meta name=\"keywords\" content=\"a&amp;amp;b\"></head>",
    );
    assert_eq!(texts(&s.meta_descriptions), ["Er sagte \"Hallo\" & ging"]);
    assert_eq!(texts(&s.meta_keywords), ["a&amp;b"]);
}

// spec: ONPAGE-008
#[test]
fn meta_keywords_case_insensitive_with_length() {
    // onpage-meta/keywords.html
    let s = sets("<head><meta name=\"KEYWORDS\" content=\"a, b\"></head>");
    assert_eq!(texts(&s.meta_keywords), ["a, b"]);
    assert_eq!(s.meta_keywords[0].len, 4);
}

// spec: ONPAGE-008
#[test]
fn meta_in_body_is_collected_with_head_flag_false() {
    // `<div>` im Head beendet ihn implizit; die Meta danach liegt im Body (TECH-003).
    let s = sets(
        "<html><head><meta name=description content=a><div></div>\
         <meta name=description content=b></head><body></body></html>",
    );
    assert_eq!(texts(&s.meta_descriptions), ["a", "b"]);
    assert_eq!(
        s.meta_descriptions
            .iter()
            .map(|t| t.in_head)
            .collect::<Vec<_>>(),
        [true, false]
    );
}

// spec: ONPAGE-008
#[test]
fn h1_nested_text_is_joined() {
    // onpage-h1/nested.html
    let s = sets("<body><h1>Hallo <span>Welt</span></h1></body>");
    assert_eq!(texts(&s.h1), ["Hallo Welt"]);
    let s = sets("<body><h1>Ab<b>c</b>d</h1></body>");
    assert_eq!(texts(&s.h1), ["Abcd"]);
    let s = sets("<body><h1>  viel \n\t Platz <em> hier </em> </h1></body>");
    assert_eq!(texts(&s.h1), ["viel Platz hier"]);
}

// spec: ONPAGE-008
#[test]
fn h1_includes_img_alt_text() {
    // onpage-h1/img-alt.html (Annahme A-30)
    let s = sets("<body><h1><img src=l.png alt=\"Logo\"> Firma</h1></body>");
    assert_eq!(texts(&s.h1), ["Logo Firma"]);
    // Bild ohne alt trägt nichts bei.
    let s = sets("<body><h1><img src=l.png> Firma</h1></body>");
    assert_eq!(texts(&s.h1), ["Firma"]);
}

// spec: ONPAGE-008
#[test]
fn h1_br_is_whitespace() {
    // onpage-h1/br.html (Annahme A-31)
    let s = sets("<body><h1>A<br>B</h1><h2>C<br/>D<BR>E</h2></body>");
    assert_eq!(texts(&s.h1), ["A B"]);
    assert_eq!(texts(&s.h2), ["C D E"]);
}

// spec: ONPAGE-008
#[test]
fn h1_ignores_script_style_and_svg_text() {
    let s = sets(
        "<body><h1>Titel<script>var x=1;</script><style>h1{}</style>\
         <svg><title>Icon</title></svg> Ende</h1></body>",
    );
    assert_eq!(texts(&s.h1), ["Titel Ende"]);
}

// spec: ONPAGE-008
#[test]
fn h1_pseudo_content_and_hidden_do_not_matter() {
    // onpage-h1/pseudo.html und hidden.html
    let s = sets(
        "<head><style>h1::before { content: \"X\" }</style></head>\
         <body><h1>Sichtbar</h1><h1 style=\"display:none\">Versteckt</h1><h1 hidden>Auch</h1></body>",
    );
    assert_eq!(texts(&s.h1), ["Sichtbar", "Versteckt", "Auch"]);
}

// spec: ONPAGE-008
#[test]
fn five_h1_are_not_capped() {
    // onpage-h1/five-h1.html
    let html: String = (1..=5).map(|i| format!("<h1>Kopf {i}</h1>")).collect();
    let s = sets(&format!("<body>{html}</body>"));
    assert_eq!(s.h1.len(), 5);
    assert_eq!(s.h1[0].text, "Kopf 1");
    assert_eq!(s.h1[1].text, "Kopf 2");
}

// spec: ONPAGE-008
#[test]
fn twelve_h2_are_not_capped_and_nested_text_is_joined() {
    // onpage-h2/twelve.html
    let html: String = (1..=12)
        .map(|i| format!("<h2>Abschnitt <span>{i}</span></h2>"))
        .collect();
    let s = sets(&format!("<body>{html}</body>"));
    assert_eq!(s.h2.len(), 12);
    assert_eq!(s.h2[0].text, "Abschnitt 1");
    assert_eq!(s.h2[11].text, "Abschnitt 12");
    assert_eq!(s.h2[11].len, 12);
}

// spec: ONPAGE-008
#[test]
fn h1_length_counts_codepoints() {
    let s = sets("<body><h1>Größe 😀</h1></body>");
    assert_eq!(s.h1[0].len, 7);
}

// spec: ONPAGE-008
#[test]
fn deeply_nested_heading_does_not_overflow_the_stack() {
    let mut html = String::from("<body><h1>");
    for _ in 0..3000 {
        html.push_str("<div>");
    }
    html.push_str("tief unten");
    // Die Tiefe wird vom Tree-Builder nicht begrenzt; die Auswertung ist iterativ.
    let text = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let doc = Document::parse(&html);
            let text = extract(&doc).h1[0].text.clone();
            // Der Baum wird iterativ freigegeben; erst danach den Text zurückgeben.
            drop(doc);
            text
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(text, "tief unten");
}
