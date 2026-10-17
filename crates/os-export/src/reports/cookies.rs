//! Cookie Summary (OUT-111).
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
    name: "Cookie Summary",
    aliases: &[],
    file_stem: "Cookie Summary",
    header: &[
        "Cookie Name",
        "Domain",
        "Path",
        "Persistence",
        "Secure",
        "HttpOnly",
        "SameSite",
        "Type",
        "First-Party",
        "URLs",
        "Sample URL",
    ],
    requires: &[ReportRequirement::Config {
        key: "extraction.cookies",
        what: "cookie extraction",
    }],
    owner: "M2-D18",
    run: stub_run,
    header_fn: None,
}];
