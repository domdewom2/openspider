//! Stored page content shared by MCP and the streaming NDJSON export (AGENT-017/019).
use crate::{ExportError, ExportFileInfo};
use os_model::{CrawlConfig, UrlId};
use os_store::{queries, CrawlFile};
use serde_json::json;
use std::io::{BufWriter, Write};
use std::path::Path;

pub const NAME: &str = "page_content_ndjson";
pub const STORAGE_HINT: &str =
    "Enable extraction.store_html (HTML speichern) or extraction.store_rendered_html before crawling.";

/// Formats of the table writers; NDJSON is specific to the page-content export.
pub fn formats() -> Vec<&'static str> {
    vec![
        crate::Format::Csv.extension(),
        crate::Format::Xlsx.extension(),
        "ndjson",
    ]
}

pub fn matches(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(NAME)
}

pub struct PageContent {
    pub html: String,
    pub text: String,
    pub source: &'static str,
}

/// Prefer a saved rendered DOM, falling back to saved original HTML. Never fetches.
pub fn read(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    id: UrlId,
) -> Result<Option<PageContent>, ExportError> {
    let rendered = queries::stored_rendered_html(file, id)?;
    let (stored, source) = match rendered {
        Some(html) => (Some(html), "rendered"),
        None => (queries::stored_html(file, id)?, "original"),
    };
    let Some(stored) = stored else {
        return Ok(None);
    };
    let content_type = stored.charset.map(|c| format!("text/html; charset={c}"));
    let (html, _) = os_parse::charset::decode(&stored.bytes, content_type.as_deref(), &[]);
    let text =
        os_parse::content::area_text(&os_parse::dom::Document::parse(&html), &cfg.content.area);
    Ok(Some(PageContent { html, text, source }))
}

/// One JSON line per HTML URL, including URLs without stored content (null text and a hint).
/// Traverses URLs in address order and keeps at most one page in memory.
pub fn export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    include_html: bool,
) -> Result<ExportFileInfo, ExportError> {
    export_with(file, cfg, out_dir, include_html, None)
}

/// App batch variant with cancellation and row progress, using the identical NDJSON writer.
pub fn export_with(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    include_html: bool,
    ctx: Option<&crate::types::ExportCtx<'_>>,
) -> Result<ExportFileInfo, ExportError> {
    if let Some(ctx) = ctx {
        ctx.checkpoint()?;
    }
    let io = |source| ExportError::Io {
        path: out_dir.into(),
        source,
    };
    std::fs::create_dir_all(out_dir).map_err(io)?;
    let path = out_dir.join(format!("{NAME}.ndjson"));
    let pending = crate::output::PendingFile::new(&path)?;
    let mut writer =
        BufWriter::new(std::fs::File::create(crate::fs_paths::fs_path(&pending.path)).map_err(io)?);
    let (rows, missing) = crate::pipeline::snapshot(file, || {
        let mut stmt = crate::links_export::sql(file.conn().prepare(
            "SELECT id,address,status_code,title_1 FROM urls WHERE content_class='HTML' ORDER BY address COLLATE BINARY,id"
        ))?;
        let mut urls = crate::links_export::sql(stmt.query([]))?;
        let (mut count, mut missing) = (0, 0);
        while let Some(row) = crate::links_export::sql(urls.next())? {
            if let Some(ctx) = ctx {
                ctx.checkpoint()?;
            }
            let id = UrlId(crate::links_export::sql(row.get(0))?);
            let page = read(file, cfg, id)?;
            let mut value = json!({
                "url": crate::links_export::sql(row.get::<_, String>(1))?,
                "status_code": crate::links_export::sql(row.get::<_, Option<u16>>(2))?,
                "title": crate::links_export::sql(row.get::<_, Option<String>>(3))?,
                "text": page.as_ref().map(|p| &p.text),
                "source": page.as_ref().map(|p| p.source),
            });
            if include_html {
                value["html"] = json!(page.as_ref().map(|p| &p.html));
            }
            if page.is_none() {
                missing += 1;
                value["message"] = json!(STORAGE_HINT);
            }
            serde_json::to_writer(&mut writer, &value)
                .map_err(|e| ExportError::Write(e.to_string()))?;
            writer.write_all(b"\n").map_err(io)?;
            count += 1;
            if let Some(ctx) = ctx {
                (ctx.progress)(count);
            }
        }
        Ok((count, missing))
    })?;
    writer.flush().map_err(io)?;
    let bytes = writer.get_ref().metadata().map_err(io)?.len();
    drop(writer);
    if let Some(ctx) = ctx {
        ctx.checkpoint()?;
    }
    let info = ExportFileInfo {
        path,
        rows,
        bytes,
        warnings: if missing == 0 {
            vec![]
        } else {
            vec![format!(
                "{missing} HTML URLs have no stored content. {STORAGE_HINT}"
            )]
        },
    };
    if rows == 0 && ctx.is_some_and(|ctx| ctx.skip_empty) {
        return Ok(info);
    }
    pending.publish(info)
}

/// Reuse the parser's bounded PNG preview without adding a parser dependency to MCP.
pub fn screenshot_preview(
    bytes: &[u8],
    max_side: u32,
    max_bytes: usize,
) -> Option<(Vec<u8>, u32, u32)> {
    os_parse::images::png_preview(bytes, max_side, max_bytes)
}
