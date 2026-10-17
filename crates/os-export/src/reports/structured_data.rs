//! Structured Data: Validierungs- und Rich-Results-Reports (OUT-100 bis OUT-103).
//!
//! STUB(M2-D20): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D20)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Structured Data:Validation Errors & Warnings Summary",
        aliases: &[],
        file_stem: "Structured Data:Validation Errors & Warnings Summary",
        header: &["Validation Type", "Severity", "Type", "Feature", "Issue", "URLs", "% of Total", "Sample URL"],
        requires: &[],
        owner: "M2-D20",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Validation Errors & Warnings",
        aliases: &[],
        file_stem: "Structured Data:Validation Errors & Warnings",
        header: &["Address", "Validation Type", "Severity", "Type", "Feature", "Issue", "Property Path"],
        requires: &[],
        owner: "M2-D20",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Google Rich Results Features Summary",
        aliases: &[],
        file_stem: "Structured Data:Google Rich Results Features Summary",
        header: &["Feature", "URLs", "% of Total", "Valid", "With Warnings", "With Errors", "Sample URL"],
        requires: &[],
        owner: "M2-D20",
        run: stub_run,
    },
    TableEntry {
        name: "Structured Data:Google Rich Results Features",
        aliases: &[],
        file_stem: "Structured Data:Google Rich Results Features",
        header: &["Address"],
        requires: &[],
        owner: "M2-D20",
        run: stub_run,
    },
];
