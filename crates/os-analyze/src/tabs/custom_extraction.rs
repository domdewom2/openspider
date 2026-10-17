//! Tab „Custom Extraction“: benutzerdefinierte Extraktoren (INTEG-053 ff.); je Extraktor dynamische Spalten.
//!
//! STUB(M2-F02): leerer Platzhalter (M2-D15 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::{ColumnSource, TabDef};

// STUB(M2-D15)
/// Definition des Tabs „Custom Extraction“.
pub fn tab() -> TabDef {
    super::stub_tab("Custom Extraction")
}

/// Quelle der dynamischen Spalten dieses Tabs (K-4); `None`, solange M2-D15 sie nicht liefert.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    None
}
