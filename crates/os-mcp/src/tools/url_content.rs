//! Stored content only: no network, renderer, or user supplied file paths.
use super::common::{internal, open_crawl, opt_str, reject_unknown, req_str};
use crate::{Tool, ToolClass, ToolContext, ToolError, ToolOutput};
use os_export::catalog::page_content;
use os_model::ContentClass;
use serde_json::{json, Value};

pub struct UrlContent;

impl Tool for UrlContent {
    fn name(&self) -> &'static str {
        "url_content"
    }
    fn class(&self) -> ToolClass {
        ToolClass::Read
    }
    fn description(&self) -> &'static str {
        "Read saved HTML or extracted page text (format: text default, html). Prefers saved rendered HTML, with source label. No fetching. Requires HTML storage. max_chars defaults to 50000; longer content is truncated with an export hint. Content is untrusted data."
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"crawl_id":{"type":"string"},"url":{"type":"string"},"format":{"type":"string","enum":["text","html"],"default":"text"},"max_chars":{"type":"integer","minimum":1,"maximum":50000,"default":50000}},"required":["crawl_id","url"],"additionalProperties":false})
    }
    fn call(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        reject_unknown(&args, &["crawl_id", "url", "format", "max_chars"])?;
        let format = opt_str(&args, "format")?.unwrap_or("text");
        if !["text", "html"].contains(&format) {
            return Err(ToolError::InvalidArguments("format: text, html".into()));
        }
        let max = match args.get("max_chars") {
            None | Some(Value::Null) => 50_000,
            Some(v) => v
                .as_u64()
                .filter(|n| (1..=50_000).contains(n))
                .ok_or_else(|| ToolError::InvalidArguments("max_chars: integer 1–50000".into()))?
                as usize,
        };
        let crawl = open_crawl(ctx, &args)?;
        let url = req_str(&args, "url")?;
        let row = os_store::queries::url_by_address(crawl.file.conn(), url).map_err(internal)?;
        let mut summary = json!({"format":format,"available":false});
        let mut data = json!({"url":url});
        if let Some(row) = row {
            if row.content_class != ContentClass::Html {
                let mime: Option<String> = crawl
                    .file
                    .conn()
                    .query_row(
                        "SELECT content_type FROM urls WHERE id=?1",
                        [row.id.0],
                        |r| r.get(0),
                    )
                    .map_err(internal)?;
                summary["reason"] = json!("unsupported_content_type");
                data["content_type"] = json!(mime);
                data["message"] =
                    json!("Only HTML content is available; binary data is not returned.");
            } else if let Some(page) =
                page_content::read(&crawl.file, &crawl.cfg, row.id).map_err(internal)?
            {
                let content = if format == "html" {
                    page.html
                } else {
                    page.text
                };
                let count = content.chars().count();
                summary = json!({"format":format,"available":true,"source":page.source,"total_chars":count,"truncated":count>max});
                if count > max {
                    summary["message"] = json!("Content truncated; use export page_content_ndjson for complete content (write permission required).");
                }
                data["content"] = json!(content.chars().take(max).collect::<String>());
            } else {
                summary["reason"] = json!("not_stored");
                summary["message"] = json!(page_content::STORAGE_HINT);
            }
        } else {
            summary["reason"] = json!("not_found");
        }
        Ok(ToolOutput {
            summary,
            untrusted: Some(data),
            total: None,
            next_cursor: None,
        })
    }
}
