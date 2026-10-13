//! Tab „Images“.
//!
//! STUB(M1-P23): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Images“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Images",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Over {image.max_kb} KB", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing Alt Text", FilterKind::Live, &[]),
            FilterDef::placeholder("Missing Alt Attribute", FilterKind::Live, &[]),
            FilterDef::placeholder(
                "Alt Text Over {image.alt_max_chars} Characters",
                FilterKind::Live,
                &[],
            ),
        ],
    )
}
