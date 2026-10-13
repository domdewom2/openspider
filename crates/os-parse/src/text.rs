//! Textnormalisierung N(x) und Zeichenlänge (ONPAGE-003, 02 §0.2).

use std::borrow::Cow;

use scraper::Html;

/// Dekodiert HTML-Entities wie ein Browser im Textkontext: benannte (mit und ohne Semikolon, soweit die
/// HTML5-Spezifikation das erlaubt) und numerische; ungültige wie `&foo;` bleiben unverändert.
fn decode_entities(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    // `<` wird vorher maskiert, damit der Tokenizer nie Markup erkennt; Entities dekodiert html5ever.
    let escaped = s.replace('<', "&lt;");
    let fragment = Html::parse_fragment(&escaped);
    let mut out = String::with_capacity(s.len());
    for node in fragment.tree.nodes() {
        if let scraper::Node::Text(t) = node.value() {
            out.push_str(t);
        }
    }
    Cow::Owned(out)
}

/// N(x): (1) Entities dekodieren, (2) jede Folge von Unicode-`White_Space` (inkl. U+00A0, Tab, CR, LF)
/// durch genau ein U+0020 ersetzen, (3) führenden und abschließenden Leerraum entfernen. Keine Änderung der
/// Groß-/Kleinschreibung und keine Unicode-Normalisierung (Annahme A-1).
///
/// Nur für **Rohstrings** (z. B. Header- oder Attributwerte, die nicht durch den HTML-Tree-Builder liefen).
/// Texte und Attributwerte aus dem DOM sind bereits dekodiert: dafür [`normalize_ws`] verwenden, sonst
/// entsteht Doppeldekodierung (`&amp;amp;` im Quelltext → `&amp;` im DOM → `&` nach `normalize`).
pub fn normalize(s: &str) -> String {
    normalize_ws(&decode_entities(s))
}

/// N(x) ohne Entity-Schritt: nur die Schritte (2) und (3), für bereits dekodierte DOM-Texte und -Attribute
/// (`dom::text_content`, `Element::attr`). Die Ausgabe stimmt mit [`normalize`] überein, sobald die
/// Eingabe keine Entities mehr enthält.
pub fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    out
}

/// Länge in Unicode-Codepoints (Annahme A-2). Aufrufer übergeben den bereits mit [`normalize`]
/// normalisierten Text.
pub fn char_len(s: &str) -> u32 {
    u32::try_from(s.chars().count()).unwrap_or(u32::MAX)
}
