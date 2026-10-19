//! MCP exports reuse the core writers. Files are generated in a private staging directory and
//! published with create_new, avoiding overwrite races and final-file symlink traversal.
use super::common::{internal, open_crawl, opt_str, reject_unknown, req_str};
use crate::{Tool, ToolClass, ToolContext, ToolError, ToolOutput};
use os_export::{catalog, ExportKind, ExportRef, Format};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub struct Export;
fn invalid(msg: impl Into<String>) -> ToolError {
    ToolError::InvalidArguments(msg.into())
}

fn export_root(ctx: &ToolContext) -> Result<PathBuf, ToolError> {
    let settings = os_model::settings::AppSettings::load(&ctx.settings_dir).map_err(internal)?;
    let root = settings
        .export
        .directory
        .map(PathBuf::from)
        .or_else(|| dirs::document_dir().map(|p| p.join("OpenSpider").join("Exports")))
        .ok_or_else(|| invalid("Cannot locate Documents; configure export.directory"))?;
    std::fs::create_dir_all(&root).map_err(internal)?;
    root.canonicalize().map_err(internal)
}

fn resolve_parent(path: &Path) -> Result<PathBuf, ToolError> {
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            _ => clean.push(part.as_os_str()),
        }
    }
    let mut existing = clean.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        if std::fs::symlink_metadata(existing).is_ok() {
            return Err(invalid("Export parent is a broken symlink"));
        }
        missing.push(
            existing
                .file_name()
                .ok_or_else(|| invalid("Invalid parent"))?
                .to_os_string(),
        );
        existing = existing.parent().ok_or_else(|| invalid("Invalid parent"))?;
    }
    let mut real = existing.canonicalize().map_err(internal)?;
    for part in missing.iter().rev() {
        real.push(part);
    }
    Ok(real)
}

fn destination(
    ctx: &ToolContext,
    root: &Path,
    path: Option<&str>,
    default: &str,
) -> Result<PathBuf, ToolError> {
    let target = path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default));
    let target = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    // Resolve lexical parent traversal before realpath; missing parents are checked before creation.
    let mut clean = PathBuf::new();
    for part in target.components() {
        match part {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            _ => clean.push(part.as_os_str()),
        }
    }
    let parent = clean
        .parent()
        .ok_or_else(|| invalid("Export path needs a parent directory"))?;
    let name = clean
        .file_name()
        .ok_or_else(|| invalid("Export path needs a filename"))?;
    let real = resolve_parent(parent)?;
    let mut allowed = vec![root.to_path_buf()];
    for path in &ctx.settings.export_dirs {
        let path = if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        };
        allowed.push(resolve_parent(&path)?);
    }
    if !allowed.iter().any(|dir| real.starts_with(dir)) {
        return Err(invalid(
            "Export path is outside allowed directories (export.directory / mcp.toml export_dirs)",
        ));
    }
    std::fs::create_dir_all(&real).map_err(internal)?;
    let real = real.canonicalize().map_err(internal)?;
    if !allowed.iter().any(|dir| real.starts_with(dir)) {
        return Err(invalid(
            "Export parent resolves outside allowed directories",
        ));
    }
    let target = real.join(name);
    if std::fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(invalid("Export target must not be a symlink"));
    }
    Ok(target)
}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn stage(root: &Path) -> Result<Stage, ToolError> {
    for _ in 0..10 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(internal)?;
        let name = bytes.iter().map(|n| format!("{n:02x}")).collect::<String>();
        let path = root.join(format!(".mcp-export-{name}"));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(Stage(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(internal(e)),
        }
    }
    Err(invalid("Cannot create export staging directory"))
}

fn publish(source: &Path, target: &Path) -> Result<PathBuf, ToolError> {
    for suffix in 1..10_000 {
        let candidate = if suffix == 1 {
            target.to_path_buf()
        } else {
            let stem = target.file_stem().unwrap_or_default().to_string_lossy();
            let ext = target
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            target.with_file_name(format!("{stem} ({suffix}){ext}"))
        };
        if std::fs::symlink_metadata(&candidate).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid("Export target must not be a symlink"));
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let result = (|| {
                    let mut input = std::fs::File::open(source)?;
                    std::io::copy(&mut input, &mut file)?;
                    file.flush()?;
                    Ok::<_, std::io::Error>(())
                })();
                if let Err(e) = result {
                    let _ = std::fs::remove_file(&candidate);
                    return Err(internal(e));
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(internal(e)),
        }
    }
    Err(invalid("Too many export name collisions"))
}

impl Tool for Export {
    fn name(&self) -> &'static str {
        "export"
    }
    fn class(&self) -> ToolClass {
        ToolClass::Write
    }
    fn description(&self) -> &'static str {
        "Export a tab, bulk export or report using canonical CLI names and the core CSV/XLSX writers. kind: tab|bulk|report. Optional path is resolved relative to export.directory and restricted to that directory or configured MCP export_dirs; symlinks outside are rejected. Existing files are preserved with a (2) suffix. Without path use the OUT-020 filename. Returns absolute paths."
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","properties":{"crawl_id":{"type":"string"},"kind":{"type":"string","enum":["tab","bulk","report"]},"name":{"type":"string"},"format":{"type":"string","enum":["csv","xlsx"]},"path":{"type":"string"}},"required":["crawl_id","kind","name","format"],"additionalProperties":false})
    }
    fn call(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        reject_unknown(&args, &["crawl_id", "kind", "name", "format", "path"])?;
        let crawl = open_crawl(ctx, &args)?;
        let kind = match req_str(&args, "kind")? {
            "tab" => ExportKind::Tab,
            "bulk" => ExportKind::Bulk,
            "report" => ExportKind::Report,
            _ => return Err(invalid("kind: tab, bulk or report")),
        };
        let format = match req_str(&args, "format")? {
            "csv" => Format::Csv,
            "xlsx" => Format::Xlsx,
            _ => return Err(invalid("format: csv or xlsx")),
        };
        let name = req_str(&args, "name")?;
        let resolved =
            catalog::resolve_with(kind, name, &crawl.cfg).map_err(|e| invalid(e.to_string()))?;
        let root = export_root(ctx)?;
        let requested = opt_str(&args, "path")?;
        // Validate an explicit destination before generating anything.
        if let Some(path) = requested {
            destination(ctx, &root, Some(path), "export.csv")?;
        }
        let staging = stage(&root)?;
        let tab_canonical = if let ExportRef::Tab {
            ref tab,
            ref filter,
        } = resolved
        {
            Some(
                os_export::tab_export::canonical_name(
                    &os_analyze::tabs::configured(&crawl.cfg),
                    &crawl.cfg,
                    tab,
                    filter,
                )
                .map_err(internal)?,
            )
        } else {
            None
        };
        let files = match resolved {
            ExportRef::Tab { tab, filter } => {
                let q = os_analyze::views::ViewQuery::new(&tab, &filter);
                vec![
                    os_export::export_tab(&crawl.file, &crawl.cfg, &q, &staging.0, format)
                        .map_err(internal)?,
                ]
            }
            ExportRef::Bulk(name) => {
                os_export::export_bulk(&crawl.file, &crawl.cfg, &name, &staging.0, format)
                    .map_err(internal)?
            }
            ExportRef::Report(name) => {
                os_export::save_report(&crawl.file, &crawl.cfg, &name, &staging.0, format)
                    .map_err(internal)?
                    .files
            }
        };
        if requested.is_some() && files.len() != 1 {
            return Err(invalid(
                "path requires an export producing exactly one file",
            ));
        }
        let mut paths = Vec::new();
        for file in files {
            let default = if let Some(canonical) = &tab_canonical {
                os_export::tab_export::suggest_file_name(
                    canonical,
                    None,
                    crawl.meta.started_at,
                    None,
                    format,
                )
            } else {
                file.path
                    .strip_prefix(&staging.0)
                    .map_err(internal)?
                    .to_string_lossy()
                    .into_owned()
            };
            let target = destination(ctx, &root, requested, &default)?;
            paths.push(publish(&file.path, &target)?);
        }
        Ok(ToolOutput {
            summary: json!({"files":paths}),
            untrusted: None,
            total: None,
            next_cursor: None,
        })
    }
}
