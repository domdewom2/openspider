//! Canonicals: Canonical Chains (OUT-086).
//!
//! STUB(M2-D04): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D04)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Canonicals:Canonical Chains",
        aliases: &[],
        file_stem: "Canonicals:Canonical Chains",
        header: crate::catalog::CHAIN_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
];
