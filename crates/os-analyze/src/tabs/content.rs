//! Tab „Content“.
//!
//! STUB(M1-P23): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Content“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Content",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "Exact Duplicates",
                FilterKind::Live,
                &["extraction.hash", "duplicates.exact_enabled"],
            ),
            FilterDef::placeholder("Low Content Pages", FilterKind::Live, &[]),
        ],
    )
}
