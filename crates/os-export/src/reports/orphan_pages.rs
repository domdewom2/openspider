//! Orphan Pages: Sitemap-Anteil (OUT-099). GA/GSC ergänzt M2-I06.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::TableWriter;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Orphan Pages",
    aliases: &[],
    file_stem: "Orphan Pages",
    header: &["Address", "Source", "Status Code", "Indexability"],
    header_fn: None,
    requires: &[
        ReportRequirement::CrawlAnalysis,
        ReportRequirement::OrphanSourceAvailable,
    ],
    owner: "M2-D13",
    run,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    crate::catalog::bulk_sitemaps::write_filter(job, writer, "Orphan URLs", true)
}
