//! Inhaltsbereich, Wortzahl und Textanteil (ONPAGE-140, ONPAGE-141, Text Ratio nach Annahme A-7).
//!
//! **Inhaltsbereich (ONPAGE-140):** Textinhalt des `<body>`. Immer ausgeschlossen sind `script`, `style`,
//! `noscript`, `template`, `svg` und Kommentare. Dazu kommen die konfigurierten Exclude-Regeln (Elementnamen,
//! Klassen tokenweise, IDs; Default `nav`, `footer`, `header`, `aside`). Ist mindestens eine Include-Regel gesetzt, zählt nur
//! Text innerhalb passender Elemente; Exclude gewinnt bei Verschachtelung. Blockgrenzen und `<br>` erzeugen
//! Leerraum (`<p>a</p><p>b</p>` → zwei Wörter), Ankergrenzen ebenfalls; sonstige Inline-Elemente nicht (`a<b>b</b>` → ein Wort). Versteckter
//! Text (`hidden`, `display:none`) bleibt enthalten.
//!
//! **Wortzahl (ONPAGE-141, A-35, A-35b):** Anzahl der durch Unicode-Leerraum (inkl. U+00A0) getrennten nicht
//! leeren Token; reine Satzzeichen-Token zählen. Dazu kommt ein Wort je Listeneintrag `<li>` im Inhaltsbereich
//! (A-35b, K-59, Paritätsbefund N-03): Die Referenz zählt je Listenpunkt ein Wort mehr (Aufzählungszeichen der
//! Textdarstellung), unabhängig vom Ankertext. K-59 vermutete „+ 1 je `<a href>`“; an den fünf Paritäts-Sites
//! trifft nur die Regel je `<li>` zellgenau (Links in `<p>` zählen nicht mit, Links in `<li>` genau einmal).
//! Listeneinträge in ausgeschlossenen Bereichen (nav, footer, Exclude-Regeln) zählen nicht. Keine
//! Sprachsegmentierung: CJK-Text ohne Leerzeichen zählt als ein Wort je Block (bekannte Einschränkung).
//!
//! **Text Ratio (A-7):** Zeichen des sichtbaren Textes im gesamten `<body>` (ohne `script`, `style`,
//! `noscript`, `template`; Leerraum zusammengefasst und getrennt durch genau ein Leerzeichen) geteilt durch
//! die Zeichen des gesamten HTML-Quelltextes × 100, drei Nachkommastellen, begrenzt auf 0 … 100.
//! Der Quelltext steht in [`Document`] nicht zur Verfügung; [`analyze`] nähert die Länge deshalb über die
//! Serialisierung des Baums an, [`analyze_with_source`] nimmt die echte Zeichenzahl des dekodierten Quelltexts.

use ego_tree::iter::Edge;
use os_model::ContentAreaConfig;
use scraper::{ElementRef, Node};

use crate::dom::{is_block, Document, XHTML_NS};

/// Kennzahlen des Inhaltsbereichs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContentStats {
    /// Anzahl der Wörter im Inhaltsbereich.
    pub word_count: u32,
    /// Textanteil in Prozent.
    pub text_ratio: f32,
}

/// Zählt Token und deren Zeichen in einem Strom aus Textstücken und Blockgrenzen.
#[derive(Default)]
struct TokenCounter {
    /// Optional text sink (INTEG-027, M2-D02); word-count/ratio callers allocate no text.
    text: Option<String>,
    /// Sink-Modus: Blockgrenzen als Zeilenumbruch und Leerraum zusammengefasst (M2-D02),
    /// sonst Rohtext mit Leerzeichen an Blockgrenzen (INTEG-027).
    keep_breaks: bool,
    tokens: u64,
    /// Anzahl `<li>` im Inhaltsbereich (A-35b).
    list_items: u64,
    token_chars: u64,
    in_token: bool,
}

impl TokenCounter {
    fn push_text(&mut self, text: &str) {
        if !self.keep_breaks {
            if let Some(out) = &mut self.text {
                out.push_str(text);
            }
        }
        for c in text.chars() {
            if c.is_whitespace() {
                self.in_token = false;
                if let (true, Some(out)) = (self.keep_breaks, &mut self.text) {
                    if !out.ends_with([' ', '\n']) {
                        out.push(' ');
                    }
                }
            } else {
                if !self.in_token {
                    self.in_token = true;
                    self.tokens += 1;
                }
                self.token_chars += 1;
                if let (true, Some(out)) = (self.keep_breaks, &mut self.text) {
                    out.push(c);
                }
            }
        }
    }

    fn inline_boundary(&mut self) {
        if let Some(out) = &mut self.text {
            if !out.ends_with([' ', '\n']) {
                out.push(' ');
            }
        }
        self.in_token = false;
    }

    fn boundary(&mut self) {
        if let Some(out) = &mut self.text {
            if !self.keep_breaks {
                out.push(' ');
            } else if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
        }
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
    /// Je `<li>` ein Wort mehr zählen (Wortzahl, A-35b); die Text Ratio zählt keine Listenpunkte.
    count_list_items: bool,
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
            count_list_items: true,
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
            count_list_items: false,
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
    walk_into(body, rules, TokenCounter::default())
}

/// Same traversal with an optional text sink; existing counters keep their defaults.
fn walk_into(body: ElementRef, rules: &Rules, mut counter: TokenCounter) -> TokenCounter {
    let has_include = rules.has_include();
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
                    let block = is_block(name);
                    if rules.count_list_items && name == "a" {
                        counter.inline_boundary();
                    }
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
                    if rules.count_list_items
                        && (!has_include || include_depth > 0)
                        && name == "li"
                        && &*el.name.ns == XHTML_NS
                    {
                        counter.list_items += 1;
                    }
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
                        if skip_depth == 0 && is_block(el.name()) {
                            counter.boundary();
                        }
                        continue;
                    }
                    if include_marks.pop() == Some(true) {
                        include_depth -= 1;
                    }
                    if rules.count_list_items && el.name() == "a" {
                        counter.inline_boundary();
                    }
                    if is_block(el.name()) {
                        counter.boundary();
                    }
                }
            }
        }
    }
    counter
}

/// Text des Inhaltsbereichs (INTEG-027), mit denselben Regeln und Blockgrenzen
/// wie die Wortzahl (ONPAGE-140/141). Listenpunkt-Zusatzwörter sind keine Textzeichen.
/// Leerraum ist zu einzelnen Leerzeichen zusammengefasst.
pub fn area_text_flat(doc: &Document, area: &ContentAreaConfig) -> String {
    let Some(body) = body_of(doc) else {
        return String::new();
    };
    let counter = TokenCounter {
        text: Some(String::new()),
        ..Default::default()
    };
    let text = walk_into(body, &Rules::for_area(area), counter)
        .text
        .unwrap_or_default();
    crate::text::normalize_ws(&text)
}

/// Wortzahl des Inhaltsbereichs (ONPAGE-141).
pub fn word_count(doc: &Document, area: &ContentAreaConfig) -> u32 {
    let Some(body) = body_of(doc) else {
        return 0;
    };
    let counter = walk(body, &Rules::for_area(area));
    u32::try_from(counter.tokens + counter.list_items).unwrap_or(u32::MAX)
}

/// Text Ratio in Prozent (drei Nachkommastellen) aus der Zeichenzahl des gesamten Quelltexts (A-7).
fn text_ratio(doc: &Document, source_chars: usize) -> f32 {
    let Some(body) = body_of(doc) else {
        return 0.0;
    };
    if source_chars == 0 {
        return 0.0;
    }
    let text_chars = walk(body, &Rules::whole_body()).normalized_chars();
    let ratio = (text_chars as f64 / source_chars as f64 * 100.0).clamp(0.0, 100.0);
    ((ratio * 1000.0).round() / 1000.0) as f32
}

/// Berechnet Wortzahl und Textanteil. Die Länge des HTML-Quelltexts wird aus der Serialisierung des Baums
/// angenähert, weil [`Document`] den Quelltext nicht hält; [`analyze_with_source`] liefert den exakten
/// Textanteil.
pub fn analyze(doc: &Document, area: &ContentAreaConfig) -> ContentStats {
    analyze_with_source(doc, area, serialized_chars(doc))
}

/// Schreibziel, das nur die UTF-8-Zeichen zählt (Bytes, die keine Fortsetzungsbytes sind) und nichts
/// speichert: die Serialisierung des Baums braucht so keine seitengroße String-Allokation.
#[derive(Default)]
struct CharCounter {
    chars: usize,
}

impl std::io::Write for CharCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.chars += buf.iter().filter(|b| (**b & 0xC0) != 0x80).count();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Zeichenzahl der Serialisierung des gesamten Baums (identisch mit `root_element().html().chars().count()`,
/// aber ohne den String zu bauen).
fn serialized_chars(doc: &Document) -> usize {
    use html5ever::serialize::{serialize, SerializeOpts, TraversalScope};
    let mut counter = CharCounter::default();
    let opts = SerializeOpts {
        scripting_enabled: false,
        traversal_scope: TraversalScope::IncludeNode,
        create_missing_parent: false,
    };
    // Schreiben in den Zähler schlägt nie fehl.
    let _ = serialize(&mut counter, &doc.html().root_element(), opts);
    counter.chars
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

/// Text des Inhaltsbereichs; Blockgrenzen bleiben für die Satzanalyse erhalten.
pub fn area_text(doc: &Document, area: &ContentAreaConfig) -> String {
    body_text_with_breaks(doc, &Rules::for_area(area))
}

/// Text mit erhaltenen Blockgrenzen für die Regeln `rules`.
fn body_text_with_breaks(doc: &Document, rules: &Rules) -> String {
    let Some(body) = body_of(doc) else {
        return String::new();
    };
    let counter = TokenCounter {
        text: Some(String::new()),
        keep_breaks: true,
        ..Default::default()
    };
    walk_into(body, rules, counter)
        .text
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Erfasst Text für konfigurierbare Filter ohne erneuten Abruf.
pub fn run(ctx: &crate::PageCtx<'_>, page: &mut os_model::ParsedPage) {
    use os_model::{ExtKey, Value};
    let area = area_text(ctx.doc, &ctx.cfg.content_area);
    let body = body_text_with_breaks(ctx.doc, &Rules::whole_body());
    let select_text = |name| {
        let selector = scraper::Selector::parse(name).expect("static selector");
        ctx.doc
            .html()
            .select(&selector)
            .map(|e| e.text().collect::<String>())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let soft = crate::text::normalize(&format!(
        "{} {} {}",
        select_text("title"),
        select_text("h1"),
        area
    ))
    .to_lowercase();
    let selector = scraper::Selector::parse("meta[name]").expect("static selector");
    let description = ctx
        .doc
        .html()
        .select(&selector)
        .filter(|e| {
            e.value()
                .attr("name")
                .is_some_and(|n| n.eq_ignore_ascii_case("description"))
        })
        .filter_map(|e| e.value().attr("content"))
        .collect::<Vec<_>>()
        .join(" ");
    let lorem = crate::text::normalize(&format!(
        "{} {} {}",
        select_text("title"),
        description,
        body
    ))
    .to_lowercase()
    .matches("lorem ipsum")
    .count();
    for (key, value) in [
        ("content.text", Value::Text(area)),
        ("content.soft404_text", Value::Text(soft)),
        ("content.lorem", Value::Bool(lorem > 0)),
        ("content.lorem_occurrences", Value::Int(lorem as i64)),
    ] {
        page.ext.push((ExtKey::from_static(key), value));
    }
}
