//! Custom Extraction: Exporte (OUT-061).
//!
//! STUB(M2-D15): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D15)
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Custom Extraction:All Extractions",
    aliases: &[],
    file_stem: "Custom Extraction:All Extractions",
    header: &["Address", "Extractor", "Index", "Value"],
    requires: &[],
    owner: "M2-D15",
    run: stub_run,
}];
