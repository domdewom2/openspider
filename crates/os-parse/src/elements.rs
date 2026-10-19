//! Onpage-Elemente: Titel, Meta-Descriptions, Meta-Keywords, H1, H2 (ONPAGE-090, -100, -110, -120, -130,
//! Mehrfachvorkommen ONPAGE-008).
//!
//! Gesammelt werden **alle** Vorkommen in Dokumentreihenfolge (im `<head>` und außerhalb, ungedeckelt); die
//! Deckelung auf zwei Vorkommen für Titel/Meta-Description/Meta-Keywords nimmt die Auswertung nach dem
//! Speichern vor (`Occurrences` = `min(len, 2)`, bei H1/H2 die volle Länge). Elemente innerhalb von
//! `<template>` werden ignoriert. `<noscript>`-Inhalt ist normales Markup (Tree-Builder ohne Scripting).
//!
//! Textnormalisierung: Texte und Attributwerte kommen bereits entity-dekodiert aus dem Tree-Builder und
//! werden daher nur mit [`text::normalize_ws`] normalisiert (nie mit `normalize`, sonst Doppeldekodierung).

use ego_tree::iter::Edge;
use os_model::ExtractedText;
use scraper::{ElementRef, Node};

use crate::dom::{self, is_block, Document, XHTML_NS};
use crate::text;

/// Vollständige Listen der erfassten Elemente in Dokumentreihenfolge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementSets {
    /// `<title>`.
    pub titles: Vec<ExtractedText>,
    /// `<meta name="description">`.
    pub meta_descriptions: Vec<ExtractedText>,
    /// `<meta name="keywords">`.
    pub meta_keywords: Vec<ExtractedText>,
    /// `<h1>`.
    pub h1: Vec<ExtractedText>,
    /// `<h2>`.
    pub h2: Vec<ExtractedText>,
}

fn extracted(raw: &str, in_head: bool) -> ExtractedText {
    let text = text::normalize_ws(raw);
    ExtractedText {
        len: text::char_len(&text),
        text,
        px: None,
        in_head,
    }
}

/// Text einer Überschrift: Textknoten und `alt` von `<img>` (A-30) in Dokumentreihenfolge; `<br>` und
/// Blockgrenzen sind Leerraum (A-31); `script`, `style`, `noscript`, `template` und `svg` zählen nicht.
/// Versteckte Überschriften werden nicht anders behandelt (keine Sichtbarkeitsprüfung).
///
/// Iterativ (`ego_tree::traverse`, wie `content::walk`), damit auch pathologisch tief verschachtelte
/// Überschriften keinen Stapelüberlauf auslösen.
fn heading_text(node: ElementRef) -> String {
    let mut out = String::new();
    // > 0: innerhalb eines ausgeschlossenen Teilbaums (Tiefe relativ zu dessen Wurzel).
    let mut skip_depth = 0usize;
    for edge in node.traverse() {
        match edge {
            Edge::Open(child) => match child.value() {
                Node::Element(e) => {
                    if skip_depth > 0 {
                        skip_depth += 1;
                        continue;
                    }
                    let name = e.name();
                    if matches!(name, "script" | "style" | "noscript" | "template" | "svg") {
                        skip_depth = 1;
                    } else if name == "img" {
                        if let Some(alt) = e.attr("alt") {
                            out.push_str(alt);
                        }
                    } else if child.id() != node.id() && is_block(name) {
                        out.push(' ');
                    }
                }
                Node::Text(t) if skip_depth == 0 => out.push_str(t),
                _ => {}
            },
            Edge::Close(child) => {
                if let Node::Element(e) = child.value() {
                    if skip_depth > 0 {
                        skip_depth -= 1;
                    } else if child.id() != node.id() && is_block(e.name()) {
                        out.push(' ');
                    }
                }
            }
        }
    }
    out
}

/// Text des `<title>`: nur Textknoten (RCDATA – Tags im Titel sind literaler Text).
fn title_text(node: ElementRef) -> String {
    let mut out = String::new();
    for child in node.children() {
        if let Node::Text(t) = child.value() {
            out.push_str(t);
        }
    }
    out
}

/// Extrahiert die Elemente.
pub fn extract(doc: &Document) -> ElementSets {
    let mut sets = ElementSets::default();
    for node in doc.html().tree.root().descendants() {
        let Some(el) = ElementRef::wrap(node) else {
            continue;
        };
        let value = el.value();
        if &*value.name.ns != XHTML_NS {
            continue;
        }
        let kind = match value.name() {
            "title" => Kind::Title,
            "h1" => Kind::H1,
            "h2" => Kind::H2,
            "meta" => {
                let Some(name) = value.attr("name") else {
                    continue;
                };
                if name.trim().eq_ignore_ascii_case("description") {
                    Kind::MetaDescription
                } else if name.trim().eq_ignore_ascii_case("keywords") {
                    Kind::MetaKeywords
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        if dom::in_template(el) {
            continue;
        }
        let in_head = dom::in_head(el);
        match kind {
            Kind::Title => sets.titles.push(extracted(&title_text(el), in_head)),
            Kind::H1 => sets.h1.push(extracted(&heading_text(el), in_head)),
            Kind::H2 => sets.h2.push(extracted(&heading_text(el), in_head)),
            Kind::MetaDescription => sets
                .meta_descriptions
                .push(extracted(value.attr("content").unwrap_or(""), in_head)),
            Kind::MetaKeywords => sets
                .meta_keywords
                .push(extracted(value.attr("content").unwrap_or(""), in_head)),
        }
    }
    sets
}

#[derive(Clone, Copy)]
enum Kind {
    Title,
    MetaDescription,
    MetaKeywords,
    H1,
    H2,
}

/// Annotates DOM facts independently of the validation analyzer.
pub fn annotate_facts(doc: &Document, cfg: &crate::ParseConfig, page: &mut os_model::ParsedPage) {
    use os_model::{ExtKey, Value};
    let mut first_level = None;
    let mut has_h1 = false;
    let mut has_h2 = false;
    let mut deep_before_h2 = false;
    let mut title_outside = false;
    let mut description_outside = false;
    let mut alt = false;
    for node in doc.html().tree.root().descendants() {
        let Some(el) = ElementRef::wrap(node) else {
            continue;
        };
        if &*el.value().name.ns != XHTML_NS || dom::in_template(el) {
            continue;
        }
        let name = el.value().name();
        if matches!(name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
            let level = name.as_bytes()[1];
            first_level.get_or_insert(level);
            has_h1 |= level == b'1';
            has_h2 |= level == b'2';
            deep_before_h2 |= !has_h2 && level > b'2';
        }
        if cfg.html_validation && !dom::in_head(el) {
            title_outside |= name == "title";
            description_outside |= name == "meta"
                && el
                    .value()
                    .attr("name")
                    .is_some_and(|n| n.trim().eq_ignore_ascii_case("description"));
        }
        if name == "img"
            && el
                .value()
                .attr("alt")
                .is_some_and(|a| !text::normalize_ws(a).is_empty())
        {
            alt |= el
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|a| a.value().name() == "h1");
        }
    }
    let h1_seq = has_h1 && first_level != Some(b'1');
    let h2_seq = has_h2 && deep_before_h2;
    for (key, value) in [
        ("head.h1_alt", alt),
        ("head.h1_non_sequential", h1_seq),
        ("head.h2_non_sequential", h2_seq),
        // Only the first two values are stored as columns. Missing must still
        // distinguish a nonempty third (or later) occurrence from all-empty tags.
        (
            "head.meta_keywords_nonempty",
            page.meta_keywords
                .iter()
                .any(|value| !value.text.is_empty()),
        ),
    ] {
        page.ext
            .push((ExtKey::from_static(key), Value::Bool(value)));
    }
    if cfg.html_validation {
        for (key, value) in [
            ("head.title_outside", title_outside),
            ("head.description_outside", description_outside),
        ] {
            page.ext
                .push((ExtKey::from_static(key), Value::Bool(value)));
        }
    }
}
