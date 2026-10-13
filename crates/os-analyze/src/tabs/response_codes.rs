//! Tab „Response Codes“.
//!
//! STUB(M1-P21): Platzhalter mit den kanonischen Filter-Kennungen aus `docs/plan/M1-filters.toml`;
//! das Paket ersetzt Spalten, Prädikate und Parameter. Der Test `tests/filters_registry.rs` bleibt aktiv.

use crate::defs::{placeholder_tab, FilterDef, FilterKind, TabDef};

/// Definition des Tabs „Response Codes“.
pub fn tab() -> TabDef {
    placeholder_tab(
        "Response Codes",
        vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            FilterDef::placeholder("Blocked by Robots.txt", FilterKind::Live, &[]),
            FilterDef::placeholder("No Response", FilterKind::Live, &[]),
            FilterDef::placeholder("Success (2XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Redirection (3XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Redirection (Meta Refresh)", FilterKind::Live, &[]),
            FilterDef::placeholder("Redirect Chain", FilterKind::Live, &[]),
            FilterDef::placeholder("Redirect Loop", FilterKind::Live, &[]),
            FilterDef::placeholder("Client Error (4XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Server Error (5XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Other", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal All", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Blocked by Robots.txt", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal No Response", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Success (2XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Redirection (3XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Redirection (Meta Refresh)", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Redirect Chain", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Redirect Loop", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Client Error (4XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Server Error (5XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("Internal Other", FilterKind::Live, &[]),
            FilterDef::placeholder("External All", FilterKind::Live, &[]),
            FilterDef::placeholder("External Blocked by Robots.txt", FilterKind::Live, &[]),
            FilterDef::placeholder("External No Response", FilterKind::Live, &[]),
            FilterDef::placeholder("External Success (2XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("External Redirection (3XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("External Redirection (Meta Refresh)", FilterKind::Live, &[]),
            FilterDef::placeholder("External Redirect Chain", FilterKind::Live, &[]),
            FilterDef::placeholder("External Redirect Loop", FilterKind::Live, &[]),
            FilterDef::placeholder("External Client Error (4XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("External Server Error (5XX)", FilterKind::Live, &[]),
            FilterDef::placeholder("External Other", FilterKind::Live, &[]),
        ],
    )
}
