//! Tab „Custom Search“: benutzerdefinierte Suchfilter (INTEG-031 ff.); je Filter eine dynamische Spalte.
//!
//! STUB(M2-F02): leerer Platzhalter (M2-D14 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::{ColumnSource, TabDef};

// STUB(M2-D14)
/// Definition des Tabs „Custom Search“.
pub fn tab() -> TabDef {
    super::stub_tab("Custom Search")
}

/// Quelle der dynamischen Spalten dieses Tabs (K-4); `None`, solange M2-D14 sie nicht liefert.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    None
}
