//! Hinweise aus HTTP-Headern (`Link`, `X-Robots-Tag`, `Refresh`) und ihre Zusammenführung mit der Seite.
//!
//! Gilt für jede Antwort, auch für Nicht-HTML (PDF, Bilder): `parse_headers` ruft [`hints`] auf. Headernamen
//! werden ohne Beachtung der Schreibweise verglichen, mehrfach vorkommende Header werden alle ausgewertet, in
//! der Reihenfolge der übergebenen Liste.
//!
//! * `Link` (RFC 8288): kommagetrennte Einträge `<URL>; rel="…"; hreflang="…"`, Parameterwerte mit oder ohne
//!   Anführungszeichen. `rel=canonical` → Canonical, `rel=next|prev|previous` → Pagination,
//!   `rel=alternate` mit nicht-leerem `hreflang` → hreflang. Relative URLs werden gegen die Antwort-URL aufgelöst.
//! * `X-Robots-Tag`: wie Meta-Robots (siehe `directives`), zusätzlich mit Agentenpräfix `agent:`.
//! * `Refresh`: gleiche Syntax wie Meta-Refresh (siehe `refresh`).
//!
//! Angaben aus Headern tragen `in_head = true` (sie liegen nicht „außerhalb des Heads“) und zählen ihre
//! `order`-Werte ab 0; [`merge`] setzt sie hinter die Angaben der Seite fort.

use os_model::{
    CanonicalRef, Directive, DirectiveSource, HeaderHints, HintSource, PaginationRef, ParsedPage,
    RefreshSource,
};

use crate::canonical::{has_rel_token, raw_and_resolved};
use crate::directives::{agent_applies, parse_value};
use crate::hreflang::{self, make_ref, MAX_ANNOTATIONS};
use crate::refresh;
use crate::ParseConfig;

/// Ein Eintrag eines `Link`-Headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkEntry {
    /// URL zwischen `<` und `>`.
    pub target: String,
    /// Parameter (Name klein, Wert ohne Anführungszeichen), in Reihenfolge.
    pub params: Vec<(String, String)>,
}

impl LinkEntry {
    /// Wert des ersten Parameters mit dem Namen (RFC 8288: spätere Wiederholungen zählen nicht).
    pub(crate) fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Zerlegt einen `Link`-Header (RFC 8288) in seine Einträge. Kaputte Einträge werden bis zum nächsten Komma
/// übersprungen.
pub(crate) fn parse_link_header(value: &str) -> Vec<LinkEntry> {
    let bytes = value.as_bytes();
    let mut entries = Vec::new();
    let mut i = 0usize;
    let skip_ws = |i: &mut usize| {
        while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    while i < bytes.len() {
        skip_ws(&mut i);
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b',' {
            i += 1;
            continue;
        }
        if bytes[i] != b'<' {
            // Fehlererholung: bis zum nächsten Komma außerhalb von Anführungszeichen
            let mut quoted = false;
            while i < bytes.len() && (quoted || bytes[i] != b',') {
                if bytes[i] == b'"' {
                    quoted = !quoted;
                }
                i += 1;
            }
            continue;
        }
        let Some(close) = value[i + 1..].find('>') else {
            break;
        };
        let target = value[i + 1..i + 1 + close].trim().to_string();
        i += close + 2;
        let mut params = Vec::new();
        loop {
            skip_ws(&mut i);
            if i >= bytes.len() || bytes[i] == b',' {
                break;
            }
            if bytes[i] != b';' {
                i += 1;
                continue;
            }
            i += 1;
            skip_ws(&mut i);
            let name_start = i;
            while i < bytes.len() && !matches!(bytes[i], b'=' | b';' | b',') {
                i += 1;
            }
            let name = value[name_start..i].trim().to_ascii_lowercase();
            let mut val = String::new();
            if i < bytes.len() && bytes[i] == b'=' {
                i += 1;
                skip_ws(&mut i);
                if i < bytes.len() && bytes[i] == b'"' {
                    i += 1;
                    let mut raw = Vec::new();
                    while i < bytes.len() && bytes[i] != b'"' {
                        if bytes[i] == b'\\' && i + 1 < bytes.len() {
                            i += 1;
                        }
                        raw.push(bytes[i]);
                        i += 1;
                    }
                    i += 1; // schließendes Anführungszeichen
                    val = String::from_utf8_lossy(&raw).into_owned();
                } else {
                    let start = i;
                    while i < bytes.len()
                        && !matches!(bytes[i], b';' | b',')
                        && !bytes[i].is_ascii_whitespace()
                    {
                        i += 1;
                    }
                    val = value[start..i].to_string();
                }
            }
            if !name.is_empty() {
                params.push((name, val));
            }
        }
        entries.push(LinkEntry { target, params });
    }
    entries
}

/// Wertet die Header einer Antwort aus (auch Nicht-HTML).
pub fn hints(url: &url::Url, headers: &[(String, String)], cfg: &ParseConfig) -> HeaderHints {
    let tokens = vec![cfg.robots_token.clone(), "googlebot".to_string()];
    let mut out = HeaderHints::default();
    let mut hreflang_refs = Vec::new();

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("link") {
            for entry in parse_link_header(value) {
                let rel = entry.param("rel").unwrap_or_default();
                let (raw, resolved) = raw_and_resolved(url, &entry.target);
                if has_rel_token(rel, "canonical") {
                    out.canonicals.push(CanonicalRef {
                        source: HintSource::Http,
                        raw: raw.clone(),
                        resolved: resolved.clone(),
                        in_head: true,
                        order: out.canonicals.len() as u32,
                    });
                }
                let next = has_rel_token(rel, "next");
                let prev = has_rel_token(rel, "prev") || has_rel_token(rel, "previous");
                for rel_next in [true, false] {
                    if (rel_next && next) || (!rel_next && prev) {
                        out.pagination.push(PaginationRef {
                            rel_next,
                            source: HintSource::Http,
                            raw: raw.clone(),
                            resolved: resolved.clone(),
                            in_head: true,
                            order: out.pagination.len() as u32,
                        });
                    }
                }
                if has_rel_token(rel, "alternate") {
                    if let Some(code) = entry.param("hreflang").filter(|c| !c.trim().is_empty()) {
                        hreflang_refs.push(make_ref(
                            HintSource::Http,
                            code,
                            raw.clone(),
                            resolved.clone(),
                            true,
                            hreflang_refs.len() as u32,
                            cfg.allow_script_subtag,
                        ));
                    }
                }
            }
        } else if name.eq_ignore_ascii_case("x-robots-tag") {
            for group in parse_value(value, true) {
                out.directives.push(Directive {
                    source: DirectiveSource::XRobotsTag,
                    applies: agent_applies(&group.agent, &tokens),
                    agent: group.agent,
                    raw: group.raw,
                    tokens: group.tokens,
                    in_head: true,
                    order: out.directives.len() as u32,
                });
            }
        } else if name.eq_ignore_ascii_case("refresh") {
            if let Some(r) = refresh::make_ref(
                RefreshSource::Header,
                value,
                url,
                true,
                out.refresh.len() as u32,
            ) {
                out.refresh.push(r);
            }
        }
    }

    out.hreflang = hreflang::apply_limit(hreflang_refs, cfg.hreflang_over_limit).0;
    out
}

/// Fügt die Header-Hinweise den Angaben der Seite hinzu (Header nach HTML). Die `order`-Werte der Header-Angaben
/// werden hinter die der Seite fortgesetzt. Mehr als 500 hreflang-Annotationen insgesamt werden abgeschnitten und
/// in `hreflang_truncated` gezählt.
pub fn merge(page: &mut ParsedPage, hints: HeaderHints) {
    let shift = |len: usize| u32::try_from(len).unwrap_or(u32::MAX);

    let base = shift(page.canonicals.len());
    page.canonicals
        .extend(hints.canonicals.into_iter().map(|mut c| {
            c.order = c.order.saturating_add(base);
            c
        }));
    let base = shift(page.directives.len());
    page.directives
        .extend(hints.directives.into_iter().map(|mut d| {
            d.order = d.order.saturating_add(base);
            d
        }));
    let base = shift(page.refresh.len());
    page.refresh.extend(hints.refresh.into_iter().map(|mut r| {
        r.order = r.order.saturating_add(base);
        r
    }));
    let base = shift(page.hreflang.len());
    page.hreflang
        .extend(hints.hreflang.into_iter().map(|mut h| {
            h.order = h.order.saturating_add(base);
            h
        }));
    if page.hreflang.len() > MAX_ANNOTATIONS {
        let dropped = page.hreflang.len() - MAX_ANNOTATIONS;
        page.hreflang.truncate(MAX_ANNOTATIONS);
        page.hreflang_truncated = page
            .hreflang_truncated
            .saturating_add(u32::try_from(dropped).unwrap_or(u32::MAX));
    }
    let base = shift(page.pagination.len());
    page.pagination
        .extend(hints.pagination.into_iter().map(|mut p| {
            p.order = p.order.saturating_add(base);
            p
        }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_header_entries_and_params() {
        let e = parse_link_header(
            "<https://a.test/x>; rel=\"canonical\", </p2>; rel=next; title=\"a, b\" , <u>; REL = alternate; hreflang=de",
        );
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].target, "https://a.test/x");
        assert_eq!(e[0].param("rel"), Some("canonical"));
        assert_eq!(e[1].param("rel"), Some("next"));
        assert_eq!(e[1].param("title"), Some("a, b"));
        assert_eq!(e[2].param("rel"), Some("alternate"));
        assert_eq!(e[2].param("hreflang"), Some("de"));
    }

    #[test]
    fn link_header_garbage_is_skipped() {
        let e = parse_link_header("garbage, <a>; rel=next, <broken");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].target, "a");
        assert!(parse_link_header("").is_empty());
    }
}
