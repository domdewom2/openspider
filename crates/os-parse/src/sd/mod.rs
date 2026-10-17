//! Strukturierte Daten (TECH-180 ff.): JSON-LD, Microdata und RDFa nach `ParsedPage.structured_data`.
//!
//! M2-F02 legt Modul und Aufruf an (`parse_html` ruft `run` nur bei mindestens einem eingeschalteten Format auf);
//! Unterstufen: `jsonld`, `microdata`, `rdfa`, `model` (M2-D09). Die Validierung (`validate/`) gehört M2-D20.

pub mod jsonld;
pub mod microdata;
pub mod model;
pub mod rdfa;

use os_model::ParsedPage;

use crate::PageCtx;

/// Extrahiert die eingeschalteten Formate.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}

// STUB(M2-D09)
pub fn to_writes(
    url: os_model::UrlId,
    items: &[os_model::SdItem],
) -> (Vec<os_model::SdItemWrite>, Vec<os_model::SdIssueWrite>) {
    let writes = items
        .iter()
        .map(|item| os_model::SdItemWrite {
            url_id: url,
            format: item.format.clone(),
            type_path: item.type_path.clone(),
            json: item.json.clone(),
        })
        .collect();
    let issues = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            item.parse_error
                .as_ref()
                .map(|error| os_model::SdIssueWrite {
                    url_id: url,
                    item: Some(i as u32),
                    validation: "parse".into(),
                    feature: None,
                    severity: "error".into(),
                    property: None,
                    code: error.clone(),
                })
        })
        .collect();
    (writes, issues)
}
