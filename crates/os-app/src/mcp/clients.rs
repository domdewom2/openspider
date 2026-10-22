//! Preview-bound local client configuration writes with backups and conflict detection.
use super::{error, State};
use crate::{dto::*, App, CmdError};
use os_mcp::print_config::Client;
use std::{io::Write, path::PathBuf};
fn target(app: &App, kind: McpClientKind) -> Result<(Client, PathBuf), CmdError> {
    // Explicit app-home overrides isolate tests and portable installations from the real user directory.
    let home = if app.config().documents_dir.is_some() {
        app.config().home.clone()
    } else {
        dirs::home_dir().ok_or_else(|| error("Home directory unavailable"))?
    };
    Ok(match kind {
        McpClientKind::Codex => (Client::Codex, home.join(".codex/config.toml")),
        McpClientKind::ClaudeCode => (Client::ClaudeCode, home.join(".claude.json")),
        McpClientKind::ClaudeDesktop => {
            let dir = if app.config().documents_dir.is_some() {
                home.join("Claude")
            } else {
                dirs::config_dir()
                    .ok_or_else(|| error("Config directory unavailable"))?
                    .join("Claude")
            };
            (
                Client::ClaudeDesktop,
                dir.join("claude_desktop_config.json"),
            )
        }
    })
}
fn read(path: &std::path::Path) -> Result<String, CmdError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(error(e)),
    }
}
pub fn preview(
    app: &App,
    state: &mut State,
    kind: McpClientKind,
) -> Result<McpClientPreview, CmdError> {
    let (client, path) = target(app, kind)?;
    let before = read(&path)?;
    let exe = std::env::current_exe()
        .map_err(error)?
        .with_file_name(if cfg!(windows) {
            "openspider.exe"
        } else {
            "openspider"
        });
    let after =
        os_mcp::client_config::merged(client, &before, &exe.to_string_lossy(), &["mcp".into()])
            .map_err(error)?;
    let diff = similar::TextDiff::from_lines(&before, &after)
        .unified_diff()
        .header("before", "after")
        .to_string();
    let result = McpClientPreview {
        path: path.to_string_lossy().into_owned(),
        before,
        after,
        diff,
    };
    state.previews.insert(client.name().into(), result.clone());
    Ok(result)
}
pub fn write(
    app: &App,
    state: &mut State,
    kind: McpClientKind,
) -> Result<McpClientWritten, CmdError> {
    let (client, path) = target(app, kind)?;
    let preview = state
        .previews
        .remove(client.name())
        .ok_or_else(|| error("Preview and confirm the client configuration first"))?;
    if read(&path)? != preview.before {
        return Err(error("Client configuration changed; preview again"));
    }
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(error("Client configuration must not be a symlink"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("Invalid configuration path"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let backup_path = if path.exists() {
        let backup = path.with_extension(format!("{}.bak", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
            .map_err(error)?;
        file.write_all(preview.before.as_bytes()).map_err(error)?;
        file.sync_all().map_err(error)?;
        Some(backup.to_string_lossy().into_owned())
    } else {
        None
    };
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    staged.write_all(preview.after.as_bytes()).map_err(error)?;
    staged.as_file().sync_all().map_err(error)?;
    staged.persist(path).map_err(error)?;
    Ok(McpClientWritten { backup_path })
}
