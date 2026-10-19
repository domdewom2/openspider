//! Links aus dem Dokument (CRAWL-052, CRAWL-041, TECH-131): Hyperlinks und eingebundene Ressourcen.
//!
//! Der HTML5-Parser liefert Element- und Attributnamen unabhängig von der Schreibweise im Quelltext und
//! repariert kaputtes Markup wie ein Browser. Kommentare und `<script>`-Inhalte sind keine Elemente; der
//! Inhalt eines `<template>` wird übersprungen; `<noscript>` ist im Textmodus normales Markup (dom.rs).
//!
//! Nicht hier: Canonical, Hreflang, Next/Prev, AMP, Mobile Alternate und Meta Refresh (P06), `srcset` und
//! Bildinstanzen (`images`), `url()` aus CSS (`css`).

use ego_tree::iter::Edge;
use os_model::{ExtractedLink, LinkPosition, LinkType};
use scraper::ElementRef;

use crate::base::{clean_url_text, is_data_uri, path_type, resolve_string};
use crate::dom::{self, Document, XHTML_NS};
use crate::{text, ParseConfig};

const SVG_NS: &str = "http://www.w3.org/2000/svg";

/// Element im HTML-Namensraum?
pub(crate) fn is_html(el: ElementRef) -> bool {
    &*el.value().name.ns == XHTML_NS
}

fn is_svg(el: ElementRef) -> bool {
    &*el.value().name.ns == SVG_NS
}

/// Läuft in Dokumentreihenfolge über alle Elemente außerhalb von `<template>` und ruft `f` mit dem Element,
/// seiner laufenden Nummer (Pre-Order-Index über alle Elemente, 0-basiert – das ist `doc_order`) und der
/// Linkposition auf (nächster Vorfahre `head`/`nav`/`header`/`main`/`aside`/`footer`, sonst Content).
pub(crate) fn walk<'a>(doc: &'a Document, mut f: impl FnMut(ElementRef<'a>, u32, LinkPosition)) {
    let mut ordinal: u32 = 0;
    let mut template_depth = 0usize;
    let mut landmarks: Vec<LinkPosition> = Vec::new();
    for edge in doc.html().tree.root().traverse() {
        match edge {
            Edge::Open(node) => {
                let Some(el) = ElementRef::wrap(node) else {
                    continue;
                };
                let current = ordinal;
                ordinal = ordinal.saturating_add(1);
                if is_html(el) && el.value().name() == "template" {
                    template_depth += 1;
                    continue;
                }
                if template_depth > 0 {
                    continue;
                }
                let position = landmarks.last().cloned().unwrap_or(LinkPosition::Content);
                f(el, current, position);
                if let Some(landmark) = landmark_of(el) {
                    landmarks.push(landmark);
                }
            }
            Edge::Close(node) => {
                let Some(el) = ElementRef::wrap(node) else {
                    continue;
                };
                if is_html(el) && el.value().name() == "template" {
                    template_depth = template_depth.saturating_sub(1);
                } else if template_depth == 0 && landmark_of(el).is_some() {
                    landmarks.pop();
                }
            }
        }
    }
}

fn landmark_of(el: ElementRef) -> Option<LinkPosition> {
    if !is_html(el) {
        return None;
    }
    match el.value().name() {
        // alles im `<head>` hat die Position Head (OUT-031, TECH-131, K-63)
        "head" => Some(LinkPosition::Head),
        "nav" => Some(LinkPosition::Navigation),
        "header" => Some(LinkPosition::Header),
        "main" => Some(LinkPosition::Content),
        "aside" => Some(LinkPosition::Sidebar),
        "footer" => Some(LinkPosition::Footer),
        _ => None,
    }
}

/// `rel` eines Elements: in Kleinbuchstaben, Tokens in Originalreihenfolge durch ein Leerzeichen getrennt.
/// Zweiter Wert: enthält `nofollow`, `ugc` oder `sponsored`; dritter: die Tokens.
fn rel_info(el: ElementRef) -> (Option<String>, bool, Vec<String>) {
    let Some(raw) = el.value().attr("rel") else {
        return (None, false, Vec::new());
    };
    let tokens: Vec<String> = raw
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if tokens.is_empty() {
        return (None, false, tokens);
    }
    let nofollow = tokens
        .iter()
        .any(|t| matches!(t.as_str(), "nofollow" | "ugc" | "sponsored"));
    (Some(tokens.join(" ")), nofollow, tokens)
}

/// Normalisierter, nicht leerer Attributwert.
fn attr_text(el: ElementRef, name: &str) -> Option<String> {
    let value = text::normalize_ws(el.value().attr(name)?);
    (!value.is_empty()).then_some(value)
}

struct Builder<'u> {
    doc: &'u Document,
    base: &'u url::Url,
}

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn link(
        &self,
        link_type: LinkType,
        el: ElementRef,
        raw: &str,
        ordinal: u32,
        position: LinkPosition,
        rel: (Option<String>, bool),
        anchor: Option<String>,
        alt_text: Option<String>,
        target: Option<String>,
    ) -> ExtractedLink {
        let cleaned = clean_url_text(raw);
        ExtractedLink {
            link_type,
            href_raw: raw.trim_matches(|c: char| c <= ' ').to_string(),
            resolved: resolve_string(self.base, &cleaned),
            anchor,
            alt_text,
            rel: rel.0,
            nofollow_rel: rel.1,
            target,
            path_type: path_type(&cleaned),
            xpath: crate::link_position::element_path(self.doc, el),
            position,
            in_head: dom::in_head(el),
            doc_order: ordinal,
            ..Default::default()
        }
    }

    /// Ressource ohne Anker: leere Werte und `data:`-URIs sind keine Verweise.
    fn resource(
        &self,
        link_type: LinkType,
        el: ElementRef,
        raw: &str,
        ordinal: u32,
        position: LinkPosition,
        alt_text: Option<String>,
    ) -> Option<ExtractedLink> {
        let cleaned = clean_url_text(raw);
        if cleaned.is_empty() || is_data_uri(&cleaned) {
            return None;
        }
        let (rel, nofollow, _) = rel_info(el);
        Some(self.link(
            link_type,
            el,
            raw,
            ordinal,
            position,
            (rel, nofollow),
            None,
            alt_text,
            None,
        ))
    }
}

/// Ankertext und Alt-Text eines `<a>`: Ankertext ist N(Textinhalt); nur wenn er leer ist und der Link ein
/// `<img>` mit nicht leerem `alt` enthält, wird dessen `alt` als Alt-Text geführt (TECH-131). `aria-label`
/// und `title` zählen nicht.
fn anchor_and_alt(a: ElementRef) -> (Option<String>, Option<String>) {
    let anchor = text::normalize_ws(&dom::text_content(a, true));
    if !anchor.is_empty() {
        return (Some(anchor), None);
    }
    let alt = a
        .descendants()
        .filter_map(ElementRef::wrap)
        .filter(|e| is_html(*e) && e.value().name() == "img")
        .find_map(|img| attr_text(img, "alt"));
    (None, alt)
}

/// Bildtyp aus `as` bei `rel=preload|prefetch`.
fn type_for_as(as_value: Option<&str>) -> LinkType {
    match as_value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("style") => LinkType::Css,
        Some("script" | "worker" | "sharedworker") => LinkType::JavaScript,
        Some("image") => LinkType::Image,
        Some("audio" | "video" | "track") => LinkType::Media,
        _ => LinkType::Other,
    }
}

/// Extrahiert alle Links in Dokumentreihenfolge.
pub fn extract(doc: &Document, base: &url::Url, cfg: &ParseConfig) -> Vec<ExtractedLink> {
    let builder = Builder { base, doc };
    let mut out: Vec<ExtractedLink> = Vec::new();
    walk(doc, |el, ordinal, position| {
        let html = is_html(el);
        let name = el.value().name();
        let v = el.value();
        match name {
            // Hyperlinks (auch `<a>` in Inline-SVG)
            "a" if html || is_svg(el) => {
                if let Some(href) = v.attr("href") {
                    let (rel, nofollow, _) = rel_info(el);
                    let (anchor, alt) = anchor_and_alt(el);
                    out.push(builder.link(
                        LinkType::Hyperlink,
                        el,
                        href,
                        ordinal,
                        position,
                        (rel, nofollow),
                        anchor,
                        alt,
                        attr_text(el, "target"),
                    ));
                }
            }
            "area" if html => {
                if let Some(href) = v.attr("href") {
                    let (rel, nofollow, _) = rel_info(el);
                    out.push(builder.link(
                        LinkType::Hyperlink,
                        el,
                        href,
                        ordinal,
                        position,
                        (rel, nofollow),
                        None,
                        attr_text(el, "alt"),
                        attr_text(el, "target"),
                    ));
                }
            }
            // Bilder
            "img" if html => {
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(
                        LinkType::Image,
                        el,
                        src,
                        ordinal,
                        position,
                        attr_text(el, "alt"),
                    ));
                }
            }
            "input" if html => {
                let is_image = v
                    .attr("type")
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case("image"));
                if let (true, Some(src)) = (is_image, v.attr("src")) {
                    out.extend(builder.resource(
                        LinkType::Image,
                        el,
                        src,
                        ordinal,
                        position,
                        attr_text(el, "alt"),
                    ));
                }
            }
            "video" if html => {
                if let Some(poster) = v.attr("poster") {
                    out.extend(builder.resource(
                        LinkType::Image,
                        el,
                        poster,
                        ordinal,
                        position.clone(),
                        None,
                    ));
                }
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(LinkType::Media, el, src, ordinal, position, None));
                }
            }
            // Medien
            "audio" | "source" | "track" if html => {
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(LinkType::Media, el, src, ordinal, position, None));
                }
            }
            // JavaScript
            "script" if html => {
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(
                        LinkType::JavaScript,
                        el,
                        src,
                        ordinal,
                        position.clone(),
                        None,
                    ));
                }
            }
            // Frames
            "iframe" | "frame" if html => {
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(
                        LinkType::Iframe,
                        el,
                        src,
                        ordinal,
                        position.clone(),
                        None,
                    ));
                }
            }
            // Flash/Plugins
            "object" if html => {
                if let Some(data) = v.attr("data") {
                    out.extend(builder.resource(
                        LinkType::Other,
                        el,
                        data,
                        ordinal,
                        position.clone(),
                        None,
                    ));
                }
            }
            "embed" if html => {
                if let Some(src) = v.attr("src") {
                    out.extend(builder.resource(LinkType::Other, el, src, ordinal, position, None));
                }
            }
            // CSS, modulepreload, preload/prefetch
            "link" if html => {
                let Some(href) = v.attr("href") else {
                    return;
                };
                let (_, _, tokens) = rel_info(el);
                let has = |t: &str| tokens.iter().any(|x| x == t);
                let link_type = if has("stylesheet") {
                    Some(LinkType::Css)
                } else if has("modulepreload") {
                    Some(LinkType::JavaScript)
                } else if has("preload") || has("prefetch") {
                    Some(type_for_as(v.attr("as")))
                } else {
                    None
                };
                if let Some(link_type) = link_type {
                    out.extend(builder.resource(link_type, el, href, ordinal, position, None));
                }
            }
            _ => {}
        }
    });
    // Ab M2 (INTEG-080): die konfigurierten Regeln bestimmen die Position; die Standardregeln liefern die Position der
    // Vorfahrensuche von M1 unverändert (M2-D16 füllt `link_position::classify`).
    for link in &mut out {
        link.position =
            crate::link_position::classify(&link.xpath, link.position.clone(), &cfg.link_positions);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{CrawlConfig, PathType};

    fn run(html: &str) -> Vec<ExtractedLink> {
        let doc = Document::parse(html);
        let base = url::Url::parse("https://site.test/dir/page.html").unwrap();
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        extract(&doc, &base, &cfg)
    }

    #[test]
    fn hyperlink_basics() {
        let links = run("<body><nav><a href='/x' rel='NoFollow  Noopener' target=_blank>Hallo  <b>Welt</b></a></nav>");
        assert_eq!(links.len(), 1);
        let l = &links[0];
        assert_eq!(l.link_type, LinkType::Hyperlink);
        assert_eq!(l.resolved.as_deref(), Some("https://site.test/x"));
        assert_eq!(l.anchor.as_deref(), Some("Hallo Welt"));
        assert_eq!(l.rel.as_deref(), Some("nofollow noopener"));
        assert!(l.nofollow_rel);
        assert_eq!(l.target.as_deref(), Some("_blank"));
        assert_eq!(l.path_type, PathType::RootRelative);
        assert_eq!(l.position, LinkPosition::Navigation);
        assert_eq!(l.xpath, "/html[1]/body[1]/nav[1]/a[1]");
        assert!(!l.in_head);
    }

    #[test]
    fn template_and_script_are_ignored() {
        let links = run(
            "<body><template><a href=/t>t</a></template><script>var a='<a href=\"/s\">';</script><!-- <a href=/c> --><a href=/ok>ok</a>",
        );
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].href_raw, "/ok");
    }

    #[test]
    fn noscript_is_parsed() {
        let links = run("<body><noscript><a href=/n>n</a><img src=n.png></noscript>");
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn landmark_stack_unwinds() {
        let links = run(
            "<body><header><a href=/h>h</a></header><a href=/c>c</a><footer><nav><a href=/f>f</a></nav><a href=/g>g</a></footer>",
        );
        let pos: Vec<LinkPosition> = links.iter().map(|l| l.position.clone()).collect();
        assert_eq!(
            pos,
            [
                LinkPosition::Header,
                LinkPosition::Content,
                LinkPosition::Navigation,
                LinkPosition::Footer
            ]
        );
    }

    #[test]
    fn doc_order_is_non_decreasing() {
        let links = run("<body><a href=/1>1</a><p><a href=/2>2</a></p><a href=/3>3</a>");
        let orders: Vec<u32> = links.iter().map(|l| l.doc_order).collect();
        assert!(orders.windows(2).all(|w| w[0] < w[1]), "{orders:?}");
    }
}
