//! Tab „Meta Description“.
//!
//! STUB(M1-P22): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Meta Description“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Meta Description",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing", FilterKind::Live, &[]),
            FilterDef::placeholder("Duplicate", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "Over {meta_description.max_chars} Characters",
                FilterKind::Live,
                &[],
            ),
            FilterDef::placeholder(
                "Below {meta_description.min_chars} Characters",
                FilterKind::Live,
                &[],
            ),
            FilterDef::placeholder("Multiple", FilterKind::Live, &[]),
        ],
    )
}
