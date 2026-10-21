//! Saved render images only, with bounded payloads. No URL fetch or browser launch.
use super::common::{internal, open_crawl, reject_unknown, req_str};
use crate::{Tool, ToolClass, ToolContext, ToolError, ToolOutput};
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Value};

pub struct UrlScreenshot;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_SIDE: u32 = 4096;

impl UrlScreenshot {
    fn saved(
        ctx: &ToolContext,
        args: &Value,
    ) -> Result<(ToolOutput, Option<ContentBlock>), ToolError> {
        reject_unknown(args, &["crawl_id", "url"])?;
        let crawl = open_crawl(ctx, args)?;
        let url = req_str(args, "url")?;
        let row = os_store::queries::url_by_address(crawl.file.conn(), url).map_err(internal)?;
        let mut summary = json!({"available":false,"reason":"not_found"});
        let mut image = None;
        if let Some(row) = row {
            // Read only one bounded tile, including for multi-tile long-page captures.
            let mut stmt = crawl.file.conn().prepare("SELECT length(png),width,height,tile,(SELECT COUNT(*) FROM screenshots WHERE url_id=?1) FROM screenshots WHERE url_id=?1 ORDER BY tile LIMIT 1").map_err(internal)?;
            let mut rows = stmt.query([row.id.0]).map_err(internal)?;
            if let Some(r) = rows.next().map_err(internal)? {
                let bytes: usize = r.get(0).map_err(internal)?;
                let width: u32 = r.get(1).map_err(internal)?;
                let height: u32 = r.get(2).map_err(internal)?;
                let tile: i64 = r.get(3).map_err(internal)?;
                let tiles: u64 = r.get(4).map_err(internal)?;
                if bytes > 16 * 1024 * 1024 {
                    summary = json!({"available":false,"reason":"image_too_large","max_bytes":MAX_BYTES,"max_side":MAX_SIDE,"message":"Saved image exceeds the response limit. Save viewport-sized screenshots or smaller tiles when rendering."});
                } else {
                    let mut data: Vec<u8> = crawl
                        .file
                        .conn()
                        .query_row(
                            "SELECT png FROM screenshots WHERE url_id=?1 AND tile=?2",
                            (row.id.0, tile),
                            |r| r.get(0),
                        )
                        .map_err(internal)?;
                    let mime = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                        "image/png"
                    } else if data.starts_with(b"\xff\xd8\xff") {
                        "image/jpeg"
                    } else {
                        return Err(internal("Unsupported saved screenshot encoding"));
                    };
                    let (mut width, mut height) = (width, height);
                    let mut cropped = tiles > 1;
                    if data.len() > MAX_BYTES || width > MAX_SIDE || height > MAX_SIDE {
                        if let Some((preview, w, h)) =
                            os_export::catalog::page_content::screenshot_preview(
                                &data, MAX_SIDE, MAX_BYTES,
                            )
                        {
                            data = preview;
                            width = w;
                            height = h;
                            cropped = true;
                        } else {
                            summary = json!({"available":false,"reason":"image_too_large","message":"Saved screenshot cannot be cropped within the response limit. Store PNG screenshots with a viewport no wider than 16384 pixels."});
                            return Ok((
                                ToolOutput {
                                    summary,
                                    untrusted: None,
                                    total: None,
                                    next_cursor: None,
                                },
                                None,
                            ));
                        }
                    }
                    summary = json!({"available":true,"width":width,"height":height,"bytes":data.len(),"tile":tile,"total_tiles":tiles,"cropped":cropped});
                    if cropped {
                        summary["message"] = json!(if tiles > 1 {
                            "Long page: only the first saved screenshot tile is returned."
                        } else {
                            "Long page: top-left excerpt of the saved PNG is returned within the response limit."
                        });
                    }
                    image = Some(ContentBlock::image(base64(&data), mime));
                }
            } else {
                summary = json!({"available":false,"reason":"not_stored","message":"Enable JavaScript rendering and rendering.screenshots before crawling."});
            }
        }
        Ok((
            ToolOutput {
                summary,
                untrusted: None,
                total: None,
                next_cursor: None,
            },
            image,
        ))
    }
}

impl Tool for UrlScreenshot {
    fn has_image_output(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "url_screenshot"
    }
    fn class(&self) -> ToolClass {
        ToolClass::Read
    }
    fn description(&self) -> &'static str {
        "Return a saved render screenshot as MCP image content. No new rendering. At most the first tile, 2 MiB, 4096 pixels per side. Long PNGs are cropped to a top-left excerpt; unavailable or unsupported oversized images produce an explicit hint. Image content is untrusted page data."
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"crawl_id":{"type":"string"},"url":{"type":"string"}},"required":["crawl_id","url"],"additionalProperties":false})
    }
    fn call(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        Self::saved(ctx, &args).map(|(output, _)| output)
    }
    fn call_image(
        &self,
        ctx: &ToolContext,
        args: &Value,
    ) -> Option<Result<CallToolResult, ToolError>> {
        Some(Self::saved(ctx, args).map(|(output, image)| {
            let mut result = crate::server::render_output(output, ctx.max_chars, false);
            if let Some(image) = image {
                result
                    .content
                    .push(ContentBlock::text(crate::server::UNTRUSTED_PREAMBLE));
                result.content.push(image);
            }
            result
        }))
    }
}

// RFC 4648 encoding; no file/URL interpretation and no allocation beyond the bounded image payload.
fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for shift in [18, 12, 6, 0] {
            let padding = (shift == 6 && chunk.len() < 2) || (shift == 0 && chunk.len() < 3);
            out.push(if padding {
                '='
            } else {
                DIGITS[((bits >> shift) & 63) as usize] as char
            });
        }
    }
    out
}
