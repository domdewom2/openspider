//! Tab „Links“.
//!
//! STUB(M1-P25): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Links“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Links",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Pages With High Crawl Depth", FilterKind::Live, &[]),
            FilterDef::placeholder("Pages Without Internal Outlinks", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Nofollow Outlinks", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "Internal Outlinks With No Anchor Text",
                FilterKind::Live,
                &[],
            ),
        ],
    )
}
