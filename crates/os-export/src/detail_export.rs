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

/// Paged variant of the F05 detail export for app operations (additive K2-61).
/// Keeps memory bounded and checks cancellation between source pages and rows.
pub fn export_detail_with_ctx(
    kind: DetailKind,
    job: &TableJob<'_>,
    req: &DetailReq<'_>,
    writer: &mut dyn crate::TableWriter,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<u64, ExportError> {
    let (_, source) = os_analyze::details::app_detail_source(kind);
    let mut offset = 0;
    let mut header = None;
    loop {
        ctx.checkpoint()?;
        let page = source(
            job.file,
            job.cfg,
            &DetailReq {
                url_ids: req.url_ids,
                params: req.params,
                sort: req.sort,
                offset,
                limit: 500,
            },
        )?;
        let (names, kinds): (Vec<_>, Vec<_>) = page.columns.into_iter().unzip();
        if let Some(expected) = &header {
            if expected != &names {
                return Err(ExportError::Write(
                    "Detail columns changed in snapshot".into(),
                ));
            }
        } else {
            writer.header(&names)?;
            header = Some(names);
        }
        let count = page.rows.len() as u64;
        for row in page.rows {
            ctx.checkpoint()?;
            writer.row(&kinds, &row)?;
        }
        offset += count;
        (ctx.progress)(offset);
        if count == 0 || offset >= page.total {
            return Ok(offset);
        }
    }
}
