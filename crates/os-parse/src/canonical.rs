//! Canonical-Angaben (`<link rel=canonical>`, TECH-010 ff.).
//!
//! Erfasst werden `<link>`-Elemente mit dem `rel`-Token `canonical` und einem `href`-Attribut. Der HTML5-Parser
//! liefert Element- und Attributnamen unabhängig von der Schreibweise im Quelltext; Kommentare sind keine Elemente,
//! der Inhalt eines `<template>` zählt nicht, `<noscript>` im Head wird (Textmodus) wie normales Markup erfasst.
//! `in_head` entscheidet der Tree-Builder (`dom::in_head`, TECH-003) und wird immer berechnet.
//!
//! Die gemeinsamen Hilfen (`link_elements`, `has_rel_token`, `raw_and_resolved`) nutzen auch `hreflang` und
//! `pagination`.

use std::sync::OnceLock;

use os_model::{CanonicalRef, HintSource};
use scraper::{ElementRef, Selector};

use crate::base::{clean_url_text, resolve_string};
use crate::dom::{self, Document};

/// Alle `<link>`-Elemente des HTML-Namensraums in Dokumentreihenfolge, ohne `<template>`-Inhalt.
pub(crate) fn link_elements(doc: &Document) -> impl Iterator<Item = ElementRef<'_>> {
    static SEL: OnceLock<Selector> = OnceLock::new();
    let sel = SEL.get_or_init(|| Selector::parse("link").expect("gültiger Selektor"));
    doc.html()
        .select(sel)
        .filter(|el| crate::links::is_html(*el) && !dom::in_template(*el))
}

/// Enthält der `rel`-Wert (Tokenliste, ASCII-Leerraum-getrennt, ohne Beachtung der Schreibweise) das Token?
pub(crate) fn has_rel_token(rel: &str, token: &str) -> bool {
    rel.split(|c: char| c.is_ascii_whitespace())
        .any(|t| t.eq_ignore_ascii_case(token))
}

/// Rohwert (ohne umgebende Steuerzeichen/Leerzeichen) und gegen die Basis aufgelöste `http`/`https`-URL
/// (TECH-002). Ein leerer Wert löst sich zur Basis-URL selbst auf; nicht auflösbare Werte bleiben ohne URL.
pub(crate) fn raw_and_resolved(base: &url::Url, href: &str) -> (String, Option<String>) {
    let raw = href.trim_matches(|c: char| c <= ' ').to_string();
    let resolved = resolve_string(base, &clean_url_text(href));
    (raw, resolved)
}

/// Extrahiert alle Canonical-Angaben aus `<link>`-Elementen (TECH-010).
pub fn extract(doc: &Document, base: &url::Url) -> Vec<CanonicalRef> {
    let mut out = Vec::new();
    for el in link_elements(doc) {
        let Some(rel) = el.value().attr("rel") else {
            continue;
        };
        if !has_rel_token(rel, "canonical") {
            continue;
        }
        let Some(href) = el.value().attr("href") else {
            continue;
        };
        let (raw, resolved) = raw_and_resolved(base, href);
        out.push(CanonicalRef {
            source: HintSource::Html,
            raw,
            resolved,
            in_head: dom::in_head(el),
            order: out.len() as u32,
            xpath: Some(dom::xpath(el)),
            ..Default::default()
        });
    }
    out
}

// STUB(M2-D04)
pub fn response_facts(
    _input: &crate::ResponseInput<'_>,
) -> Vec<(os_model::ExtKey, os_model::Value)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_tokens_are_case_insensitive_and_whole_words() {
        assert!(has_rel_token("Canonical", "canonical"));
        assert!(has_rel_token("canonical alternate", "canonical"));
        assert!(has_rel_token("  alternate\tCANONICAL ", "canonical"));
        assert!(!has_rel_token("canonicalx", "canonical"));
        assert!(!has_rel_token("", "canonical"));
    }
}
