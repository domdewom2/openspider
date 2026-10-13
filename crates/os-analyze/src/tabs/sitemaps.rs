//! Tab „Sitemaps“.
//!
//! STUB(M1-P25): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use os_model::AnalysisStep;

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Sitemaps“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Sitemaps",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "URLs In Sitemap",
                FilterKind::PostCrawl(AnalysisStep::Sitemaps),
                &[],
            ),
            FilterDef::placeholder(
                "URLs Not In Sitemap",
                FilterKind::PostCrawl(AnalysisStep::Sitemaps),
                &[],
            ),
            FilterDef::placeholder(
                "Orphan URLs",
                FilterKind::PostCrawl(AnalysisStep::Sitemaps),
                &[],
            ),
            FilterDef::placeholder(
                "Non-Indexable URLs In Sitemap",
                FilterKind::PostCrawl(AnalysisStep::Sitemaps),
                &[],
            ),
        ],
    )
}
