//! HTTP Header Summary (OUT-110).
//!
//! STUB(M2-D18): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D18)
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "HTTP Header Summary",
    aliases: &[],
    file_stem: "HTTP Header Summary",
    header: &[
        "Header Name",
        "URLs",
        "% of Total",
        "Distinct Values",
        "Most Common Value",
    ],
    requires: &[ReportRequirement::Config {
        key: "extraction.http_headers",
        what: "HTTP header extraction",
    }],
    owner: "M2-D18",
    run: stub_run,
    header_fn: None,
}];
