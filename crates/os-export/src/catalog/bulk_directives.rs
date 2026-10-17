//! Directives: Exporte je Filter (OUT-055).
//!
//! STUB(M2-D06): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D06)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Directives:Index Inlinks",
        aliases: &[],
        file_stem: "Directives:Index Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Noindex Inlinks",
        aliases: &[],
        file_stem: "Directives:Noindex Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Follow Inlinks",
        aliases: &[],
        file_stem: "Directives:Follow Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Nofollow Inlinks",
        aliases: &[],
        file_stem: "Directives:Nofollow Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:None Inlinks",
        aliases: &[],
        file_stem: "Directives:None Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoArchive Inlinks",
        aliases: &[],
        file_stem: "Directives:NoArchive Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoSnippet Inlinks",
        aliases: &[],
        file_stem: "Directives:NoSnippet Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Max-Snippet Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Snippet Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Max-Image-Preview Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Image-Preview Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Max-Video-Preview Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Video-Preview Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoODP Inlinks",
        aliases: &[],
        file_stem: "Directives:NoODP Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoYDIR Inlinks",
        aliases: &[],
        file_stem: "Directives:NoYDIR Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoImageIndex Inlinks",
        aliases: &[],
        file_stem: "Directives:NoImageIndex Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:NoTranslate Inlinks",
        aliases: &[],
        file_stem: "Directives:NoTranslate Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Unavailable_After Inlinks",
        aliases: &[],
        file_stem: "Directives:Unavailable_After Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Refresh Inlinks",
        aliases: &[],
        file_stem: "Directives:Refresh Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
    TableEntry {
        name: "Directives:Outside <head> Inlinks",
        aliases: &[],
        file_stem: "Directives:Outside <head> Inlinks",
        header: crate::catalog::LINK_HEADER,
        requires: &[],
        owner: "M2-D06",
        run: stub_run,
    },
];
