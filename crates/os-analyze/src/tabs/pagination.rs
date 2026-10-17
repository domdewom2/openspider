//! Tab „Pagination“: rel=next/prev (TECH-031 ff.).
//!
//! STUB(M2-F02): leerer Platzhalter (M2-D05 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::TabDef;

// STUB(M2-D05)
/// Definition des Tabs „Pagination“.
pub fn tab() -> TabDef {
    super::stub_tab("Pagination")
}
