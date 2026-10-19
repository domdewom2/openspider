//! Hreflang reports OUT-090–096 and selection exports TECH-089.

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::types::TableWriter;
use crate::{ExportError, Format};
use os_analyze::details::DetailReq;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Hreflang:All Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang:All Hreflang URLs",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Non-200 Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang Non-200 Hreflang URLs Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
            "Status",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Unlinked Hreflang URLs",
        aliases: &[],
        file_stem: "Hreflang Unlinked Hreflang URLs Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Annotation Source",
            "Status Code",
            "Indexability",
            "Is Self Reference",
        ],
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Missing Return Links",
        aliases: &[],
        file_stem: "Hreflang Missing Return Links Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Hreflang URL Status Code",
            "Hreflang URL Indexability",
            "Missing Return Link To",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Inconsistent Language & Region Return Links",
        aliases: &["Hreflang:Inconsistent Language Return Links"],
        file_stem: "Hreflang Inconsistent Language & Region Return Links Report",
        header: &[
            "Address",
            "Hreflang URL",
            "Expected Language",
            "Actual Language",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Non Canonical Return Links",
        aliases: &[],
        file_stem: "Hreflang:Non Canonical Return Links",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Canonical Of Hreflang URL",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
    TableEntry {
        name: "Hreflang:Noindex Return Links",
        aliases: &[],
        file_stem: "Hreflang Noindex Return Links Report",
        header: &[
            "Address",
            "Hreflang Language",
            "Hreflang URL",
            "Indexability Status",
        ],
        requires: &[],
        owner: "M2-D07",
        run: run_report,
        header_fn: None,
    },
];

fn run_report(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let group: Vec<_> = entries().iter().collect();
    let canonical = crate::catalog::find_table(&group, job.entry_name)
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?
        .name;
    let filter = match canonical {
        "Hreflang:All Hreflang URLs" => "All",
        "Hreflang:Non Canonical Return Links" => "Non-Canonical Return Links",
        "Hreflang:Inconsistent Language Return Links" => {
            "Inconsistent Language & Region Return Links"
        }
        name => name.strip_prefix("Hreflang:").unwrap_or(name),
    };
    let mut offset = 0;
    loop {
        let data = os_analyze::tabs::hreflang::report_rows(job.file, job.cfg, filter, offset)?;
        let kinds = data.columns.into_iter().map(|(_, k)| k).collect::<Vec<_>>();
        for row in &data.rows {
            writer.row(&kinds, row)?;
        }
        offset += data.rows.len() as u64;
        if offset >= data.total || data.rows.is_empty() {
            return Ok(offset);
        }
    }
}

/// Exports all selected annotations, paging through SQL in bounded blocks. Request sort is retained;
/// offset/limit belong to the viewport and do not restrict the selected URLs' export.
pub fn export_detail(
    job: &TableJob<'_>,
    req: &DetailReq<'_>,
    path: &std::path::Path,
    format: Format,
) -> Result<crate::types::ExportFileInfo, ExportError> {
    let options = crate::pipeline::writer_options(job.cfg);
    let mut writer = crate::output::open_writer(path, format, job.entry_name, &options)?;
    let mut offset = 0;
    let mut notes = Vec::new();
    loop {
        let page_req = DetailReq {
            offset,
            limit: 500,
            ..*req
        };
        let data = os_analyze::tabs::hreflang::detail_rows(job.file, job.cfg, &page_req)?;
        if offset == 0 {
            writer.header(
                &data
                    .columns
                    .iter()
                    .map(|(n, _)| n.clone())
                    .collect::<Vec<_>>(),
            )?;
            notes = data.notes;
        }
        let kinds = data.columns.into_iter().map(|(_, k)| k).collect::<Vec<_>>();
        for row in &data.rows {
            writer.row(&kinds, row)?;
        }
        offset += data.rows.len() as u64;
        if offset >= data.total || data.rows.is_empty() {
            break;
        }
    }
    let mut info = writer.finish()?;
    info.warnings.extend(
        notes
            .into_iter()
            .map(|(key, note)| format!("{key}: {note}")),
    );
    Ok(info)
}

#[cfg(test)]
mod tests;
