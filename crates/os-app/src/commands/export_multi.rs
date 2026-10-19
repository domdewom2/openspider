//! Multi-export and portable TOML presets (M2-X01, OUT-075).
use crate::{
    dto::{
        CmdError, ErrorCode, ExportBatchResult, ExportFormatDto, ExportMultiArgs, ExportPreset,
        ExportPresetArgs, FileResult,
    },
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{App, CallCtx},
};
use futures_util::future::BoxFuture;
use os_export::types::{ExportPreset as CorePreset, ExportSelection};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn register(registry: &mut Registry) {
    for (name, args, ret, long_running, handler) in [
        (
            "export.multi",
            "ExportMultiArgs",
            "ExportBatchResult",
            true,
            multi as crate::registry::Handler,
        ),
        ("export.presetList", "void", "ExportPreset[]", false, list),
        ("export.presetSave", "ExportPresetArgs", "void", false, save),
        (
            "export.presetDelete",
            "ExportPresetArgs",
            "void",
            false,
            delete,
        ),
    ] {
        registry.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running,
            },
            handler,
        );
    }
}
fn invalid(field: &str) -> CmdError {
    CmdError::invalid_input("err-invalid-args", field)
}
fn failure(error: impl std::fmt::Display) -> CmdError {
    CmdError::new(ErrorCode::Io, "err-io").with_detail(error.to_string())
}
fn preset_failure(error: os_export::ExportError) -> CmdError {
    match error {
        os_export::ExportError::Write(detail) => invalid("preset").with_detail(detail),
        other => failure(other),
    }
}
fn preset_path(home: &Path, name: &str) -> Result<PathBuf, CmdError> {
    if name.is_empty()
        || name.len() > 100
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-'))
    {
        return Err(invalid("name"));
    }
    Ok(home.join("export-presets").join(format!("{name}.ospreset")))
}
fn core(p: ExportPreset) -> CorePreset {
    CorePreset {
        name: p.name,
        items: p
            .items
            .into_iter()
            .map(|s| ExportSelection {
                kind: s.kind,
                name: s.name,
            })
            .collect(),
        single_workbook: p.single_workbook,
        skip_empty: p.skip_empty,
    }
}
fn dto(p: CorePreset) -> ExportPreset {
    ExportPreset {
        name: p.name,
        items: p
            .items
            .into_iter()
            .map(|s| crate::dto::ExportSel {
                kind: s.kind,
                name: s.name,
            })
            .collect(),
        single_workbook: p.single_workbook,
        skip_empty: p.skip_empty,
    }
}
fn list<'a>(app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    let directory = app.config().home.join("export-presets");
    Box::pin(async move {
        let presets = tokio::task::spawn_blocking(move || {
            let mut presets = Vec::new();
            let entries = match std::fs::read_dir(directory) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(presets),
                Err(e) => return Err(failure(e)),
            };
            for entry in entries {
                let path = entry.map_err(failure)?.path();
                if path.extension().is_some_and(|ext| ext == "ospreset") {
                    match CorePreset::load(&path) {
                        Ok(preset) => presets.push(dto(preset)),
                        Err(e) => tracing::warn!("{}: {e}", path.display()),
                    }
                }
            }
            presets.sort_by(|a, b| a.name.cmp(&b.name));
            Ok::<_, CmdError>(presets)
        })
        .await
        .map_err(failure)??;
        serde_json::to_value(presets).map_err(failure)
    })
}
fn save<'a>(app: &'a App, _ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ExportPresetArgs = serde_json::from_value(args).map_err(|_| invalid("preset"))?;
        let path = preset_path(&app.config().home, &args.name)?;
        let mut preset = args.preset.ok_or_else(|| invalid("preset"))?;
        preset.name = args.name;
        tokio::task::spawn_blocking(move || core(preset).save(&path))
            .await
            .map_err(failure)?
            .map_err(failure)?;
        Ok(Value::Null)
    })
}
fn delete<'a>(app: &'a App, _ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ExportPresetArgs = serde_json::from_value(args).map_err(|_| invalid("name"))?;
        let path = preset_path(&app.config().home, &args.name)?;
        tokio::task::spawn_blocking(move || match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        })
        .await
        .map_err(failure)?
        .map_err(failure)?;
        Ok(Value::Null)
    })
}
fn multi<'a>(app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ExportMultiArgs = serde_json::from_value(args).map_err(|_| invalid("items"))?;
        let session = app
            .sessions()
            .current(&ctx.window)
            .ok_or_else(|| CmdError::new(ErrorCode::NoCrawl, "err-no-crawl"))?;
        let format = match args.format {
            ExportFormatDto::Csv => os_export::Format::Csv,
            ExportFormatDto::Xlsx => os_export::Format::Xlsx,
            ExportFormatDto::Json => return Err(invalid("format")),
        };
        if args.dir.trim().is_empty() {
            return Err(invalid("dir"));
        }
        let preset = args
            .preset
            .as_ref()
            .map(|name| {
                if Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext == "ospreset")
                {
                    Ok(PathBuf::from(name))
                } else {
                    preset_path(&app.config().home, name)
                }
            })
            .transpose()?;
        let app = app.clone();
        let handle = app
            .clone()
            .ops()
            .spawn("op-export", true, move |cancel, progress| {
                Box::pin(async move {
                    let outcome = app
                        .read_pool()
                        .run_with(session.key, cancel.clone(), move |file, token| {
                            let preset = match preset {
                                Some(path) => match CorePreset::load(&path) {
                                    Ok(p) => Some(p),
                                    Err(e) => return Ok(Err(preset_failure(e))),
                                },
                                None => None,
                            };
                            let items: Vec<_> =
                                preset.as_ref().map(|p| p.items.clone()).unwrap_or_else(|| {
                                    args.items
                                        .into_iter()
                                        .map(|s| ExportSelection {
                                            kind: s.kind,
                                            name: s.name,
                                        })
                                        .collect()
                                });
                            let exported = os_export::pipeline::snapshot(file, || {
                                let total = os_export::pipeline::batch_row_count(
                                    file,
                                    &session.config,
                                    &items,
                                )?;
                                let mut control = os_export::types::ExportCtx {
                                    cancel: token,
                                    progress: &|done| progress.report(done, total),
                                    skip_empty: preset
                                        .as_ref()
                                        .map_or(args.skip_empty, |p| p.skip_empty),
                                    workbook: None,
                                    skipped: Vec::new(),
                                };
                                let metadata = file.meta()?;
                                let name = if metadata.name.trim().is_empty() {
                                    "crawl"
                                } else {
                                    &metadata.name
                                };
                                let options = os_export::pipeline::BatchOptions {
                                    format,
                                    directory: Path::new(&args.dir),
                                    crawl_name: name,
                                    single_workbook: preset
                                        .as_ref()
                                        .map_or(args.single_workbook, |p| p.single_workbook),
                                };
                                os_export::pipeline::export_batch(
                                    file,
                                    &session.config,
                                    &items,
                                    &options,
                                    &mut control,
                                )
                            });
                            Ok(exported.map_err(|e| {
                                if token.is_cancelled() {
                                    CmdError::new(ErrorCode::Cancelled, "err-cancelled")
                                } else {
                                    failure(e)
                                }
                            }))
                        })
                        .await??;
                    let result = ExportBatchResult {
                        files: outcome
                            .files
                            .into_iter()
                            .map(|f| FileResult {
                                path: f.path.to_string_lossy().into(),
                                bytes: f.bytes,
                            })
                            .collect(),
                        warnings: outcome.warnings,
                        skipped: outcome.skipped,
                    };
                    serde_json::to_value(result).map_err(failure)
                })
            });
        serde_json::to_value(handle).map_err(failure)
    })
}
