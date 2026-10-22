//! Custom JavaScript execution and configuration (INTEG-060 … 063, 071, 072).
//! Only the isolated render browser evaluates user code; no host bindings are exposed.
mod clock;
mod manage;
mod runtime;
#[cfg(test)]
mod tests;

pub use manage::{network_warning, snippet_name, PendingImport, SnippetManager};
pub use runtime::run;

use os_model::{
    CrawlConfig, CustomJsWrite, ExtKey, ExtWrite, JsSnippet, RenderingMode, UrlId, Value,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

type ExtractionPages = Arc<Mutex<HashSet<String>>>;

/// Per-render configuration. An empty default preserves rendering without snippets.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub snippets: Vec<(u32, JsSnippet)>,
    pub rendering_timeout_ms: u32,
    extracting: ExtractionPages,
}
impl RunOptions {
    /// Gate before opening a browser: exact status 200, internal, non-PDF, JS mode.
    pub fn for_response(
        cfg: &CrawlConfig,
        internal: bool,
        status: u16,
        content_type: &str,
    ) -> Self {
        let mime = content_type.split(';').next().unwrap_or("").trim();
        let mut out = Self {
            rendering_timeout_ms: cfg.rendering.page_timeout_s.saturating_mul(1000),
            ..Default::default()
        };
        if !internal
            || status != 200
            || cfg.rendering.mode != RenderingMode::JavaScript
            || mime.eq_ignore_ascii_case("application/pdf")
        {
            return out;
        }
        out.snippets = cfg
            .custom_js
            .snippets
            .iter()
            .take(100)
            .enumerate()
            .filter(|(_, s)| {
                s.content_types
                    .iter()
                    .flat_map(|t| t.split(','))
                    .any(|t| t.trim() == "*" || t.trim().eq_ignore_ascii_case(mime))
            })
            .map(|(i, s)| (i as u32, s.clone()))
            .collect();
        out
    }
    /// Reserve time for snippets in addition to the ordinary page deadline.
    pub fn page_budget_ms(&self, base: u64) -> u64 {
        let actions = self
            .snippets
            .iter()
            .filter(|(_, s)| s.kind == os_model::JsSnippetKind::Action)
            .map(|(_, s)| u64::from(s.timeout_s) * 1000)
            .max()
            .unwrap_or(0);
        let extraction: u64 = self
            .snippets
            .iter()
            .filter(|(_, s)| s.kind == os_model::JsSnippetKind::Extraction)
            .map(|(_, s)| {
                if s.timeout_s == 0 {
                    u64::from(self.rendering_timeout_ms)
                } else {
                    u64::from(s.timeout_s) * 1000
                }
            })
            .sum();
        base.saturating_add(actions).saturating_add(extraction)
    }
    /// Passive page resources stop during extraction. Explicit snippet fetch/XHR
    /// remain permitted (INTEG-072), subject to the existing crawl policy.
    pub fn blocks_resource(&self, target: &str, resource_type: &str) -> bool {
        !self.snippets.is_empty()
            && !matches!(resource_type, "Fetch" | "XHR" | "Preflight")
            && self
                .extracting
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains(target)
    }
}

/// Each attempted snippet has a trace, including actions and empty extractions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnippetResult {
    pub def_index: u32,
    pub action: bool,
    pub values: Vec<Option<String>>,
    pub error: Option<String>,
    pub started_us: u64,
    pub finished_us: u64,
}

/// Preserve empty/error executions as one NULL row so filters also include them.
pub fn writes(url_id: UrlId, results: &[SnippetResult]) -> Vec<CustomJsWrite> {
    results
        .iter()
        .filter(|r| !r.action)
        .flat_map(|r| {
            let values = if r.values.is_empty() {
                vec![None]
            } else {
                r.values.clone()
            };
            values
                .into_iter()
                .enumerate()
                .map(move |(number, value)| CustomJsWrite {
                    url_id,
                    def_index: r.def_index,
                    number: number as u32,
                    value,
                    error: r.error.clone(),
                })
        })
        .collect()
}

/// Small per-page execution log, independent of the number of returned cells.
pub fn trace(url: UrlId, results: &[SnippetResult]) -> Vec<ExtWrite> {
    vec![
        ExtWrite {
            url,
            key: ExtKey::from_static("custom_js.executed"),
            value: Value::Int(results.len() as i64),
        },
        ExtWrite {
            url,
            key: ExtKey::from_static("custom_js.log"),
            value: Value::Text(
                serde_json::to_string(
                    &results
                        .iter()
                        .map(|r| (r.def_index, r.action, r.started_us, r.finished_us, &r.error))
                        .collect::<Vec<_>>(),
                )
                .expect("trace contains only JSON primitives"),
            ),
        },
    ]
}
