use super::{RunOptions, SnippetResult};
use crate::RenderError;
use chromiumoxide::{
    cdp::js_protocol::runtime::{EvaluateParams, TerminateExecutionParams},
    Page,
};
use os_model::JsSnippetKind;
use serde_json::Value;
use std::time::{Duration, Instant};

// data/error carry a closure-private identity; arbitrary returned objects remain data.
const WRAPPER: &str = include_str!("runtime.js");

/// Actions run first in table order, with a shared wait equal to the largest
/// action timeout. Extractions then run sequentially against that DOM.
pub async fn run(page: &Page, opts: &RunOptions) -> Result<Vec<SnippetResult>, RenderError> {
    if opts.snippets.is_empty() {
        return Ok(Vec::new());
    }
    let epoch = Instant::now();
    let mut results = Vec::new();
    let actions_started = Instant::now();
    let wait = opts
        .snippets
        .iter()
        .filter(|(_, s)| s.kind == JsSnippetKind::Action)
        .map(|(_, s)| u64::from(s.timeout_s))
        .max()
        .unwrap_or(0);
    struct ExtractionGuard(super::ExtractionPages, String);
    impl Drop for ExtractionGuard {
        fn drop(&mut self) {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&self.1);
        }
    }
    let target = page.target_id().as_ref().to_owned();
    let _guard = ExtractionGuard(opts.extracting.clone(), target.clone());
    for action in [true, false] {
        if !action {
            let remaining = Duration::from_secs(wait).saturating_sub(actions_started.elapsed());
            tokio::time::sleep(remaining).await;
            if !opts
                .snippets
                .iter()
                .any(|(_, s)| s.kind == JsSnippetKind::Extraction)
            {
                break;
            }
            opts.extracting
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(target.clone());
            // Cancel unfinished passive loads before evaluating the extraction.
            page.execute(chromiumoxide::cdp::browser_protocol::page::StopLoadingParams::default())
                .await
                .map_err(|e| RenderError::Protocol(e.to_string()))?;
        }
        for (index, snippet) in &opts.snippets {
            if (snippet.kind == JsSnippetKind::Action) != action {
                continue;
            }
            let started_us = (epoch.elapsed().as_micros() as u64).max(
                results
                    .last()
                    .map_or(0, |r: &SnippetResult| r.finished_us.saturating_add(1)),
            );
            let timeout_ms = if snippet.timeout_s == 0 {
                opts.rendering_timeout_ms.max(1)
            } else {
                snippet.timeout_s.saturating_mul(1000)
            };
            let timeout_ms = if action {
                timeout_ms.min(
                    Duration::from_secs(wait)
                        .saturating_sub(actions_started.elapsed())
                        .as_millis() as u32,
                )
            } else {
                timeout_ms
            };
            let source = WRAPPER
                .replace("/* ACTION */", if action { "true" } else { "false" })
                .replace("/* USER_CODE */", &snippet.code);
            let evaluate = async {
                page.evaluate_expression(
                    EvaluateParams::builder()
                        .expression(source)
                        .await_promise(true)
                        .return_by_value(true)
                        .build()
                        .map_err(|e| e.to_string())?,
                )
                .await
                .map_err(|e| e.to_string())?
                .into_value::<Value>()
                .map_err(|e| e.to_string())
            };
            let (values, error) = match tokio::time::timeout(
                Duration::from_millis(timeout_ms.into()),
                evaluate,
            )
            .await
            {
                Ok(Ok(value)) => {
                    let error = value["error"].as_str().map(str::to_owned);
                    let values = value["values"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|v| v.as_str().map(str::to_owned))
                        .collect();
                    (values, error)
                }
                Ok(Err(error)) => (Vec::new(), Some(error)),
                Err(_) => {
                    // Unlike a Promise.race in the page this stops synchronous loops.
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        page.execute(TerminateExecutionParams::default()),
                    )
                    .await
                    .map_err(|_| RenderError::Timeout("Snippet termination".into()))?
                    .map_err(|e| RenderError::Protocol(e.to_string()))?;
                    (Vec::new(), Some("Timeout".into()))
                }
            };
            results.push(SnippetResult {
                def_index: *index,
                action,
                values: if action { Vec::new() } else { values },
                error,
                started_us,
                finished_us: (epoch.elapsed().as_micros() as u64).max(started_us),
            });
        }
    }
    Ok(results)
}
