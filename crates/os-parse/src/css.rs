//! Verweise aus CSS: `<style>`, `style=` und CSS-Dateien (CRAWL-040).
//!
//! Gefunden werden `url(…)` in den drei Schreibweisen (`url(a.png)`, `url('a.png')`, `url("a.png")`),
//! Zeichenketten in `image-set(…)` und `@import` (mit `url()` oder als Zeichenkette). Typen:
//! `@import` → `Css`, `url()` innerhalb von `@font-face` → `Other` (Schriftart; die Kategorie „Schriftarten“
//! ergibt sich aus dem Content-Type der Antwort, `LinkType` hat keinen eigenen Wert), sonst `Image`.
//! Nicht gewertet werden `data:`-URIs, reine Fragmentverweise (`url(#clip)`) sowie `url()` in
//! `@namespace`, `@document` und `@supports`, weil dort keine Ressource geladen wird.

use os_model::{ExtractedLink, LinkPosition, LinkType};

use crate::base::{clean_url_text, is_data_uri, path_type, resolve_string};
use crate::dom::{self, Document};
use crate::links::{is_html, walk};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Image,
    Import,
    Font,
}

impl RefKind {
    fn link_type(self) -> LinkType {
        match self {
            RefKind::Image => LinkType::Image,
            RefKind::Import => LinkType::Css,
            RefKind::Font => LinkType::Other,
        }
    }
}

/// Verweise aus `<style>`-Elementen und `style=`-Attributen.
pub fn extract_inline(doc: &Document, base: &url::Url) -> Vec<ExtractedLink> {
    let mut out = Vec::new();
    walk(doc, |el, ordinal, position| {
        let v = el.value();
        let make = |refs: Vec<(RefKind, String)>, out: &mut Vec<ExtractedLink>| {
            for (kind, raw) in refs {
                out.push(link(
                    base,
                    kind,
                    &raw,
                    crate::link_position::element_path(doc, el),
                    position.clone(),
                    dom::in_head(el),
                    ordinal,
                ));
            }
        };
        if let Some(style) = v.attr("style") {
            make(scan(style), &mut out);
        }
        if v.name() == "style" && (is_html(el) || &*v.name.ns == "http://www.w3.org/2000/svg") {
            let css: String = el.text().collect();
            make(scan(&css), &mut out);
        }
    });
    out
}

/// Verweise aus einer CSS-Datei; relative Verweise werden gegen die URL der CSS-Datei aufgelöst.
pub fn extract_sheet(base: &url::Url, body: &[u8]) -> Vec<ExtractedLink> {
    let css = crate::charset::decode_plain(body);
    scan(&css)
        .into_iter()
        .enumerate()
        .map(|(i, (kind, raw))| {
            link(
                base,
                kind,
                &raw,
                String::new(),
                LinkPosition::Content,
                false,
                u32::try_from(i).unwrap_or(u32::MAX),
            )
        })
        .collect()
}

fn link(
    base: &url::Url,
    kind: RefKind,
    raw: &str,
    xpath: String,
    position: LinkPosition,
    in_head: bool,
    doc_order: u32,
) -> ExtractedLink {
    let cleaned = clean_url_text(raw);
    ExtractedLink {
        link_type: kind.link_type(),
        href_raw: raw.to_string(),
        resolved: resolve_string(base, &cleaned),
        anchor: None,
        alt_text: None,
        rel: None,
        nofollow_rel: false,
        target: None,
        path_type: path_type(&cleaned),
        xpath,
        position,
        in_head,
        doc_order,
        ..Default::default()
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
}

fn is_css_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

/// Überspringt einen Kommentar ab `c[i] == '/'`, `c[i+1] == '*'`.
fn skip_comment(c: &[char], mut i: usize) -> usize {
    i += 2;
    while i < c.len() {
        if c[i] == '*' && c.get(i + 1) == Some(&'/') {
            return i + 2;
        }
        i += 1;
    }
    c.len()
}

/// Überspringt Leerraum und Kommentare.
fn skip_space(c: &[char], mut i: usize) -> usize {
    loop {
        if i < c.len() && is_css_space(c[i]) {
            i += 1;
        } else if c.get(i) == Some(&'/') && c.get(i + 1) == Some(&'*') {
            i = skip_comment(c, i);
        } else {
            return i;
        }
    }
}

/// Liest ein Escape; `c[*i]` ist das Zeichen nach dem Backslash.
fn read_escape(c: &[char], i: &mut usize) -> char {
    let Some(&first) = c.get(*i) else {
        return '\u{FFFD}';
    };
    if first.is_ascii_hexdigit() {
        let mut value = 0u32;
        let mut digits = 0;
        while digits < 6 && *i < c.len() && c[*i].is_ascii_hexdigit() {
            value = value * 16 + c[*i].to_digit(16).unwrap_or(0);
            *i += 1;
            digits += 1;
        }
        if *i < c.len() && is_css_space(c[*i]) {
            *i += 1;
        }
        match char::from_u32(value) {
            Some(ch) if value != 0 => ch,
            _ => '\u{FFFD}',
        }
    } else {
        *i += 1;
        first
    }
}

/// Liest eine Zeichenkette ab dem öffnenden Anführungszeichen. Rückgabe: Inhalt, ob sie ordnungsgemäß
/// geschlossen wurde, und die Position hinter der Zeichenkette.
fn read_string(c: &[char], start: usize) -> (String, bool, usize) {
    let quote = c[start];
    let mut i = start + 1;
    let mut s = String::new();
    while i < c.len() {
        let ch = c[i];
        if ch == quote {
            return (s, true, i + 1);
        }
        match ch {
            '\n' | '\r' | '\u{c}' => return (s, false, i),
            '\\' => {
                i += 1;
                match c.get(i) {
                    None => return (s, false, i),
                    // Zeilenfortsetzung
                    Some('\n') | Some('\u{c}') => i += 1,
                    Some('\r') => {
                        i += 1;
                        if c.get(i) == Some(&'\n') {
                            i += 1;
                        }
                    }
                    Some(_) => s.push(read_escape(c, &mut i)),
                }
            }
            _ => {
                s.push(ch);
                i += 1;
            }
        }
    }
    (s, false, i)
}

/// Liest den Inhalt von `url(` ab der Position hinter der Klammer. Rückgabe: der Wert (nur bei
/// wohlgeformtem Token) und die Position hinter der schließenden Klammer.
fn read_url_body(c: &[char], start: usize) -> (Option<String>, usize) {
    let n = c.len();
    let mut i = skip_space(c, start);
    if i < n && (c[i] == '"' || c[i] == '\'') {
        let (s, closed, next) = read_string(c, i);
        i = skip_space(c, next);
        if closed && i < n && c[i] == ')' {
            return (Some(s), i + 1);
        }
        while i < n && c[i] != ')' {
            i += 1;
        }
        return (None, (i + 1).min(n));
    }
    let mut s = String::new();
    let mut bad = false;
    while i < n && c[i] != ')' {
        match c[i] {
            ch if is_css_space(ch) => {
                i = skip_space(c, i);
                if i < n && c[i] != ')' {
                    bad = true;
                }
            }
            '"' | '\'' | '(' => {
                bad = true;
                i += 1;
            }
            '\\' => {
                i += 1;
                s.push(read_escape(c, &mut i));
            }
            ch => {
                s.push(ch);
                i += 1;
            }
        }
    }
    let next = (i + 1).min(n);
    (if bad { None } else { Some(s) }, next)
}

/// Bis zum Ende der Anweisung (`;`, `{` oder `}`; nicht verbraucht), Zeichenketten und Kommentare beachtend.
fn skip_statement(c: &[char], mut i: usize) -> usize {
    while i < c.len() {
        match c[i] {
            ';' | '{' | '}' => return i,
            '"' | '\'' => i = read_string(c, i).2,
            '/' if c.get(i + 1) == Some(&'*') => i = skip_comment(c, i),
            _ => i += 1,
        }
    }
    i
}

/// Durchsucht CSS-Text (Datei, `<style>`-Inhalt oder Inhalt eines `style`-Attributs).
fn scan(src: &str) -> Vec<(RefKind, String)> {
    let c: Vec<char> = src.chars().collect();
    let n = c.len();
    let mut out: Vec<(RefKind, String)> = Vec::new();
    let mut emit = |kind: RefKind, raw: &str| {
        let value = raw.trim_matches(is_css_space);
        if value.is_empty() || is_data_uri(value) || value.starts_with('#') {
            return;
        }
        out.push((kind, value.to_string()));
    };

    let mut blocks: Vec<bool> = Vec::new(); // true = @font-face-Block
    let mut font_depth = 0usize;
    let mut at_rule: Option<String> = None;
    let mut stmt_started = false;
    let mut parens = 0usize;
    let mut image_set_depth: Option<usize> = None;
    let mut i = 0usize;

    while i < n {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            i = skip_comment(&c, i);
            continue;
        }
        if is_css_space(ch) {
            i += 1;
            continue;
        }
        match ch {
            '"' | '\'' => {
                let (s, closed, next) = read_string(&c, i);
                i = next;
                if closed && image_set_depth == Some(parens) {
                    emit(RefKind::Image, &s);
                }
                stmt_started = true;
            }
            '{' | '}' | ';' => {
                if ch == '{' {
                    let font = at_rule.as_deref() == Some("font-face");
                    blocks.push(font);
                    if font {
                        font_depth += 1;
                    }
                } else if ch == '}' && blocks.pop() == Some(true) {
                    font_depth = font_depth.saturating_sub(1);
                }
                at_rule = None;
                stmt_started = false;
                parens = 0;
                image_set_depth = None;
                i += 1;
            }
            '(' => {
                parens += 1;
                stmt_started = true;
                i += 1;
            }
            ')' => {
                parens = parens.saturating_sub(1);
                if image_set_depth.is_some_and(|d| parens < d) {
                    image_set_depth = None;
                }
                stmt_started = true;
                i += 1;
            }
            '@' if !stmt_started => {
                let mut j = i + 1;
                let mut name = String::new();
                while j < n && is_ident_char(c[j]) {
                    name.push(c[j].to_ascii_lowercase());
                    j += 1;
                }
                i = j;
                stmt_started = true;
                match name.as_str() {
                    "import" => {
                        i = skip_space(&c, i);
                        if i < n && (c[i] == '"' || c[i] == '\'') {
                            let (s, closed, next) = read_string(&c, i);
                            i = next;
                            if closed {
                                emit(RefKind::Import, &s);
                            }
                        } else if is_url_function(&c, i) {
                            let (value, next) = read_url_body(&c, i + 4);
                            i = next;
                            if let Some(v) = value {
                                emit(RefKind::Import, &v);
                            }
                        }
                    }
                    "namespace" | "document" | "-moz-document" | "supports" => {
                        i = skip_statement(&c, i);
                    }
                    _ => {}
                }
                at_rule = Some(name);
            }
            _ if is_ident_char(ch) || ch == '\\' => {
                // Maximaler Bezeichner (inkl. Escapes), damit `myurl(` nicht als `url(` gilt.
                let mut name = String::new();
                while i < n {
                    if c[i] == '\\' {
                        i += 1;
                        name.push(read_escape(&c, &mut i));
                    } else if is_ident_char(c[i]) {
                        name.push(c[i]);
                        i += 1;
                    } else {
                        break;
                    }
                }
                stmt_started = true;
                if c.get(i) == Some(&'(') {
                    let lower = name.to_ascii_lowercase();
                    match lower.as_str() {
                        "url" => {
                            let (value, next) = read_url_body(&c, i + 1);
                            i = next;
                            if let Some(v) = value {
                                let kind = if font_depth > 0 {
                                    RefKind::Font
                                } else {
                                    RefKind::Image
                                };
                                emit(kind, &v);
                            }
                        }
                        "image-set" | "-webkit-image-set" => {
                            parens += 1;
                            image_set_depth = Some(parens);
                            i += 1;
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                stmt_started = true;
                i += 1;
            }
        }
    }
    out
}

/// Steht an Position `i` ein `url(` (case-insensitiv)?
fn is_url_function(c: &[char], i: usize) -> bool {
    c.len() >= i + 4
        && c[i..i + 3]
            .iter()
            .collect::<String>()
            .eq_ignore_ascii_case("url")
        && c[i + 3] == '('
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(css: &str) -> Vec<(RefKind, String)> {
        scan(css)
    }

    #[test]
    fn url_forms() {
        let r = kinds(
            "a{background:url(a.png)} b{background:url('b.png')} c{background:url( \"c.png\" )}",
        );
        let urls: Vec<&str> = r.iter().map(|(_, u)| u.as_str()).collect();
        assert_eq!(urls, ["a.png", "b.png", "c.png"]);
        assert!(r.iter().all(|(k, _)| *k == RefKind::Image));
    }

    #[test]
    fn imports() {
        let r = kinds("@charset \"utf-8\";\n@import url(\"b.css\");\n@import 'c.css' screen;\n@import URL(d.css);");
        assert_eq!(
            r,
            [
                (RefKind::Import, "b.css".to_string()),
                (RefKind::Import, "c.css".to_string()),
                (RefKind::Import, "d.css".to_string())
            ]
        );
    }

    #[test]
    fn font_face_and_skipped_urls() {
        let css = "@namespace svg url(http://www.w3.org/2000/svg);\n@font-face{font-family:F;src:url(f.woff2) format('woff2'),url(f.woff)}\n.a{background:url(data:image/png;base64,AAA=)}\n.b{fill:url(#grad)}\n.c{background:url(bg.png)}\n@supports (background:url(x.png)){.d{background:url(d.png)}}";
        let r = kinds(css);
        assert_eq!(
            r,
            [
                (RefKind::Font, "f.woff2".to_string()),
                (RefKind::Font, "f.woff".to_string()),
                (RefKind::Image, "bg.png".to_string()),
                (RefKind::Image, "d.png".to_string()),
            ]
        );
    }

    #[test]
    fn comments_strings_and_lookalikes() {
        let css = "/* url(no.png) */ .a{content:\"url(no2.png)\";background:url(yes.png)} .b{x:myurl(no3.png)}";
        let r = kinds(css);
        assert_eq!(r, [(RefKind::Image, "yes.png".to_string())]);
    }

    #[test]
    fn image_set_strings() {
        let r = kinds(".a{background-image:image-set(\"a.png\" 1x, url(b.png) 2x)}");
        let urls: Vec<&str> = r.iter().map(|(_, u)| u.as_str()).collect();
        assert_eq!(urls, ["a.png", "b.png"]);
    }

    #[test]
    fn escapes_and_bad_urls() {
        assert_eq!(kinds(".a{x:url(a\\20 b.png)}")[0].1, "a b.png");
        assert!(kinds(".a{x:url(a b.png)}").is_empty());
        assert!(kinds(".a{x:url('open.png)}").is_empty());
    }

    #[test]
    fn style_attribute_without_braces() {
        let doc = Document::parse("<body><div style=\"background:url(bg.png); color:red\">x</div>");
        let base = url::Url::parse("https://site.test/p/").unwrap();
        let links = extract_inline(&doc, &base);
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].resolved.as_deref(),
            Some("https://site.test/p/bg.png")
        );
        assert_eq!(links[0].xpath, "/html[1]/body[1]/div[1]");
    }
}
