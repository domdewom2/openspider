//! Tab „Analytics“: Google-Analytics-4-Daten (INTEG-097 ff.).
//!
//! STUB(M2-F02): leerer Platzhalter (M2-I02 liefert Spalten, Population und Filterprädikate). Die Filterkennungen stehen in
//! `tabs::M2_FILTERS` (`docs/plan/M2-filters.toml`); ein hier selbst definierter Filter hat Vorrang vor dem Stub.

use crate::defs::TabDef;

// STUB(M2-I02)
/// Definition des Tabs „Analytics“.
pub fn tab() -> TabDef {
    super::stub_tab("Analytics")
}
