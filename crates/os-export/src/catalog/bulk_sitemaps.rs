//! Sitemaps: Exporte je Filter (OUT-057).
//!
//! STUB(M2-D13): Alle Einträge liefern bis zur Umsetzung nur die Kopfzeile (K-14, `stub_run`). Das Paket ersetzt `run`
//! je Eintrag durch die Auswertung und ergänzt oder schärft Kopfzeile, Voraussetzungen und Aliase; der Katalog in
//! `catalog/mod.rs` bleibt unverändert, weil er nur `entries()` aufruft.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{stub_run, TableEntry};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

// STUB(M2-D13)
const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Sitemaps:URLs In Sitemap Details",
        aliases: &["Sitemaps:URLs In Sitemap Inlinks"],
        file_stem: "Sitemaps:URLs In Sitemap Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:URLs Not In Sitemap Details",
        aliases: &["Sitemaps:URLs Not In Sitemap Inlinks"],
        file_stem: "Sitemaps:URLs Not In Sitemap Details",
        header: &["Address"],
        header_fn: None,
        requires: &[],
        owner: "M2-D13",
        run: stub_run,
    },
    TableEntry {
        name: "Sitemaps:Non-Indexable URLs In Sitemap Details",
        aliases: &["Sitemaps:Non-Indexable URLs In Sitemap Inlinks"],
        file_stem: "Sitemaps:Non-Indexable URLs In Sitemap Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:URLs In Multiple Sitemaps Details",
        aliases: &["Sitemaps:URLs In Multiple Sitemaps Inlinks"],
        file_stem: "Sitemaps:URLs In Multiple Sitemaps Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:XML Sitemap With Over 50k URLs Details",
        aliases: &["Sitemaps:XML Sitemap With Over 50k URLs Inlinks"],
        file_stem: "Sitemaps:XML Sitemap With Over 50k URLs Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:XML Sitemap Over 50MB Details",
        aliases: &["Sitemaps:XML Sitemap Over 50MB Inlinks"],
        file_stem: "Sitemaps:XML Sitemap Over 50MB Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:Orphan URLs Details",
        aliases: &["Sitemaps:Orphan URLs Inlinks"],
        file_stem: "Sitemaps:Orphan URLs Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
            "Found Via",
        ],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D13",
        run: stub_run,
        header_fn: None,
    },
];
