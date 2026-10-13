//! Tab „Directives“.
//!
//! STUB(M1-P24): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Directives“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Directives",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Index", FilterKind::Live, &[]),
            FilterDef::placeholder("Noindex", FilterKind::Live, &[]),
            FilterDef::placeholder("Follow", FilterKind::Live, &[]),
            FilterDef::placeholder("Nofollow", FilterKind::Live, &[]),
            FilterDef::placeholder("None", FilterKind::Live, &[]),
            FilterDef::placeholder("Refresh", FilterKind::Live, &[]),
        ],
    )
}
