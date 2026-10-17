//! Pagination: Exporte je Filter (OUT-066).
//!
//! STUB(M2-D05): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D05)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Pagination:Contains Pagination Inlinks",
        aliases: &[],
        file_stem: "Pagination:Contains Pagination Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:First Page Inlinks",
        aliases: &[],
        file_stem: "Pagination:First Page Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Paginated 2+ Pages Inlinks",
        aliases: &[],
        file_stem: "Pagination:Paginated 2+ Pages Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Pagination URL Not In Anchor Tag Inlinks",
        aliases: &[],
        file_stem: "Pagination:Pagination URL Not In Anchor Tag Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Non-200 Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Non-200 Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Unlinked Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Unlinked Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Non-Indexable Inlinks",
        aliases: &[],
        file_stem: "Pagination:Non-Indexable Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Multiple Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Multiple Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Pagination Loop Inlinks",
        aliases: &[],
        file_stem: "Pagination:Pagination Loop Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
    TableEntry {
        name: "Pagination:Sequence Error Inlinks",
        aliases: &[],
        file_stem: "Pagination:Sequence Error Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run: stub_run,
    },
];
