//! Consolidated segment exports reuse the saved segment exporter and disk-backed target adapters.
use crate::{plan::Plan, registry::Resolved, reporter::Reporter, signals::Signals};
use os_export::types::{TableWriter, WorkbookTarget};
use os_model::CrawlConfig;
use os_store::CrawlFile;
use std::path::Path;

pub async fn export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    plan: &Plan,
    out: &Path,
    signals: &Signals,
    rep: &Reporter,
) -> Result<usize, String> {
    let token = signals.cancel_token();
    let snapshot = file
        .conn()
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    let client = plan
        .google
        .as_ref()
        .map(|target| target.client(cfg, &plan.paths))
        .transpose()
        .map_err(|e| e.message)?;
    let crawl = file.meta().map_err(|e| e.to_string())?.name;
    let crawl = if crawl.is_empty() { "crawl" } else { &crawl };
    let mut count = 0;
    let mut names = os_export::fs_paths::NameRegistry::new();
    for scope in
        os_export::segment::selections(file, plan.segment.as_deref()).map_err(|e| e.to_string())?
    {
        token.checkpoint().map_err(|e| e.to_string())?;
        let segment = scope.as_ref().and_then(|scope| scope.name.as_deref());
        let staging = crate::staging::Staging::new(out).map_err(|e| e.to_string())?;
        let mut tables = Vec::new();
        for item in &plan.exports {
            let (kind, name) = match item {
                Resolved::Tab { tab, filter } => ("tab", format!("{tab}:{filter}")),
                Resolved::Bulk(name) => ("bulk", name.clone()),
                Resolved::Issues => ("bulk", os_export::catalog::bulk_issues::BULK_NAME.into()),
                Resolved::Report(name) => ("report", name.clone()),
            };
            let selection = os_export::types::ExportSelection {
                kind: kind.into(),
                name,
            };
            let files = os_export::csv::with_header_labels(&plan.header_labels, || {
                os_export::segment::export_all(
                    file,
                    cfg,
                    &selection,
                    segment,
                    &staging.path,
                    os_export::Format::Csv,
                )
            })
            .map_err(|e| e.to_string())?;
            for mut info in files {
                for warning in &info.warnings {
                    rep.warn(warning);
                }
                if plan.batch.is_some_and(|b| b.skip_empty) && info.rows == 0 {
                    continue;
                }
                if info.path.extension().is_some_and(|e| e == "csv") {
                    tables.push(crate::google::table(&info.path)?);
                } else {
                    let relative = info
                        .path
                        .strip_prefix(&staging.path)
                        .map_err(|e| e.to_string())?;
                    let path = out.join(relative);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::copy(&info.path, &path).map_err(|e| e.to_string())?;
                    info.path = path;
                    rep.note_export(&info.path);
                    count += 1;
                }
            }
        }
        if tables.is_empty() {
            continue;
        }
        let requested = os_export::naming::file_name(&format!("{crawl} export"), segment, "xlsx");
        let name = names.claim(&requested, &format!("{segment:?}"));
        if let Some(client) = &client {
            let name = Path::new(&name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let mut destination =
                os_export::target_gsheet::Destination::new(&plan.project, crawl, &name);
            destination.subfolder = plan.google.as_ref().and_then(|t| t.timestamp.clone());
            let result = os_export::target_gsheet::export(client, &destination, tables, &token)
                .await
                .map_err(|e| e.to_string())?;
            rep.info(format!(
                "Google Sheets: {} ({} Zeilen)",
                result.url, result.rows
            ));
            for warning in result.warnings {
                rep.warn(warning);
            }
        } else {
            let destination = out.join(&name);
            let temporary = staging.path.join("workbook.xlsx");
            let mut writer = os_export::xlsx::XlsxWriter::create(
                &temporary,
                "Export",
                &os_export::pipeline::writer_options(cfg),
            )
            .map_err(|e| e.to_string())?;
            writer.set_cancel(token.clone());
            for table in tables {
                let kinds = crate::google::kinds(&table.header);
                let sheet = writer.sheet(&table.name).map_err(|e| e.to_string())?;
                sheet.header(&table.header).map_err(|e| e.to_string())?;
                for row in table.rows {
                    token.checkpoint().map_err(|e| e.to_string())?;
                    sheet
                        .row(&kinds, &row.map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                }
            }
            let mut info = Box::new(writer).finish().map_err(|e| e.to_string())?;
            #[cfg(windows)]
            if destination.exists() {
                std::fs::remove_file(&destination).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&info.path, &destination).map_err(|e| e.to_string())?;
            info.path = destination;
            rep.note_export(&info.path);
            rep.info(format!(
                "Export: {} ({} Zeilen)",
                info.path.display(),
                info.rows
            ));
            for warning in info.warnings {
                rep.warn(warning);
            }
        }
        count += 1;
    }
    snapshot.commit().map_err(|e| e.to_string())?;
    Ok(count)
}
