//! Auswahlbezogene Detail-Exporte (OUT-010).
use crate::{
    catalog::TableJob,
    error::ExportError,
    types::{ExportFileInfo, Format},
};
use os_analyze::details::{detail_source, DetailKind, DetailReq};
use std::path::Path;

pub fn export_detail(
    kind: DetailKind,
    job: &TableJob<'_>,
    req: &DetailReq<'_>,
    path: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    match kind {
        DetailKind::Hreflang => {
            return crate::reports::hreflang::export_detail(job, req, path, format)
        }
        DetailKind::CrawlPath => {
            return crate::reports::crawl_path::export_detail(job, req, path, format)
        }
        DetailKind::StructuredDataItems
            if req.params.get("tree").and_then(|v| v.as_bool()) == Some(true) =>
        {
            return crate::catalog::bulk_sd::export_detail(job, req, path, format)
        }
        _ => {}
    }
    let (_, source) = detail_source(kind);
    let data = source(job.file, job.cfg, req)?;
    let options = crate::pipeline::writer_options(job.cfg);
    let mut writer = crate::output::open_writer(path, format, job.entry_name, &options)?;
    let (names, kinds): (Vec<_>, Vec<_>) = data.columns.into_iter().unzip();
    writer.header(&names)?;
    for row in data.rows {
        writer.row(&kinds, &row)?;
    }
    let mut info = writer.finish()?;
    info.warnings.extend(
        data.notes
            .into_iter()
            .map(|(name, note)| format!("{name}: {note}")),
    );
    Ok(info)
}
