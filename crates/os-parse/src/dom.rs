//! HTML-Dokumentbaum (html5ever über `scraper`), TECH-003.
//!
//! Der Baum lebt nur während der Verarbeitung einer Seite (Architekturgrundsatz 3). Ob ein Element im
//! `<head>` liegt, entscheidet allein der HTML5-Tree-Builder: Ein `<img>`, `<div>` oder Text im Head
//! beendet ihn implizit, ein zweites `<head>` vor `<body>` wird ignoriert, seine Kinder landen im
//! ersten Head.
//!
//! Der Tree-Builder läuft mit `scripting_enabled = false` (wie ein Browser ohne JavaScript): Der Inhalt von
//! `<noscript>` wird als Elemente geparst und ist damit für die Extraktion sichtbar (TECH-017, TECH-008,
//! CRAWL-052 Q9). Ein `<noscript>` im Head mit Nicht-Head-Inhalt (`<img>`, `<div>`, Text) schließt den Head
//! wie jedes andere Element.

use std::sync::OnceLock;

use html5ever::driver::{self, ParseOpts};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use scraper::{ElementRef, Html, HtmlTreeSink, Selector};

/// HTML-Namensraum (gemeinsam genutzt von `content`, `elements` und `links`).
pub(crate) const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// Ein geparstes HTML-Dokument.
pub struct Document {
    html: Html,
}

impl Document {
    /// Parst `html` nach dem WHATWG-Algorithmus (immer erfolgreich; Fehler werden wie im Browser repariert),
    /// mit `scripting_enabled = false`: `<noscript>`-Inhalt wird zu Elementen (TECH-017, CRAWL-052 Q9).
    pub fn parse(html: &str) -> Self {
        let opts = ParseOpts {
            tree_builder: TreeBuilderOpts {
                scripting_enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let parser = driver::parse_document(HtmlTreeSink::new(Html::new_document()), opts);
        Document {
            html: parser.one(html),
        }
    }

    /// Zugriff auf den zugrunde liegenden `scraper`-Baum (für Selektoren und Traversierung).
    pub fn html(&self) -> &Html {
        &self.html
    }

    /// Wert des ersten `<base href>` im Dokument (Baumreihenfolge, `<template>`-Inhalt zählt nicht),
    /// ohne umgebenden ASCII-Leerraum. `None`, wenn es kein `<base>` mit `href` gibt.
    pub fn base_href(&self) -> Option<String> {
        static SEL: OnceLock<Selector> = OnceLock::new();
        let sel = SEL.get_or_init(|| Selector::parse("base[href]").expect("gültiger Selektor"));
        self.html
            .select(sel)
            .find(|e| !in_template(*e))
            .and_then(|e| e.value().attr("href"))
            .map(|v| {
                v.trim_matches(|c: char| c.is_ascii_whitespace())
                    .to_string()
            })
    }
}

fn is_html_element_named(node: &ego_tree::NodeRef<scraper::Node>, name: &str) -> bool {
    node.value()
        .as_element()
        .is_some_and(|e| e.name() == name && &*e.name.ns == XHTML_NS)
}

/// Liegt das Element im `<head>` (Nachfahre des `head`-Elements im HTML5-Baum)? Das `head`-Element selbst
/// liegt nicht in sich selbst.
pub fn in_head(node: ElementRef) -> bool {
    node.ancestors().any(|a| is_html_element_named(&a, "head"))
}

/// Liegt das Element innerhalb eines `<template>`? Solche Elemente werden für die SEO-Extraktion ignoriert.
pub fn in_template(node: ElementRef) -> bool {
    node.ancestors()
        .any(|a| is_html_element_named(&a, "template"))
}

/// Absoluter XPath des Elements mit durchgängigen Positionsindizes (1-basiert unter gleichnamigen
/// Geschwistern, auch bei nur einem Geschwister), z. B. `/html[1]/body[1]/main[1]/p[2]/a[1]`. Das ist die
/// verbindliche Form der Spalte `Link Path` (OUT-031); derselbe Quelltext ergibt immer denselben Pfad.
pub fn xpath(node: ElementRef) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(*node);
    while let Some(n) = current {
        if let Some(el) = n.value().as_element() {
            let name = el.name();
            let mut position = 1usize;
            if let Some(parent) = n.parent() {
                let mut same = 0usize;
                for sibling in parent.children() {
                    if sibling
                        .value()
                        .as_element()
                        .is_some_and(|se| se.name() == name)
                    {
                        same += 1;
                        if sibling.id() == n.id() {
                            position = same;
                            break;
                        }
                    }
                }
            }
            segments.push(format!("{name}[{position}]"));
        }
        current = n.parent();
    }
    segments.reverse();
    format!("/{}", segments.join("/"))
}

/// Elemente, deren Inhalt nie als sichtbarer Text zählt (ONPAGE-140).
fn is_hidden_content(name: &str) -> bool {
    matches!(name, "script" | "style" | "noscript" | "template" | "svg")
}

/// Elemente, die einen Block (Zeilenumbruch) bilden; `br` erzeugt ebenfalls einen Umbruch.
pub(crate) fn is_block(name: &str) -> bool {
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

fn collect_text(node: ego_tree::NodeRef<scraper::Node>, block_separators: bool, out: &mut String) {
    for child in node.children() {
        match child.value() {
            scraper::Node::Text(t) => out.push_str(t),
            scraper::Node::Element(e) => {
                let name = e.name();
                if is_hidden_content(name) {
                    continue;
                }
                let block = block_separators && is_block(name);
                if block {
                    out.push(' ');
                }
                collect_text(child, block_separators, out);
                if block {
                    out.push(' ');
                }
            }
            _ => {}
        }
    }
}

/// Textinhalt des Elements in Dokumentreihenfolge, ohne `script`, `style`, `noscript`, `template`, `svg`
/// und Kommentare. Mit `block_separators` erzeugen Blockgrenzen (`p`, `div`, `li`, `td`, `h1`–`h6`, `br`,
/// `section`, `article` …) je ein Leerzeichen, damit `<p>a</p><p>b</p>` zu „a b“ wird (ONPAGE-140).
/// Der Text ist nicht normalisiert und enthält bereits dekodierte Zeichen (der Tree-Builder hat Entities
/// aufgelöst): Aufrufer wenden [`crate::text::normalize_ws`] an, **nicht** [`crate::text::normalize`] – sonst
/// würde `&amp;amp;` im Quelltext doppelt zu `&` dekodiert.
pub fn text_content(node: ElementRef, block_separators: bool) -> String {
    let mut out = String::new();
    let name = node.value().name();
    if is_hidden_content(name) {
        return out;
    }
    collect_text(*node, block_separators, &mut out);
    out
}
