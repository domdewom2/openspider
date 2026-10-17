//! Tab „Custom JavaScript“: benutzerdefinierte JavaScript-Snippets (INTEG-071 ff.); je Snippet dynamische Spalten.
//!
//! STUB(M2-F02): leerer Platzhalter (M2-D22 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::{ColumnSource, TabDef};

// STUB(M2-D22)
/// Definition des Tabs „Custom JavaScript“.
pub fn tab() -> TabDef {
    super::stub_tab("Custom JavaScript")
}

/// Quelle der dynamischen Spalten dieses Tabs (K-4); `None`, solange M2-D22 sie nicht liefert.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    None
}
