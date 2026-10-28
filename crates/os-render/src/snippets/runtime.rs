use super::{RunOptions, SnippetResult};
use crate::RenderError;
use chromiumoxide::{
    cdp::js_protocol::runtime::{EvaluateParams, TerminateExecutionParams},
    Page,
};
use os_model::JsSnippetKind;
use serde_json::Value;
use std::{future::Future, time::Duration};

use super::clock::{Clock, TokioClock};

// data/error carry a closure-private identity; arbitrary returned objects remain data.
const WRAPPER: &str = include_str!("runtime.js");

/// Actions run first in table order, with a shared wait equal to the largest
/// action timeout. Extractions then run sequentially against that DOM.
pub async fn run(page: &Page, opts: &RunOptions) -> Result<Vec<SnippetResult>, RenderError> {
    execute(page, opts, &TokioClock::default()).await
}

// The scheduler depends on an injected clock and browser operations, so budget
// boundaries and the persisted trace can be tested without a live browser.
pub(super) trait SnippetPage: Sync {
    fn stop_loading(&self) -> impl Future<Output = Result<(), RenderError>> + Send;
    fn evaluate(&self, source: String) -> impl Future<Output = Result<Value, String>> + Send;
    fn terminate(&self) -> impl Future<Output = Result<(), RenderError>> + Send;
    fn target(&self) -> String;
}
impl SnippetPage for Page {
    async fn stop_loading(&self) -> Result<(), RenderError> {
        self.execute(chromiumoxide::cdp::browser_protocol::page::StopLoadingParams::default())
            .await
            .map(|_| ())
            .map_err(|e| RenderError::Protocol(e.to_string()))
    }
    async fn evaluate(&self, source: String) -> Result<Value, String> {
        self.evaluate_expression(
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
    }
    async fn terminate(&self) -> Result<(), RenderError> {
        self.execute(TerminateExecutionParams::default())
            .await
            .map(|_| ())
            .map_err(|e| RenderError::Protocol(e.to_string()))
    }
    fn target(&self) -> String {
        self.target_id().as_ref().to_owned()
    }
}

pub(super) async fn execute(
    page: &impl SnippetPage,
    opts: &RunOptions,
    clock: &impl Clock,
) -> Result<Vec<SnippetResult>, RenderError> {
    if opts.snippets.is_empty() {
        return Ok(Vec::new());
    }
    let epoch = clock.now();
    let mut results = Vec::new();
    let actions_started = epoch;
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
    let target = page.target();
    let _guard = ExtractionGuard(opts.extracting.clone(), target.clone());
    for action in [true, false] {
        if !action {
            let remaining = Duration::from_secs(wait)
                .saturating_sub(clock.now().saturating_sub(actions_started));
            clock.sleep(remaining).await;
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
            page.stop_loading().await?;
        }
        for (index, snippet) in &opts.snippets {
            if (snippet.kind == JsSnippetKind::Action) != action {
                continue;
            }
            let started_us = (clock.now().saturating_sub(epoch).as_micros() as u64).max(
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
                        .saturating_sub(clock.now().saturating_sub(actions_started))
                        .as_millis() as u32,
                )
            } else {
                timeout_ms
            };
            let source = WRAPPER
                .replace(
                    "/* FILE_ACCESS */",
                    if opts.output_dir.is_some() {
                        "true"
                    } else {
                        "false"
                    },
                )
                .replace("/* ACTION */", if action { "true" } else { "false" })
                .replace("/* USER_CODE */", &snippet.code);
            let evaluated = tokio::select! {
                biased;
                value = page.evaluate(source) => Some(value),
                () = clock.sleep(Duration::from_millis(timeout_ms.into())) => None,
            };
            let (values, error) = match evaluated {
                Some(Ok(value)) => {
                    let mut error = value["error"].as_str().map(str::to_owned);
                    if let Err(failure) = save_files(opts.output_dir.as_deref(), &value["files"]) {
                        error = Some(failure);
                    }
                    let values = value["values"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|v| v.as_str().map(str::to_owned))
                        .collect();
                    (values, error)
                }
                Some(Err(error)) => (Vec::new(), Some(error)),
                None => {
                    // Unlike a Promise.race in the page this stops synchronous loops.
                    tokio::select! {
                        biased;
                        result = page.terminate() => result?,
                        () = clock.sleep(Duration::from_secs(2)) =>
                            return Err(RenderError::Timeout("Snippet termination".into())),
                    }
                    (Vec::new(), Some("Timeout".into()))
                }
            };
            results.push(SnippetResult {
                def_index: *index,
                action,
                values: if action { Vec::new() } else { values },
                error,
                started_us,
                finished_us: (clock.now().saturating_sub(epoch).as_micros() as u64).max(started_us),
            });
        }
    }
    Ok(results)
}

/// Treat browser results as untrusted, even after the JavaScript path validation.
pub(super) fn save_files(root: Option<&std::path::Path>, files: &Value) -> Result<(), String> {
    use std::{fs, io::Write, path::Component};
    let Some(files) = files.as_array().filter(|files| !files.is_empty()) else {
        return Ok(());
    };
    let root = root.ok_or("File access is not enabled")?;
    // A configured CLI output folder may not have been created by an export yet.
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    for file in files {
        let name = file["path"].as_str().ok_or("Invalid output path")?;
        let name = name.replace('\\', "/");
        if name.is_empty()
            || name.starts_with('/')
            || name.contains(':')
            || name.contains('\0')
            || name.split('/').any(|part| {
                part == ".." || (part != "." && (part.ends_with('.') || part.ends_with(' ')))
            })
        {
            return Err("Invalid output path".into());
        }
        let mut destination = root.clone();
        let components: Vec<_> = std::path::Path::new(&name).components().collect();
        if components.is_empty() {
            return Err("Invalid output path".into());
        }
        for (i, component) in components.iter().enumerate() {
            match component {
                Component::CurDir => continue,
                Component::Normal(part) => destination.push(part),
                _ => return Err("Invalid output path".into()),
            }
            // Reject symlinks, including dangling links, before creating or opening anything.
            match fs::symlink_metadata(&destination) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err("Invalid output path: symlink".into())
                }
                Ok(_) => {
                    if !destination
                        .canonicalize()
                        .map_err(|e| e.to_string())?
                        .starts_with(&root)
                    {
                        return Err("Invalid output path: outside output directory".into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if i + 1 < components.len() {
                        fs::create_dir(&destination).map_err(|e| e.to_string())?;
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        let bytes: Vec<u8> =
            serde_json::from_value(file["bytes"].clone()).map_err(|e| e.to_string())?;
        let append = file["append"].as_bool().unwrap_or(false);
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(&destination)
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
