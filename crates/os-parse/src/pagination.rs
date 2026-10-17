//! Paginierung (`<link rel=next|prev>`, TECH-030).
//!
//! Nur `<link>`-Elemente zählen; `<a rel=next>` ist ein normaler Hyperlink. `rel=previous` wird wie `prev`
//! behandelt (Festlegung). Steht beides im `rel`-Wert, entstehen zwei Angaben (erst `next`, dann `prev`).
//! Die Angaben aus dem `Link`-Header liefert `headers`.

use os_model::{HintSource, PaginationRef};

use crate::canonical::{has_rel_token, link_elements, raw_and_resolved};
use crate::dom::{self, Document};

/// Extrahiert die Paginierungs-Angaben.
pub fn extract(doc: &Document, base: &url::Url) -> Vec<PaginationRef> {
    let mut out = Vec::new();
    for el in link_elements(doc) {
        let Some(rel) = el.value().attr("rel") else {
            continue;
        };
        let Some(href) = el.value().attr("href") else {
            continue;
        };
        let next = has_rel_token(rel, "next");
        let prev = has_rel_token(rel, "prev") || has_rel_token(rel, "previous");
        if !next && !prev {
            continue;
        }
        let (raw, resolved) = raw_and_resolved(base, href);
        let in_head = dom::in_head(el);
        for rel_next in [true, false] {
            if (rel_next && next) || (!rel_next && prev) {
                out.push(PaginationRef {
                    rel_next,
                    source: HintSource::Html,
                    raw: raw.clone(),
                    resolved: resolved.clone(),
                    in_head,
                    order: out.len() as u32,
                    xpath: Some(dom::xpath(el)),
                });
            }
        }
    }
    out
}

// STUB(M2-D05)
pub fn response_facts(
    _input: &crate::ResponseInput<'_>,
) -> Vec<(os_model::ExtKey, os_model::Value)> {
    Vec::new()
}
