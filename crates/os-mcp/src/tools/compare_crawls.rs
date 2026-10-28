//! Read-only comparison through the same classifier as the app (AGENT-015).
use super::common::{internal, open_crawl, opt_str, reject_unknown, req_str};
use crate::{paging, Tool, ToolClass, ToolContext, ToolError, ToolOutput};
use os_analyze::compare::{change, CompareClass, Comparison, CrawlSide};
use os_model::{CancelToken, CrawlState, MappingRule};
use serde_json::{json, Value};
pub struct CompareCrawls;
impl Tool for CompareCrawls {
    fn name(&self) -> &'static str {
        "compare_crawls"
    }
    fn class(&self) -> ToolClass {
        ToolClass::Read
    }
    fn description(&self) -> &'static str {
        "Compare a (Current) to b (Previous): Added/New/Removed/Missing per tab/filter and change detection counts. Optional element returns changed URLs with limit/cursor; url_mapping accepts saved-format mapping rules."
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","required":["a","b"],"properties":{"a":{"type":"string"},"b":{"type":"string"},"element":{"type":"string"},"url_mapping":{"type":"array","items":{"type":"object"}},"limit":{"type":"integer","minimum":1,"maximum":500},"cursor":{"type":"string"}},"additionalProperties":false})
    }
    fn call(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        reject_unknown(
            &args,
            &["a", "b", "element", "url_mapping", "limit", "cursor"],
        )?;
        let a = req_str(&args, "a")?;
        let b = req_str(&args, "b")?;
        let current = open_crawl(ctx, &json!({"crawl_id":a}))?;
        let previous = open_crawl(ctx, &json!({"crawl_id":b}))?;
        for crawl in [&current, &previous] {
            if crawl
                .file
                .effective_state(ctx.clock.as_ref())
                .map_err(internal)?
                == CrawlState::Running
                || ctx
                    .session
                    .as_ref()
                    .and_then(|s| s.status(&crawl.id.to_string()))
                    .is_some_and(|s| s.state == CrawlState::Running)
            {
                return Err(ToolError::InvalidArguments(format!(
                    "Crawl {} is still running",
                    crawl.id
                )));
            }
        }
        let rules: Vec<MappingRule> = args
            .get("url_mapping")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?
            .unwrap_or_else(|| current.cfg.compare.mapping.clone());
        let side = |c: super::common::OpenCrawl| {
            let path = c.file.path().to_path_buf();
            CrawlSide::from_file(c.file, path, None).map_err(internal)
        };
        let cancel = CancelToken::new();
        let mut comparison =
            Comparison::open(side(current)?, side(previous)?, &rules, &cancel).map_err(internal)?;
        if comparison.current().meta.crawl_id.to_string() != a {
            comparison.swap();
        }
        // AGENT-015: use the opened pair, including its mapping, for counts and IDs.
        let config = comparison.current().config.compare.change_detection.clone();
        change::calculate(&comparison, &config, &cancel).map_err(internal)?;
        if let Some(element) = opt_str(&args, "element")? {
            let requested = if element == "page_title" {
                "Page Titles"
            } else {
                element
            };
            let (_, filter, _) = change::ELEMENTS
                .iter()
                .find(|(id, label, _)| {
                    config.elements.contains(id)
                        && (label.eq_ignore_ascii_case(requested)
                            || id.as_str().eq_ignore_ascii_case(requested))
                })
                .ok_or_else(|| ToolError::InvalidArguments(format!("Unknown element {element}")))?;
            let ids = change::ids(&comparison, filter, true).map_err(internal)?;
            let page = paging::PageArgs::from_args(&args)?;
            let binding = format!("compare:{a}:{b}:{element}:{}", json!(rules));
            let offset = page
                .cursor
                .as_deref()
                .map(|c| paging::decode_cursor(c, &binding))
                .transpose()?
                .unwrap_or(0);
            let mut urls = Vec::new();
            for id in ids.iter().skip(offset as usize).take(page.limit as usize) {
                let url: String = comparison
                    .current()
                    .file
                    .conn()
                    .query_row("SELECT url FROM urls WHERE id=?1", [i64::from(id)], |r| {
                        r.get(0)
                    })
                    .map_err(internal)?;
                urls.push(url);
            }
            let next = offset + urls.len() as u64;
            return Ok(ToolOutput {
                summary: json!({"element":element}),
                untrusted: Some(json!({"urls":urls})),
                total: Some(ids.len()),
                next_cursor: (next < ids.len()).then(|| paging::encode_cursor(&binding, next)),
            });
        }
        let rows = comparison.overview(&cancel).map_err(internal)?.iter().map(|r| json!({"tab":r.tab,"filter":r.label,"current":r.current,"previous":r.previous,"added":r.class(CompareClass::Added),"new":r.class(CompareClass::New),"removed":r.class(CompareClass::Removed),"missing":r.class(CompareClass::Missing)})).collect::<Vec<_>>();
        let detection = change::counts(&comparison, &config, false)
            .map_err(internal)?
            .into_iter()
            .map(|row| json!({"element":row.element,"count":row.change}))
            .collect::<Vec<_>>();
        Ok(ToolOutput {
            summary: json!({"current":a,"previous":b,"filters":rows,"change_detection":detection}),
            untrusted: None,
            total: None,
            next_cursor: None,
        })
    }
}
