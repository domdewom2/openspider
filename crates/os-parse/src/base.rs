//! Basis-URL des Dokuments (`<base href>`) und gemeinsame Hilfen zum Auflösen von Verweisen (CRAWL-002).

use os_model::PathType;
use scraper::Selector;
use std::sync::OnceLock;

use crate::dom::{self, Document};

/// Löst die Basis-URL auf (CRAWL-002).
///
/// Basis ist das erste **gültige** `<base href>` im Dokument, auch außerhalb von `<head>` (Browser-Verhalten);
/// `<base>`-Elemente in einem `<template>` zählen nicht. Ein relatives `href` wird gegen die Dokument-URL
/// aufgelöst. Gültig heißt: Das Ergebnis ist eine `http`/`https`-URL mit Host. Ein ungültiges `<base>`
/// (z. B. `mailto:` oder nicht parsbar) wird übersprungen; ab dem ersten gültigen wird der Rest ignoriert.
/// Ohne gültiges `<base>` ist die Basis die Dokument-URL.
pub fn resolve_base(doc: &Document, url: &url::Url) -> url::Url {
    static SEL: OnceLock<Selector> = OnceLock::new();
    let sel = SEL.get_or_init(|| Selector::parse("base[href]").expect("gültiger Selektor"));
    for el in doc.html().select(sel) {
        if dom::in_template(el) {
            continue;
        }
        let Some(href) = el.value().attr("href") else {
            continue;
        };
        let cleaned = clean_url_text(href);
        if let Some(resolved) = resolve_http(url, &cleaned) {
            return resolved;
        }
    }
    url.clone()
}

/// Entfernt aus einem Attributwert, was der URL-Parser des Browsers entfernt: führende und abschließende
/// C0-Steuerzeichen/Leerzeichen sowie Tabulatoren und Zeilenumbrüche an jeder Stelle (WHATWG-URL-Standard).
pub(crate) fn clean_url_text(raw: &str) -> String {
    let trimmed = raw.trim_matches(|c: char| c <= ' ');
    trimmed
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// Pfadart eines bereinigten (`clean_url_text`) Verweises.
pub(crate) fn path_type(cleaned: &str) -> PathType {
    if has_scheme(cleaned) {
        return PathType::Absolute;
    }
    let mut chars = cleaned.chars();
    let first = chars.next();
    let second = chars.next();
    let is_slash = |c: Option<char>| matches!(c, Some('/') | Some('\\'));
    if is_slash(first) && is_slash(second) {
        PathType::ProtocolRelative
    } else if is_slash(first) {
        PathType::RootRelative
    } else {
        PathType::PathRelative
    }
}

/// Beginnt der Text mit einem URL-Schema (`[A-Za-z][A-Za-z0-9+.-]*:`)?
fn has_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return false;
        }
    }
    false
}

/// Löst einen bereinigten Verweis gegen die Basis auf. Liefert nur `http`/`https`-URLs mit Host; alles andere
/// (`mailto:`, `tel:`, `javascript:`, `data:`, `about:`, `file:`, nicht parsbare Werte) ist nicht crawlbar
/// und ergibt `None`.
pub(crate) fn resolve_http(base: &url::Url, cleaned: &str) -> Option<url::Url> {
    let joined = base.join(cleaned).ok()?;
    if matches!(joined.scheme(), "http" | "https") && joined.host_str().is_some() {
        Some(joined)
    } else {
        None
    }
}

/// Wie [`resolve_http`], als Zeichenkette.
pub(crate) fn resolve_string(base: &url::Url, cleaned: &str) -> Option<String> {
    resolve_http(base, cleaned).map(String::from)
}

/// Beginnt der bereinigte Wert mit `data:` (ASCII-case-insensitiv)?
pub(crate) fn is_data_uri(cleaned: &str) -> bool {
    cleaned
        .get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn clean_strips_controls_and_inner_whitespace() {
        assert_eq!(clean_url_text("  /a\tb\nc\r "), "/abc");
        assert_eq!(clean_url_text("\u{1}\u{2} x"), "x");
    }

    #[test]
    fn path_types() {
        assert_eq!(path_type("https://a.test/"), PathType::Absolute);
        assert_eq!(path_type("mailto:x@y.test"), PathType::Absolute);
        assert_eq!(path_type("//cdn.test/x"), PathType::ProtocolRelative);
        assert_eq!(path_type("/x"), PathType::RootRelative);
        assert_eq!(path_type("x/y"), PathType::PathRelative);
        assert_eq!(path_type("?p=2"), PathType::PathRelative);
        assert_eq!(path_type("#a"), PathType::PathRelative);
        assert_eq!(path_type(""), PathType::PathRelative);
        assert_eq!(path_type("a/b:c"), PathType::PathRelative);
    }

    #[test]
    fn only_http_resolves() {
        let base = u("https://site.test/dir/page.html");
        assert_eq!(
            resolve_string(&base, "a.html").as_deref(),
            Some("https://site.test/dir/a.html")
        );
        assert_eq!(resolve_string(&base, "mailto:a@b.test"), None);
        assert_eq!(resolve_string(&base, "javascript:void(0)"), None);
        assert_eq!(resolve_string(&base, "http://site test/"), None);
    }

    #[test]
    fn base_without_element_is_document_url() {
        let doc = Document::parse("<p>x</p>");
        let url = u("https://site.test/a/b.html");
        assert_eq!(resolve_base(&doc, &url), url);
    }
}
