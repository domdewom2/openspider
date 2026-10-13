//! Tab „H1“.
//!
//! STUB(M1-P22): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „H1“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "H1",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing", FilterKind::Live, &[]),
            FilterDef::placeholder("Duplicate", FilterKind::Live, &[]),
            FilterDef::placeholder("Over {h1.max_chars} Characters", FilterKind::Live, &[]),
            FilterDef::placeholder("Multiple", FilterKind::Live, &[]),
        ],
    )
}
