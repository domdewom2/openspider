//! Meta-Refresh und `Refresh`-Header (TECH-064, TECH-050).
//!
//! Syntax (HTML-Standard, großzügig ausgelegt): Verzögerung in Sekunden (ganze Zahl, Nachkommastellen werden
//! abgeschnitten), danach optional `;`, `,` oder Leerraum und ein Ziel, das mit `url=` (beliebige Schreibweise,
//! Leerraum um `=` erlaubt) eingeleitet sein kann und in `'…'` oder `"…"` stehen darf. Ohne Ziel (`content="30"`)
//! ist es ein Selbst-Refresh; ob das Ziel die eigene URL ist, entscheidet P24/P15, nicht dieses Modul. Inhalte
//! ohne führende Zahl (`url=/x`, `abc`) sind kein Refresh und werden nicht gespeichert.
//!
//! Alle Vorkommen werden in Quelltextreihenfolge gespeichert, auch außerhalb des Heads und in `<noscript>`;
//! `<template>`-Inhalt zählt nicht. Welches Vorkommen weiterleitet (das erste), entscheidet der Aufrufer.

use std::sync::OnceLock;

use os_model::{RefreshRef, RefreshSource};
use scraper::Selector;

use crate::base::{clean_url_text, resolve_string};
use crate::dom::{self, Document};

fn is_ws(c: char) -> bool {
    c.is_ascii_whitespace()
}

/// Zerlegt einen Refresh-Wert in `(Verzögerung, Ziel roh)`; `None`, wenn es kein Refresh ist.
pub(crate) fn parse_content(content: &str) -> Option<(u32, Option<String>)> {
    let s = content.trim_start_matches(is_ws);
    let digits_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if digits_end == 0 {
        return None;
    }
    let delay = s[..digits_end]
        .parse::<u64>()
        .map(|v| v.min(u64::from(u32::MAX)) as u32)
        .unwrap_or(u32::MAX);
    let mut rest = &s[digits_end..];
    if let Some(fraction) = rest.strip_prefix('.') {
        rest = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    if !rest.is_empty() && !rest.starts_with(|c: char| is_ws(c) || c == ';' || c == ',') {
        return None;
    }
    rest = rest.trim_start_matches(is_ws);
    if let Some(after_sep) = rest.strip_prefix([';', ',']) {
        rest = after_sep.trim_start_matches(is_ws);
    }
    if rest.is_empty() {
        return Some((delay, None));
    }

    let mut url = rest;
    if rest.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("url")) {
        let after = rest[3..].trim_start_matches(is_ws);
        if let Some(value) = after.strip_prefix('=') {
            url = value.trim_start_matches(is_ws);
        }
    }
    if let Some(quote) = url.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        let inner = &url[1..];
        url = match inner.find(quote) {
            Some(end) => &inner[..end],
            None => inner,
        };
    }
    let url = url.trim_matches(is_ws);
    Some((delay, (!url.is_empty()).then(|| url.to_string())))
}

/// Baut die Angabe aus dem Rohwert; `None`, wenn der Wert kein Refresh ist.
pub(crate) fn make_ref(
    source: RefreshSource,
    raw: &str,
    base: &url::Url,
    in_head: bool,
    order: u32,
) -> Option<RefreshRef> {
    let (delay, target_raw) = parse_content(raw)?;
    let target = target_raw
        .as_deref()
        .and_then(|t| resolve_string(base, &clean_url_text(t)));
    Some(RefreshRef {
        source,
        raw: raw.trim().to_string(),
        delay: Some(delay),
        target_raw,
        target,
        in_head,
        order,
        xpath: None,
    })
}

/// Extrahiert alle Meta-Refresh-Angaben (`<meta http-equiv="refresh" content>`).
pub fn extract(doc: &Document, base: &url::Url) -> Vec<RefreshRef> {
    static SEL: OnceLock<Selector> = OnceLock::new();
    let sel = SEL
        .get_or_init(|| Selector::parse("meta[http-equiv][content]").expect("gültiger Selektor"));
    let mut out = Vec::new();
    for el in doc.html().select(sel) {
        if !crate::links::is_html(el) || dom::in_template(el) {
            continue;
        }
        let equiv = el.value().attr("http-equiv").unwrap_or_default();
        if !equiv.trim().eq_ignore_ascii_case("refresh") {
            continue;
        }
        let content = el.value().attr("content").unwrap_or_default();
        if let Some(mut r) = make_ref(
            RefreshSource::Meta,
            content,
            base,
            dom::in_head(el),
            out.len() as u32,
        ) {
            r.xpath = Some(dom::xpath(el));
            out.push(r);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<(u32, Option<String>)> {
        parse_content(s)
    }

    fn t(delay: u32, target: &str) -> Option<(u32, Option<String>)> {
        Some((delay, Some(target.to_string())))
    }

    #[test]
    fn variants() {
        assert_eq!(p("0;url=/x"), t(0, "/x"));
        assert_eq!(p("5; url=/neu"), t(5, "/neu"));
        assert_eq!(p("0; URL='/x'"), t(0, "/x"));
        assert_eq!(p("3;URL=\"/x y\""), t(3, "/x y"));
        assert_eq!(p("0, url=/x"), t(0, "/x"));
        assert_eq!(p("0 url = /x"), t(0, "/x"));
        assert_eq!(p("0;/x"), t(0, "/x"));
        assert_eq!(p("  7  "), Some((7, None)));
        assert_eq!(p("0"), Some((0, None)));
        assert_eq!(p("0;"), Some((0, None)));
        assert_eq!(p("0;url="), Some((0, None)));
        assert_eq!(p("2.5;url=/x"), t(2, "/x"));
        assert_eq!(p("0;urlaub/x"), t(0, "urlaub/x"));
    }

    #[test]
    fn not_a_refresh() {
        assert_eq!(p(""), None);
        assert_eq!(p("url=/x"), None);
        assert_eq!(p("abc"), None);
        assert_eq!(p("5x;url=/x"), None);
    }
}
