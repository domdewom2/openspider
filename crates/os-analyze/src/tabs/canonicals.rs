//! Tab „Canonicals“.
//!
//! STUB(M1-P24): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use os_model::AnalysisStep;

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Canonicals“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Canonicals",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Contains Canonical", FilterKind::Live, &[]),
            FilterDef::placeholder("Self Referencing", FilterKind::Live, &[]),
            FilterDef::placeholder("Canonicalised", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing", FilterKind::Live, &[]),
            FilterDef::placeholder("Multiple", FilterKind::Live, &[]),
            FilterDef::placeholder("Multiple Conflicting", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "Non-Indexable Canonical",
                FilterKind::PostCrawl(AnalysisStep::Canonicals),
                &[],
            ),
        ],
    )
}
