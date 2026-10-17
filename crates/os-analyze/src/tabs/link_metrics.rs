//! Tab „Link Metrics“: Linkmetriken von Majestic, Ahrefs und Moz (INTEG-151 ff.); Spalten je Anbieter, Metrik und Ebene dynamisch.
//!
//! STUB(M2-F02): leerer Platzhalter (M2-I05 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::{ColumnSource, TabDef};

// STUB(M2-I05)
/// Definition des Tabs „Link Metrics“.
pub fn tab() -> TabDef {
    super::stub_tab("Link Metrics")
}

/// Quelle der dynamischen Spalten dieses Tabs (K-4); `None`, solange M2-I05 sie nicht liefert.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    None
}
