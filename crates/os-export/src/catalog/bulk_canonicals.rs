//! Canonicals: Exporte je Filter (OUT-054).
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
        name: "Canonicals:Contains Canonical Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Contains Canonical Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Self Referencing Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Self Referencing Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Canonicalised Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Canonicalised Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Missing Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Missing Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Multiple Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Multiple Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Multiple Conflicting Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Multiple Conflicting Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Non-Indexable Canonical Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Non-Indexable Canonical Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Canonical Is Relative Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Canonical Is Relative Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Unlinked Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Unlinked Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Invalid Attribute In Annotation Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Invalid Attribute In Annotation Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Contains Fragment URL Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Contains Fragment URL Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
    TableEntry {
        name: "Canonicals:Outside <head> Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Outside <head> Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D04",
        run: stub_run,
    },
];
