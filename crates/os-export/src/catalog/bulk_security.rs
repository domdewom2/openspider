//! Security: Exporte je Filter (OUT-051).
//!
//! STUB(M2-D11): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D11)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Security:HTTP URLs Inlinks",
        aliases: &[],
        file_stem: "Security:HTTP URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
    TableEntry {
        name: "Security:Mixed Content Inlinks",
        aliases: &["Security:Mixed Content"],
        file_stem: "Security:Mixed Content Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
    TableEntry {
        name: "Security:Form URL Insecure Inlinks",
        aliases: &["Security:Form URL Insecure"],
        file_stem: "Security:Form URL Insecure Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
    TableEntry {
        name: "Security:Form On HTTP URL Inlinks",
        aliases: &["Security:Form On HTTP URL"],
        file_stem: "Security:Form On HTTP URL Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
    TableEntry {
        name: "Security:Unsafe Cross-Origin Links Inlinks",
        aliases: &["Security:Unsafe Cross-Origin Links"],
        file_stem: "Security:Unsafe Cross-Origin Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
    TableEntry {
        name: "Security:Protocol-Relative Resource Links Inlinks",
        aliases: &["Security:Protocol-Relative Resource Links"],
        file_stem: "Security:Protocol-Relative Resource Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D11",
        run: stub_run,
    },
];
