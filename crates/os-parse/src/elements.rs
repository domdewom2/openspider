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

use crate::content::{is_block_element, XHTML_NS};
use crate::dom::{self, Document};
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
                    } else if child.id() != node.id() && is_block_element(name) {
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
                    } else if child.id() != node.id() && is_block_element(e.name()) {
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
                if name.eq_ignore_ascii_case("description") {
                    Kind::MetaDescription
                } else if name.eq_ignore_ascii_case("keywords") {
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
