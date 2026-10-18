//! Auswahl-Export des Herkunftspfades (OUT-023, K2-42).
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::{ExportFileInfo, Format};
use os_analyze::{details::DetailReq, ColumnKind};
use std::path::Path;

/// Crawl Path ist ausschließlich ein Auswahl-Export, kein Eintrag im Reports-Menü.
pub fn entries() -> &'static [TableEntry] {
    &[]
}

pub fn export_detail(
    job: &TableJob<'_>,
    req: &DetailReq<'_>,
    path: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let mut writer = crate::output::open_writer(
        path,
        format,
        "Crawl Path Report",
        &crate::pipeline::writer_options(job.cfg),
    )?;
    writer.header(
        &os_store::queries::crawl_path::HEADER
            .iter()
            .map(|s| (*s).into())
            .collect::<Vec<_>>(),
    )?;
    // Ein Export umfasst die gesamte Auswahl, unabhängig von der sichtbaren Detailseite.
    let result = os_store::queries::crawl_path::paths(job.file, req.url_ids, 0, u64::MAX)?;
    let kinds = [
        ColumnKind::Url,
        ColumnKind::Int,
        ColumnKind::Url,
        ColumnKind::Int,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Int,
    ];
    for row in result.data.rows {
        writer.row(&kinds, &row)?;
    }
    let mut info = writer.finish()?;
    info.warnings.extend(result.notes);
    Ok(info)
}
