//! Web: Screenshots (OUT-043).
//!
//! STUB(M2-R02): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-R02)
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Web:Screenshots",
    aliases: &[],
    file_stem: "Web:Screenshots",
    header: &["Address", "File"],
    requires: &[ReportRequirement::Rendering],
    owner: "M2-R02",
    run: stub_run,
}];
