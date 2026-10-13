//! Tab „Hreflang“.
//!
//! STUB(M1-P25): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Hreflang“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Hreflang",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Contains Hreflang", FilterKind::Live, &[]),
            FilterDef::placeholder("Non-200 Hreflang URLs", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing Return Links", FilterKind::Live, &[]),
            FilterDef::placeholder("Incorrect Language & Region Codes", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing", FilterKind::Live, &[]),
        ],
    )
}
