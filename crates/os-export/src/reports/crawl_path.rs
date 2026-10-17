//! Crawl-Path-Report (K-11).
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
    name: "Crawl Path Report",
    aliases: &[],
    file_stem: "Crawl Path Report",
    header: &["Address", "Depth", "Path"],
    header_fn: None,
    requires: &[],
    owner: "M2-D17",
    run: stub_run,
}];

// STUB(M2-D17)
pub fn export_detail(
    _: &crate::catalog::TableJob<'_>,
    _: &os_analyze::details::DetailReq<'_>,
    _: &std::path::Path,
    _: crate::types::Format,
) -> Result<crate::types::ExportFileInfo, crate::error::ExportError> {
    Err(crate::error::ExportError::NotImplemented("M2-D17"))
}
