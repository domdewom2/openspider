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

/// Fakten aus dem bereits vorhandenen Seitenbaum, vor den Speicherschaltern.
/// Der Vergleich ignoriert Fragmente und normalisiert URL-Schreibweisen (TECH-002).
pub fn run(ctx: &crate::PageCtx<'_>, page: &mut os_model::ParsedPage) {
    // Die meisten Seiten haben keine Paginierung: keine zweite Linkmenge aufbauen.
    let missing = !page.pagination.is_empty() && missing_anchor(ctx, &page.pagination);
    page.ext.extend([
        (
            os_model::ExtKey::from_static("pagination.has_prev"),
            os_model::Value::Bool(page.pagination.iter().any(|p| !p.rel_next)),
        ),
        (
            os_model::ExtKey::from_static("pagination.not_in_anchor"),
            os_model::Value::Bool(missing),
        ),
    ]);
}

fn missing_anchor(ctx: &crate::PageCtx<'_>, refs: &[PaginationRef]) -> bool {
    static SELECTOR: std::sync::OnceLock<scraper::Selector> = std::sync::OnceLock::new();
    let selector =
        SELECTOR.get_or_init(|| scraper::Selector::parse("a[href]").expect("gültiger Selektor"));
    let anchors: std::collections::HashSet<String> = ctx
        .doc
        .html()
        .select(selector)
        .filter(|el| !dom::in_template(*el))
        .filter_map(|el| raw_and_resolved(ctx.base, el.value().attr("href")?).1)
        .filter_map(|url| os_url::compare_key(&url))
        .collect();
    refs.iter().any(|p| {
        p.resolved
            .as_ref()
            .is_none_or(|url| os_url::compare_key(url).is_none_or(|key| !anchors.contains(&key)))
    })
}

/// Header-Fakt auch für Antworten ohne HTML-Parse und bei ausgeschaltetem Speichern.
/// `pagination.not_in_anchor` schreibt allein `run()`, das auch Header-URLs mit dem Baum vergleicht.
pub fn response_facts(
    input: &crate::ResponseInput<'_>,
) -> Vec<(os_model::ExtKey, os_model::Value)> {
    let mut has_prev = false;
    for entry in input
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("link"))
        .flat_map(|(_, value)| crate::headers::parse_link_header(value))
    {
        if let Some(rel) = entry.param("rel") {
            has_prev |= has_rel_token(rel, "prev") || has_rel_token(rel, "previous");
        }
    }
    vec![(
        os_model::ExtKey::from_static("pagination.has_prev"),
        os_model::Value::Bool(has_prev),
    )]
}
