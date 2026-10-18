//! Redirect- und Canonical-Ketten (OUT-083).
use super::canonical_chains;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::TableWriter;

pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Redirects:Redirect & Canonical Chains",
    aliases: &[],
    file_stem: "Redirects:Redirect & Canonical Chains",
    header: crate::catalog::CHAIN_HEADER,
    header_fn: Some(header),
    requires: &[],
    owner: "M2-D04",
    run,
}];
fn header(job: &TableJob<'_>) -> Result<Vec<String>, ExportError> {
    canonical_chains::make_header(job, false)
}
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    canonical_chains::write(job, writer, false)
}
