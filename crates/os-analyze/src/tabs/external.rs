//! Tab „External“.
//!
//! STUB(M1-P20): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „External“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "External",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("HTML", FilterKind::Live, &[]),
            FilterDef::placeholder("JavaScript", FilterKind::Live, &[]),
            FilterDef::placeholder("CSS", FilterKind::Live, &[]),
            FilterDef::placeholder("Images", FilterKind::Live, &[]),
            FilterDef::placeholder("PDF", FilterKind::Live, &[]),
            FilterDef::placeholder("Flash", FilterKind::Live, &[]),
            FilterDef::placeholder("Other", FilterKind::Live, &[]),
            FilterDef::placeholder("Unknown", FilterKind::Live, &[]),
        ],
    )
}
