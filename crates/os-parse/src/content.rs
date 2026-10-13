//! Inhaltsbereich, Wortzahl und Textanteil (ONPAGE-140, ONPAGE-141, Text Ratio nach Annahme A-7).
//!
//! **Inhaltsbereich (ONPAGE-140):** Textinhalt des `<body>`. Immer ausgeschlossen sind `script`, `style`,
//! `noscript`, `template`, `svg` und Kommentare. Dazu kommen die konfigurierten Exclude-Regeln (Elementnamen,
//! Klassen tokenweise, IDs; Default `nav`, `footer`). Ist mindestens eine Include-Regel gesetzt, zählt nur
//! Text innerhalb passender Elemente; Exclude gewinnt bei Verschachtelung. Blockgrenzen und `<br>` erzeugen
//! Leerraum (`<p>a</p><p>b</p>` → zwei Wörter), Inline-Elemente nicht (`a<b>b</b>` → ein Wort). Versteckter
//! Text (`hidden`, `display:none`) bleibt enthalten.
//!
//! **Wortzahl (ONPAGE-141, A-35):** Anzahl der durch Unicode-Leerraum (inkl. U+00A0) getrennten nicht
//! leeren Token; reine Satzzeichen-Token zählen. Keine Sprachsegmentierung: CJK-Text ohne Leerzeichen zählt
//! als ein Wort je Block (bekannte Einschränkung).
//!
//! **Text Ratio (A-7):** Zeichen des sichtbaren Textes im gesamten `<body>` (ohne `script`, `style`,
//! `noscript`, `template`; Leerraum zusammengefasst und getrennt durch genau ein Leerzeichen) geteilt durch
//! die Zeichen des gesamten HTML-Quelltextes × 100, eine Nachkommastelle, begrenzt auf 0 … 100.
//! Der Quelltext steht in [`Document`] nicht zur Verfügung; [`analyze`] nähert die Länge deshalb über die
//! Serialisierung des Baums an, [`analyze_with_source`] nimmt die echte Zeichenzahl des dekodierten Quelltexts.

use ego_tree::iter::Edge;
use os_model::ContentAreaConfig;
use scraper::{ElementRef, Node};

use crate::dom::Document;

const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// Kennzahlen des Inhaltsbereichs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContentStats {
    /// Anzahl der Wörter im Inhaltsbereich.
    pub word_count: u32,
    /// Textanteil in Prozent.
    pub text_ratio: f32,
}

/// Elemente, die einen Block bilden (Leerraum davor und danach); `br` zählt als Umbruch.
pub(crate) fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "body"
            | "br"
            | "caption"
            | "dd"
            | "details"
            | "dialog"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "ul"
    )
}

/// Zählt Token und deren Zeichen in einem Strom aus Textstücken und Blockgrenzen.
#[derive(Default)]
struct TokenCounter {
    tokens: u64,
    token_chars: u64,
    in_token: bool,
}

impl TokenCounter {
    fn push_text(&mut self, text: &str) {
        for c in text.chars() {
            if c.is_whitespace() {
                self.in_token = false;
            } else {
                if !self.in_token {
                    self.in_token = true;
                    self.tokens += 1;
                }
                self.token_chars += 1;
            }
        }
    }

    fn boundary(&mut self) {
        self.in_token = false;
    }

    /// Länge des Textes, wenn die Token mit je genau einem Leerzeichen verbunden werden.
    fn normalized_chars(&self) -> u64 {
        self.token_chars + self.tokens.saturating_sub(1)
    }
}

/// Regeln für den Durchlauf: Namen sind klein geschrieben, Klassen/IDs exakt.
struct Rules {
    include_elements: Vec<String>,
    include_classes: Vec<String>,
    include_ids: Vec<String>,
    exclude_elements: Vec<String>,
    exclude_classes: Vec<String>,
    exclude_ids: Vec<String>,
    /// `svg` ausschließen (Inhaltsbereich) oder mitzählen (Text Ratio, A-7).
    drop_svg: bool,
}

fn lower_list(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn token_list(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|s| s.trim().trim_start_matches(['.', '#']).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl Rules {
    fn for_area(area: &ContentAreaConfig) -> Self {
        Rules {
            include_elements: lower_list(&area.include_elements),
            include_classes: token_list(&area.include_classes),
            include_ids: token_list(&area.include_ids),
            exclude_elements: lower_list(&area.exclude_elements),
            exclude_classes: token_list(&area.exclude_classes),
            exclude_ids: token_list(&area.exclude_ids),
            drop_svg: true,
        }
    }

    /// Gesamter Body ohne Konfiguration (Text Ratio).
    fn whole_body() -> Self {
        Rules {
            include_elements: Vec::new(),
            include_classes: Vec::new(),
            include_ids: Vec::new(),
            exclude_elements: Vec::new(),
            exclude_classes: Vec::new(),
            exclude_ids: Vec::new(),
            drop_svg: false,
        }
    }

    fn has_include(&self) -> bool {
        !(self.include_elements.is_empty()
            && self.include_classes.is_empty()
            && self.include_ids.is_empty())
    }

    fn always_dropped(&self, name: &str) -> bool {
        match name {
            "script" | "style" | "noscript" | "template" => true,
            "svg" => self.drop_svg,
            _ => false,
        }
    }

    fn excluded(&self, el: &scraper::node::Element) -> bool {
        matches_rule(
            el,
            &self.exclude_elements,
            &self.exclude_classes,
            &self.exclude_ids,
        )
    }

    fn included(&self, el: &scraper::node::Element) -> bool {
        matches_rule(
            el,
            &self.include_elements,
            &self.include_classes,
            &self.include_ids,
        )
    }
}

fn matches_rule(
    el: &scraper::node::Element,
    names: &[String],
    classes: &[String],
    ids: &[String],
) -> bool {
    if !names.is_empty() && names.iter().any(|n| n == el.name()) {
        return true;
    }
    if !classes.is_empty() {
        if let Some(class_attr) = el.attr("class") {
            if class_attr
                .split_ascii_whitespace()
                .any(|token| classes.iter().any(|c| c == token))
            {
                return true;
            }
        }
    }
    if !ids.is_empty() {
        if let Some(id) = el.attr("id") {
            if ids.iter().any(|i| i == id) {
                return true;
            }
        }
    }
    false
}

/// Das `<body>`-Element des HTML-Baums (der Tree-Builder legt es immer an, außer bei Frameset-Dokumenten).
fn body_of(doc: &Document) -> Option<ElementRef<'_>> {
    doc.html()
        .root_element()
        .children()
        .filter_map(ElementRef::wrap)
        .find(|e| e.value().name() == "body" && &*e.value().name.ns == XHTML_NS)
}

/// Durchläuft den Body iterativ (kein Stapelüberlauf bei tiefen Bäumen) und speist den Zähler.
fn walk(body: ElementRef, rules: &Rules) -> TokenCounter {
    let has_include = rules.has_include();
    let mut counter = TokenCounter::default();
    // > 0: innerhalb eines ausgeschlossenen Teilbaums (Tiefe relativ zu dessen Wurzel).
    let mut skip_depth = 0usize;
    // Wie viele geöffnete Vorfahren (inkl. Selbst) passen zu einer Include-Regel.
    let mut include_depth = 0usize;
    // Je geöffnetem, nicht übersprungenem Element: hat es `include_depth` erhöht?
    let mut include_marks: Vec<bool> = Vec::new();

    for edge in body.traverse() {
        match edge {
            Edge::Open(node) => match node.value() {
                Node::Element(el) => {
                    if skip_depth > 0 {
                        skip_depth += 1;
                        continue;
                    }
                    let name = el.name();
                    let block = is_block_element(name);
                    if rules.always_dropped(name) || rules.excluded(el) {
                        skip_depth = 1;
                        if block {
                            counter.boundary();
                        }
                        continue;
                    }
                    if block {
                        counter.boundary();
                    }
                    let hit = has_include && rules.included(el);
                    if hit {
                        include_depth += 1;
                    }
                    include_marks.push(hit);
                }
                Node::Text(text) if skip_depth == 0 && (!has_include || include_depth > 0) => {
                    counter.push_text(text);
                }
                _ => {}
            },
            Edge::Close(node) => {
                if let Node::Element(el) = node.value() {
                    if skip_depth > 0 {
                        skip_depth -= 1;
                        if skip_depth == 0 && is_block_element(el.name()) {
                            counter.boundary();
                        }
                        continue;
                    }
                    if include_marks.pop() == Some(true) {
                        include_depth -= 1;
                    }
                    if is_block_element(el.name()) {
                        counter.boundary();
                    }
                }
            }
        }
    }
    counter
}

/// Wortzahl des Inhaltsbereichs (ONPAGE-141).
pub fn word_count(doc: &Document, area: &ContentAreaConfig) -> u32 {
    let Some(body) = body_of(doc) else {
        return 0;
    };
    u32::try_from(walk(body, &Rules::for_area(area)).tokens).unwrap_or(u32::MAX)
}

/// Text Ratio in Prozent (eine Nachkommastelle) aus der Zeichenzahl des gesamten Quelltexts (A-7).
fn text_ratio(doc: &Document, source_chars: usize) -> f32 {
    let Some(body) = body_of(doc) else {
        return 0.0;
    };
    if source_chars == 0 {
        return 0.0;
    }
    let text_chars = walk(body, &Rules::whole_body()).normalized_chars();
    let ratio = (text_chars as f64 / source_chars as f64 * 100.0).clamp(0.0, 100.0);
    ((ratio * 10.0).round() / 10.0) as f32
}

/// Berechnet Wortzahl und Textanteil. Die Länge des HTML-Quelltexts wird aus der Serialisierung des Baums
/// angenähert, weil [`Document`] den Quelltext nicht hält; [`analyze_with_source`] liefert den exakten
/// Textanteil.
pub fn analyze(doc: &Document, area: &ContentAreaConfig) -> ContentStats {
    let approx = doc.html().root_element().html().chars().count();
    analyze_with_source(doc, area, approx)
}

/// Wie [`analyze`], aber mit der echten Zeichenzahl des dekodierten HTML-Quelltexts (`html.chars().count()`)
/// für den Textanteil nach A-7.
pub fn analyze_with_source(
    doc: &Document,
    area: &ContentAreaConfig,
    source_chars: usize,
) -> ContentStats {
    ContentStats {
        word_count: word_count(doc, area),
        text_ratio: text_ratio(doc, source_chars),
    }
}
