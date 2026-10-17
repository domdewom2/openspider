//! Dynamische API-Spalten im Tab „Internal“ (GA4, Search Console, PageSpeed, Linkmetriken; INTEG-001 ff., K-4).
//!
//! Dies ist kein Tab, sondern die Quelle zusätzlicher Spalten des Tabs „Internal“ (`tabs::column_sources()`); M2-F02
//! legt nur den Einstieg an.

use crate::defs::ColumnSource;

// STUB(M2-I06)
/// Quelle der API-Spalten für „Internal“; `None`, solange M2-I06 sie nicht liefert.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    None
}
