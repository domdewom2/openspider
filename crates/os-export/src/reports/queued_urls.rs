//! Queued URLs (CRAWL-130).
//!
//! STUB(M2-D17): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D17)
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Queued URLs",
    aliases: &[],
    file_stem: "Queued URLs",
    header: &["Address", "Depth", "Priority"],
    header_fn: None,
    requires: &[],
    owner: "M2-D17",
    run: stub_run,
}];
