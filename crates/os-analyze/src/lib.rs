//! `os-analyze` – Filterlogik je Tab, Issues und Nachlauf-Analyse (Link Score, Duplikate per
//! SimHash/MinHash, Orphans, Ketten, hreflang-Rücklinks, Sitemaps).
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Keine Abhängigkeit auf `os-engine` außer als Dev-Abhängigkeit
//! mit Feature `testing`. Laufzeit: P14 (`views`, `cursor`, `names`, `state`, `analysis`-Runner),
//! Issues: P12, Auslösung der Crawl-Analyse: P19, Tabs: P20–P25.

pub mod analysis;
pub mod cursor;
pub mod defs;
pub mod error;
pub mod issues;
pub mod names;
pub mod state;
pub mod tabs;
pub mod views;

pub use defs::{
    ColumnDef, ColumnKind, ColumnSql, FilterDef, FilterKind, Requirement, SortKey, TabDef,
};
pub use error::{AnalyzeError, NameError, ViewError};
