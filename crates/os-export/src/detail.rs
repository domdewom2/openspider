//! Detail-Export (Inlinks/Outlinks) für eine URL-Liste (OUT-021, OUT-022, ONPAGE-192).
//!
//! Das ist der Datenteil der Lower-Tabs „Inlinks“ und „Outlinks“ einer oder mehrerer in der Haupttabelle
//! gewählter URLs: alle Links im Standard-Linkformat (15 Spalten, siehe [`crate::links_export`]) in einer
//! Datei. Die Reihenfolge folgt der übergebenen URL-Liste (doppelte Einträge zählen einmal); innerhalb einer
//! URL stehen Inlinks nach `Source` und Dokumentreihenfolge, Outlinks in Dokumentreihenfolge. Die
//! Verarbeitung streamt (auch bei 50 000 URLs).

use std::collections::HashSet;
use std::path::Path;

use os_model::{UrlId, Value};
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::links_export::{link_header, link_kinds, sql, stream_links, LinkOrder};
use crate::types::{DetailKind, ExportCtx, ExportFileInfo, Format};

/// Exportiert Inlinks bzw. Outlinks der angegebenen URLs im Standard-Linkformat (ONPAGE-192).
///
/// `out` ist der Pfad der Zieldatei. Schreiboptionen (BOM, Zeilenende) stammen aus der in der Crawl-Datei
/// gespeicherten Konfiguration. Die Datei hat immer eine Kopfzeile, auch ohne Links.
pub fn export_detail(
    file: &CrawlFile,
    detail: DetailKind,
    urls: &[UrlId],
    out: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let cancel = os_model::CancelToken::new();
    let mut ctx = ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    export_detail_with(file, detail, urls, out, format, &mut ctx)?
        .ok_or_else(|| ExportError::Write("Missing detail output".into()))
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_detail_with_options(
    file: &CrawlFile,
    detail: DetailKind,
    urls: &[UrlId],
    out: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let cancel = os_model::CancelToken::new();
    let mut ctx = ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    export_detail_with_writer_options(file, detail, urls, out, format, &mut ctx, opts)?
        .ok_or_else(|| ExportError::Write("Missing detail output".into()))
}

/// Multi-selection detail export with the same control and workbook support as tab exports.
pub fn export_detail_with(
    file: &CrawlFile,
    detail: DetailKind,
    urls: &[UrlId],
    out: &Path,
    format: Format,
    ctx: &mut ExportCtx<'_>,
) -> Result<Option<ExportFileInfo>, ExportError> {
    ctx.checkpoint()?;
    if file.conn().is_autocommit() {
        os_store::link_index::ensure_if_idle(file)?;
    }
    crate::pipeline::snapshot(file, || {
        let opts = crate::pipeline::writer_options(&file.config()?);
        export_detail_with_writer_options(file, detail, urls, out, format, ctx, &opts)
    })
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_detail_with_writer_options(
    file: &CrawlFile,
    detail: DetailKind,
    urls: &[UrlId],
    out: &Path,
    format: Format,
    ctx: &mut ExportCtx<'_>,
    opts: &crate::types::WriterOptions,
) -> Result<Option<ExportFileInfo>, ExportError> {
    ctx.checkpoint()?;
    if file.conn().is_autocommit() {
        os_store::link_index::ensure_if_idle(file)?;
    }
    crate::pipeline::snapshot(file, || {
        let (sheet, where_sql) = match detail {
            DetailKind::Inlinks => ("Inlinks", "l.target_id = :url_id"),
            DetailKind::Outlinks => ("Outlinks", "l.source_id = :url_id"),
        };
        let cancel = ctx.cancel.clone();
        crate::output::write_table(out, format, sheet, opts, ctx, |writer| {
            let kinds = link_kinds();
            writer.header(&link_header())?;
            let mut seen = HashSet::new();
            let ids: Vec<i64> = urls
                .iter()
                .filter_map(|id| seen.insert(id.0).then_some(id.0))
                .collect();
            if detail == DetailKind::Inlinks
                && ids.len() > 1
                && !os_store::link_index::present(file.conn())?
            {
                return stream_unindexed_inlinks(file, &ids, writer);
            }
            for id in ids {
                cancel
                    .checkpoint()
                    .map_err(|_| ExportError::Write("Export abgebrochen".into()))?;
                let params = [(":url_id".to_string(), Value::Int(id))];
                stream_links(
                    file,
                    where_sql,
                    &params,
                    LinkOrder::Outlinks,
                    &kinds,
                    writer,
                )?;
            }
            Ok(())
        })
    })
}

/// With no target index (a running crawl), join the selected IDs to one link
/// scan rather than performing a complete scan for each selected URL.
fn stream_unindexed_inlinks(
    file: &CrawlFile,
    ids: &[i64],
    writer: &mut dyn crate::types::TableWriter,
) -> Result<(), ExportError> {
    let selected = ids
        .iter()
        .enumerate()
        .map(|(order, id)| format!("({id},{order})"))
        .collect::<Vec<_>>()
        .join(",");
    // All values interpolated above are store IDs and ordinals, never user SQL.
    let query = format!(
        "WITH selected(url_id, ordinal) AS (VALUES {selected})
        SELECT l.link_type, s.address, COALESCE(t.address,l.target_raw), t.size_bytes,
            l.alt_text, l.anchor,
            CASE WHEN t.state='done' AND t.status_code>0 THEN t.status_code END,
            CASE WHEN t.id IS NULL THEN NULL WHEN t.state<>'done' OR t.status_code IS NULL
                THEN 'Not Crawled' ELSE t.status END,
            CASE WHEN l.follow<>0 THEN 'True' ELSE 'False' END,
            l.target_attr,l.rel,l.path_type,l.link_path,l.link_position,l.link_origin
        FROM links l CROSS JOIN selected q ON q.url_id=l.target_id
        JOIN urls s ON s.id=l.source_id LEFT JOIN urls t ON t.id=l.target_id
        ORDER BY q.ordinal,s.address,l.position,l.id"
    );
    let mut stmt = sql(file.conn().prepare(&query))?;
    let mut rows = sql(stmt.query([]))?;
    let kinds = link_kinds();
    let mut values = Vec::with_capacity(kinds.len());
    while let Some(row) = sql(rows.next())? {
        values.clear();
        for column in 0..kinds.len() {
            values.push(match column {
                3 | 6 => sql(row.get::<_, Option<i64>>(column))?.map_or(Value::Null, Value::Int),
                _ => sql(row.get::<_, Option<String>>(column))?.map_or(Value::Null, Value::Text),
            });
        }
        writer.row(&kinds, &values)?;
    }
    Ok(())
}
