//! Tab „URL“.
//!
//! STUB(M1-P21): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „URL“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "URL",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Non ASCII Characters", FilterKind::Live, &[]),
            FilterDef::placeholder("Underscores", FilterKind::Live, &[]),
            FilterDef::placeholder("Uppercase", FilterKind::Live, &[]),
            FilterDef::placeholder("Multiple Slashes", FilterKind::Live, &[]),
            FilterDef::placeholder("Contains A Space", FilterKind::Live, &[]),
            FilterDef::placeholder("Parameters", FilterKind::Live, &[]),
            FilterDef::placeholder("Over {url.max_chars} Characters", FilterKind::Live, &[]),
        ],
    )
}
