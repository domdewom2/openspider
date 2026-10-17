//! SERP Summary (OUT-098).
//!
//! STUB(M2-D01): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D01)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "SERP Summary",
        aliases: &[],
        file_stem: "SERP Summary",
        header: &["Address", "Title 1", "Title 1 Length", "Title 1 Pixel Width", "Meta Description 1", "Meta Description 1 Length", "Meta Description 1 Pixel Width"],
        requires: &[],
        owner: "M2-D01",
        run: stub_run,
    },
];
