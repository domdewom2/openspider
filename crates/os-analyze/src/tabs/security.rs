//! Tab „Security“.
//!
//! STUB(M1-P20): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Security“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Security",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("HTTP URLs", FilterKind::Live, &[]),
            FilterDef::placeholder("HTTPS URLs", FilterKind::Live, &[]),
        ],
    )
}
