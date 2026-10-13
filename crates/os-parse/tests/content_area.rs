//! Inhaltsbereich (ONPAGE-140), Wortzählung (ONPAGE-141) und Text Ratio (A-7).

use os_model::ContentAreaConfig;
use os_parse::content::{analyze, analyze_with_source, word_count};
use os_parse::dom::Document;

fn words(prefix: &str, n: usize) -> String {
    (1..=n)
        .map(|i| format!("{prefix}{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Nachbau von `onpage-content/area.html`: header 5, nav 20, main 100, `.menu.mobile-menu` 15,
/// `#comments` 30, aside 10, footer 25, `div[hidden]` 3 Wörter.
fn area_html() -> String {
    format!(
        "<!doctype html><html><head><title>Bereich</title></head><body>\
         <header>{}</header>\
         <nav>{}</nav>\
         <main>{}\
           <div class=\"menu mobile-menu\">{}</div>\
           <section id=\"comments\">{}</section>\
         </main>\
         <aside>{}</aside>\
         <footer>{}</footer>\
         <div hidden>{}</div>\
         </body></html>",
        words("h", 5),
        words("n", 20),
        words("m", 100),
        words("x", 15),
        words("c", 30),
        words("a", 10),
        words("f", 25),
        words("v", 3),
    )
}

fn count(html: &str) -> u32 {
    word_count(&Document::parse(html), &ContentAreaConfig::default())
}

fn count_with(html: &str, area: &ContentAreaConfig) -> u32 {
    word_count(&Document::parse(html), area)
}

fn area(f: impl FnOnce(&mut ContentAreaConfig)) -> ContentAreaConfig {
    let mut a = ContentAreaConfig::default();
    f(&mut a);
    a
}

// spec: ONPAGE-141
#[test]
fn word_count_examples() {
    assert_eq!(count("<body>Das ist ein Test.</body>"), 4);
    // Satzzeichen-Token zählen (A-35): `Preis:`, `10`, `€`, `–`, `jetzt!`
    assert_eq!(count("<body>Preis: 10 € – jetzt!</body>"), 5);
    assert_eq!(count("<body>E-Mail-Adresse</body>"), 1);
    // NBSP trennt.
    assert_eq!(count("<body>a&nbsp;b</body>"), 2);
    assert_eq!(count("<body></body>"), 0);
    assert_eq!(count("<body>   \n\t  </body>"), 0);
    assert_eq!(count(""), 0);
    // onpage-content/wordcount-3.html
    assert_eq!(count("<body><p>eins zwei drei</p></body>"), 3);
}

// spec: ONPAGE-141
#[test]
fn word_count_unicode_whitespace_and_cjk() {
    // Ideographisches Leerzeichen U+3000 und Zeilenumbrüche trennen.
    assert_eq!(count("<body>eins\u{3000}zwei\r\ndrei\tvier</body>"), 4);
    // Keine Sprachsegmentierung: ein Block CJK ohne Leerzeichen = ein Wort.
    assert_eq!(count("<body><p>これは日本語のテキストです</p></body>"), 1);
}

// spec: ONPAGE-141
#[test]
fn area_html_default_counts_163_words() {
    // 5 + 100 + 15 + 30 + 10 + 3 = 163 (nur nav/footer ausgeschlossen)
    assert_eq!(count(&area_html()), 163);
}

// spec: ONPAGE-140
#[test]
fn default_excludes_nav_and_footer_but_keeps_header_aside_main() {
    let html = "<body><header>h1 h2</header><nav>n1 n2 n3</nav><main>m1</main>\
                <aside>a1 a2</aside><footer>f1 f2 f3 f4</footer></body>";
    assert_eq!(count(html), 2 + 1 + 2);
    // Auch tief verschachtelt: nav in header, footer in article.
    let html =
        "<body><header>h <nav>n1 n2</nav></header><article>t <footer>f1</footer></article></body>";
    assert_eq!(count(html), 2);
    // nav/footer zählen aber zum Text Ratio (A-7), siehe unten.
}

// spec: ONPAGE-140
#[test]
fn exclude_class_matches_tokens_not_substrings() {
    let html = area_html();
    let a = area(|a| a.exclude_classes = vec!["mobile-menu".into()]);
    assert_eq!(count_with(&html, &a), 163 - 15);
    // Token „menu“ trifft ebenfalls (Klasse „menu mobile-menu“ enthält das Token „menu“).
    let a = area(|a| a.exclude_classes = vec!["menu".into()]);
    assert_eq!(count_with(&html, &a), 163 - 15);
    // „mobile“ ist nur Teilstring von „mobile-menu“, kein Token.
    let a = area(|a| a.exclude_classes = vec!["mobile".into()]);
    assert_eq!(count_with(&html, &a), 163);
    // Reihenfolge und Mehrfach-Leerraum im class-Attribut sind egal.
    let html = "<body><div class=\"  a   mobile-menu\t b \">x y</div><p>z</p></body>";
    let a = area(|a| a.exclude_classes = vec!["mobile-menu".into()]);
    assert_eq!(count_with(html, &a), 1);
}

// spec: ONPAGE-140
#[test]
fn exclude_id_removes_comments_section() {
    let a = area(|a| a.exclude_ids = vec!["comments".into()]);
    assert_eq!(count_with(&area_html(), &a), 163 - 30);
    // IDs sind exakt (Groß-/Kleinschreibung zählt).
    let a = area(|a| a.exclude_ids = vec!["Comments".into()]);
    assert_eq!(count_with(&area_html(), &a), 163);
}

// spec: ONPAGE-140
#[test]
fn exclude_elements_replace_default_and_ignore_case() {
    let a = area(|a| a.exclude_elements = vec!["ASIDE".into(), "Header".into()]);
    // nav und footer sind jetzt eingeschlossen (Default ersetzt).
    assert_eq!(count_with(&area_html(), &a), 100 + 15 + 30 + 20 + 25 + 3);
    let a = area(|a| a.exclude_elements = Vec::new());
    assert_eq!(
        count_with(&area_html(), &a),
        5 + 20 + 100 + 15 + 30 + 10 + 25 + 3
    );
}

// spec: ONPAGE-140
#[test]
fn include_element_main_counts_only_main() {
    let a = area(|a| a.include_elements = vec!["main".into()]);
    // main enthält 100 + 15 + 30 Wörter.
    assert_eq!(count_with(&area_html(), &a), 100 + 15 + 30);
    // Include-Element + Exclude-ID: Exclude gewinnt bei Verschachtelung.
    let a = area(|a| {
        a.include_elements = vec!["main".into()];
        a.exclude_ids = vec!["comments".into()];
        a.exclude_classes = vec!["mobile-menu".into()];
    });
    assert_eq!(count_with(&area_html(), &a), 100);
}

// spec: ONPAGE-140
#[test]
fn include_by_class_and_id_and_exclude_wins_over_inner_include() {
    let html = "<body><div class=\"content\">a b <span id=\"keep\">c</span></div>\
                <div id=\"keep\">d e f</div><p>g h i j</p></body>";
    let a = area(|a| a.include_classes = vec!["content".into()]);
    assert_eq!(count_with(html, &a), 3);
    let a = area(|a| a.include_ids = vec!["keep".into()]);
    assert_eq!(count_with(html, &a), 1 + 3);
    let a = area(|a| {
        a.include_classes = vec!["content".into()];
        a.include_ids = vec!["keep".into()];
    });
    assert_eq!(count_with(html, &a), 3 + 3);
    // Ein ausgeschlossenes Element schließt auch darin liegende Include-Treffer aus.
    let html = "<body><nav><main>drin</main></nav><main>raus rein</main></body>";
    let a = area(|a| a.include_elements = vec!["main".into()]);
    assert_eq!(count_with(html, &a), 2);
    // Include-Regel ohne Treffer → 0.
    let a = area(|a| a.include_elements = vec!["article".into()]);
    assert_eq!(count_with(html, &a), 0);
}

// spec: ONPAGE-140, ONPAGE-141
#[test]
fn block_boundaries_create_whitespace_inline_elements_do_not() {
    assert_eq!(count("<body><p>a</p><p>b</p></body>"), 2);
    assert_eq!(count("<body><div>a</div>b</body>"), 2);
    assert_eq!(
        count("<body><ul><li>a</li><li>b</li><li>c</li></ul></body>"),
        3
    );
    assert_eq!(
        count("<body><table><tr><td>a</td><td>b</td></tr></table></body>"),
        2
    );
    assert_eq!(count("<body>a<br>b</body>"), 2);
    assert_eq!(count("<body><h2>a</h2><h3>b</h3></body>"), 2);
    // Inline-Elemente trennen nicht.
    assert_eq!(count("<body>a<b>b</b>c <em>d</em><span>e</span></body>"), 2);
    assert_eq!(count("<body><a href=\"/x\">link</a>text</body>"), 1);
}

// spec: ONPAGE-140
#[test]
fn hidden_text_is_kept() {
    assert_eq!(count("<body><div hidden>a b c</div><p>d</p></body>"), 4);
    assert_eq!(count("<body><p style=\"display:none\">a b</p></body>"), 2);
    assert_eq!(
        count("<body><p style=\"visibility:hidden\">a</p><p aria-hidden=\"true\">b</p></body>"),
        2
    );
}

// spec: ONPAGE-140
#[test]
fn script_style_noscript_template_svg_and_comments_never_count() {
    let html = "<body>vorn \
        <script>var wort = 1; zwei dreI</script>\
        <style>.a { color: red } b c</style>\
        <noscript>kein javascript hier</noscript>\
        <template><p>in der vorlage</p></template>\
        <svg><text>vektor text</text><title>titel</title></svg>\
        <!-- ein kommentar mit worten -->\
        hinten</body>";
    assert_eq!(count(html), 2);
    // Auch nach einem Block, der nur ausgeschlossenen Inhalt trägt, bleibt die Trennung erhalten.
    assert_eq!(count("<body>a<div><script>x</script></div>b</body>"), 2);
    // Script zwischen Inline-Text trennt Wörter nicht.
    assert_eq!(count("<body>ab<script>x</script>cd</body>"), 1);
}

// spec: ONPAGE-140
#[test]
fn head_content_is_not_part_of_the_area() {
    let html =
        "<html><head><title>Ein langer Titel hier</title></head><body>nur body</body></html>";
    assert_eq!(count(html), 2);
}

// spec: ONPAGE-140
#[test]
fn deeply_nested_document_does_not_overflow_the_stack() {
    let mut html = String::from("<body>");
    for _ in 0..3000 {
        html.push_str("<div>");
    }
    html.push_str("tief unten");
    // Die Tiefe wird vom Tree-Builder nicht begrenzt; die Auswertung ist iterativ.
    let n = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let doc = Document::parse(&html);
            let n = word_count(&doc, &ContentAreaConfig::default());
            // Der Baum wird in diesem Thread fallengelassen; scraper gibt ihn iterativ frei.
            n
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(n, 2);
}

// spec: ONPAGE-141
#[test]
fn text_ratio_uses_whole_body_over_source_chars() {
    // 14 Zeichen Text („eins zwei drei“) in genau 100 Zeichen Quelltext → 14,0 %.
    let mut html = String::from("<html><body><p>eins zwei drei</p></body></html><!--");
    let pad = 100 - html.chars().count() - "-->".len();
    html.push_str(&"x".repeat(pad));
    html.push_str("-->");
    assert_eq!(html.chars().count(), 100);
    let stats = analyze_with_source(&Document::parse(&html), &ContentAreaConfig::default(), 100);
    assert_eq!(stats.word_count, 3);
    assert_eq!(stats.text_ratio, 14.0);
}

// spec: ONPAGE-141
#[test]
fn text_ratio_rounds_to_one_decimal_and_stays_in_range() {
    let doc = Document::parse("<body>abc</body>");
    // 3 / 7 = 42,857… → 42,9
    let stats = analyze_with_source(&doc, &ContentAreaConfig::default(), 7);
    assert_eq!(stats.text_ratio, 42.9);
    // Quelltext kürzer als der Text (nur bei fehlerhafter Angabe möglich): auf 100 begrenzt.
    let stats = analyze_with_source(&doc, &ContentAreaConfig::default(), 2);
    assert_eq!(stats.text_ratio, 100.0);
    // Quelltextlänge 0 → 0, keine Division durch 0.
    let stats = analyze_with_source(&doc, &ContentAreaConfig::default(), 0);
    assert_eq!(stats.text_ratio, 0.0);
}

// spec: ONPAGE-141
#[test]
fn text_ratio_ignores_script_style_but_counts_nav_and_footer() {
    let plain = "<body><p>eins zwei</p></body>";
    let with_script = "<body><p>eins zwei</p><script>lorem ipsum dolor sit amet</script>\
                       <style>p { color: red }</style><noscript>kein js</noscript>\
                       <template>vorlage text</template></body>";
    let cfg = ContentAreaConfig::default();
    let a = analyze_with_source(&Document::parse(plain), &cfg, 1000);
    let b = analyze_with_source(&Document::parse(with_script), &cfg, 1000);
    assert_eq!(a.text_ratio, b.text_ratio);
    assert_eq!(a.text_ratio, 0.9); // „eins zwei“ = 9 Zeichen von 1000

    // Seite mit großem Footer: Footer-Text zählt zum Text Ratio (A-7), nicht zur Wortzahl.
    let footer = "<body><p>a</p><footer>bb cc</footer></body>";
    let stats = analyze_with_source(&Document::parse(footer), &cfg, 1000);
    assert_eq!(stats.word_count, 1);
    assert_eq!(stats.text_ratio, 0.7); // „a bb cc“ = 7 Zeichen von 1000
}

// spec: ONPAGE-141
#[test]
fn text_ratio_is_zero_without_body_text() {
    let cfg = ContentAreaConfig::default();
    for html in [
        "<body></body>",
        "<body>  \n </body>",
        "<body><script>x</script></body>",
        "",
    ] {
        let stats = analyze_with_source(&Document::parse(html), &cfg, 500);
        assert_eq!(stats.text_ratio, 0.0, "{html:?}");
    }
}

// spec: ONPAGE-141
#[test]
fn analyze_without_source_stays_within_range() {
    let stats = analyze(
        &Document::parse("<body><p>eins zwei drei</p></body>"),
        &ContentAreaConfig::default(),
    );
    assert_eq!(stats.word_count, 3);
    assert!(
        stats.text_ratio > 0.0 && stats.text_ratio <= 100.0,
        "{}",
        stats.text_ratio
    );
    assert_eq!(
        analyze(&Document::parse(""), &ContentAreaConfig::default()),
        Default::default()
    );
}

// spec: ONPAGE-141
#[test]
fn analyze_counts_serialized_chars_without_building_the_string() {
    // Mehrbyte-Zeichen (ä, €, Emoji) und Entities: die Zählung muss Codepoints zählen, nicht Bytes, und mit
    // der Zeichenzahl der Baum-Serialisierung übereinstimmen.
    let html = "<html><head><title>Käse &amp; €</title></head>\
                <body><p>Größe 10 € 😀 &lt;ok&gt;</p><img src=\"a.png\" alt=\"ä\"></body></html>";
    let doc = Document::parse(html);
    let cfg = ContentAreaConfig::default();
    let serialized = doc.html().root_element().html().chars().count();
    assert_eq!(
        analyze(&doc, &cfg),
        analyze_with_source(&doc, &cfg, serialized)
    );
}
