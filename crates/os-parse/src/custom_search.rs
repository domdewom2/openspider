//! Custom Search (INTEG-020 ff.): wertet `custom_search.filters` aus und füllt `ParsedPage.custom_search`.
//!
//! M2-F02 legt nur den Aufruf an (`parse_html` ruft `run` nur bei mindestens einem Filter auf); die Auswertung
//! gehört M2-D14.

// STUB(M2-D14)
use os_model::ParsedPage;

use crate::PageCtx;

/// Zählt die Treffer jedes Filters und setzt `page.custom_search`.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}

// STUB(M2-D14)
pub fn run_on_body(_input: &crate::ParseInput<'_>) -> Vec<os_model::SearchHit> {
    Vec::new()
}
// STUB(M2-D14)
pub fn validate(_cfg: &os_model::CustomSearchConfig) -> Vec<os_model::ConfigIssue> {
    Vec::new()
}
