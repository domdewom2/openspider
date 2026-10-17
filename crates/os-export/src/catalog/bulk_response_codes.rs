//! Response Codes: Exporte der M2-Filter (OUT-050, OUT-084); die Inlinks-Exporte der drei Bereiche stehen seit M1 in `bulk_links`, D12 ergänzt hier weitere.
//!
//! STUB(M2-D12): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::TableEntry;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D12)
const ENTRIES: &[TableEntry] = &[
    // noch keine Einträge
];
