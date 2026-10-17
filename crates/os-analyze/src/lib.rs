//! `os-analyze` – Filterlogik je Tab, Issues und Nachlauf-Analyse (Link Score, Duplikate per
//! SimHash/MinHash, Orphans, Ketten, hreflang-Rücklinks, Sitemaps).
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Keine Abhängigkeit auf `os-engine` außer als Dev-Abhängigkeit
//! mit Feature `testing`. Laufzeit: P14 (`views`, `cursor`, `names`, `state`, `analysis`-Runner),
//! Issues: P12, Auslösung der Crawl-Analyse: P19, Tabs: P20–P25.
//!
//! Ab M2 (M2-F02): Register aller Tabs und Filter aus `M1-filters.toml` und `M2-filters.toml` (`tabs`), Spalten-Metadaten
//! und dynamische Spaltenquellen (`defs`), Signaturen für View-Sitzungen (`views::session`), Suche (`search`), Aggregate,
//! Graphen, Segmente, Vergleich und MinHash als Stubs der Fachpakete.

pub mod aggregates;
pub mod analysis;
pub mod compare;
pub mod cursor;
pub mod defs;
pub mod details;
pub mod error;
pub mod graph;
pub mod issues;
pub mod minhash;
pub mod names;
pub mod search;
pub mod segments;
pub mod state;
pub mod tabs;
pub mod views;

pub use defs::{
    Align, ColumnDef, ColumnKind, ColumnSource, ColumnSql, FilterDef, FilterKind, Needs,
    Requirement, SearchOps, SortKey, TabDef,
};
pub use error::{AnalyzeError, NameError, ViewError};
