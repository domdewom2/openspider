//! Programme mit einem Klick verbinden (AGENT-043, Nutzerentscheidung 04.11.2026): eigenen Eintrag
//! formatbewahrend schreiben bzw. entfernen, Sicherung, Klassen-Freigabe `crawl` (AGENT-003).
//!
//! Die Oberfläche erhält nie den Inhalt der fremden Datei, nur maskierte geänderte Zeilen.
use super::{error, State};
use crate::{dto::*, App, CmdError};
use os_mcp::client_config::{self, ConfigError, EntryState};
use os_mcp::print_config::Client;
use os_mcp::ToolClass;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

fn target(app: &App, kind: McpClientKind) -> Result<(Client, PathBuf), CmdError> {
    // Explicit app-home overrides isolate tests and portable installations from the real user directory.
    let home = if app.config().documents_dir.is_some() {
        app.config().home.clone()
    } else {
        dirs::home_dir().ok_or_else(|| error("Home directory unavailable"))?
    };
    Ok(match kind {
        McpClientKind::Codex => (Client::Codex, home.join(".codex").join("config.toml")),
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

/// Programm, das der Client starten soll: `openspider` neben der laufenden Binärdatei, Argument `mcp`.
fn server_command() -> Result<(String, Vec<String>), CmdError> {
    let exe = std::env::current_exe()
        .map_err(error)?
        .with_file_name(if cfg!(windows) {
            "openspider.exe"
        } else {
            "openspider"
        });
    Ok((exe.to_string_lossy().into_owned(), vec!["mcp".into()]))
}

fn read(path: &Path) -> Result<String, CmdError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(error(e)),
    }
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Verständliche, feste Meldung; Details enthalten nie Inhalte der Datei.
fn config_error(e: ConfigError) -> CmdError {
    match e {
        ConfigError::Syntax { line } => {
            CmdError::new(ErrorCode::InvalidInput, "mcp-client-problem-syntax")
                .with_detail(e.to_string())
                .with_arg("line", line)
        }
        ConfigError::Unsupported(_) => {
            CmdError::new(ErrorCode::InvalidInput, "mcp-client-problem-unsupported")
                .with_detail(e.to_string())
        }
    }
}

fn symlink_error() -> CmdError {
    CmdError::new(ErrorCode::InvalidInput, "mcp-client-problem-symlink")
        .with_detail("Client configuration must not be a symlink")
}

/// Diff der maskierten Fassungen, ohne Kontextzeilen (nur tatsächlich geänderte Zeilen).
pub fn masked_diff(client: Client, before: &str, after: &str) -> Result<String, ConfigError> {
    if before == after {
        return Ok(String::new());
    }
    let before = client_config::mask(client, before)?;
    let after = client_config::mask(client, after)?;
    Ok(similar::TextDiff::from_lines(&before, &after)
        .unified_diff()
        .context_radius(0)
        .missing_newline_hint(false)
        .to_string())
}

fn crawl_allowed(app: &App) -> Result<bool, CmdError> {
    Ok(super::load(app)?.classes.contains(&ToolClass::Crawl))
}

pub fn preview(app: &App, kind: McpClientKind) -> Result<McpClientPreview, CmdError> {
    let (client, path) = target(app, kind)?;
    let (exe, args) = server_command()?;
    let mut result = McpClientPreview {
        path: path.to_string_lossy().into_owned(),
        state: McpClientState::NotConnected,
        problem: None,
        line: None,
        connect_diff: String::new(),
        disconnect_diff: String::new(),
        entry: os_mcp::print_config::render(client, &exe, &args).map_err(error)?,
        crawl_allowed: crawl_allowed(app)?,
    };
    if is_symlink(&path) {
        result.problem = Some(McpClientProblem::Symlink);
        return Ok(result);
    }
    let before = read(&path)?;
    let analysed = (|| {
        let state = client_config::entry_state(client, &before, &exe, &args)?;
        let connected = client_config::connect(client, &before, &exe, &args)?;
        let disconnected = client_config::disconnect(client, &before)?;
        Ok::<_, ConfigError>((
            state,
            masked_diff(client, &before, &connected)?,
            masked_diff(client, &before, &disconnected)?,
        ))
    })();
    match analysed {
        Ok((state, connect_diff, disconnect_diff)) => {
            result.state = match state {
                EntryState::Absent => McpClientState::NotConnected,
                EntryState::Current => McpClientState::Connected,
                EntryState::Outdated => McpClientState::Outdated,
            };
            result.connect_diff = connect_diff;
            result.disconnect_diff = disconnect_diff;
        }
        Err(ConfigError::Syntax { line }) => {
            result.problem = Some(McpClientProblem::Syntax);
            result.line = Some(u32::try_from(line).unwrap_or(u32::MAX));
        }
        Err(ConfigError::Unsupported(_)) => result.problem = Some(McpClientProblem::Unsupported),
    }
    Ok(result)
}

/// Schreibt `after` an die Stelle von `before`: Datei unmittelbar vorher erneut lesen (geändert →
/// `Stale`), Sicherung der bisherigen Datei, atomarer Austausch mit den Rechten der alten Datei.
pub fn write_guarded(path: &Path, before: &str, after: &str) -> Result<Option<String>, CmdError> {
    if is_symlink(path) {
        return Err(symlink_error());
    }
    if read(path)? != before {
        return Err(CmdError::new(ErrorCode::Stale, "mcp-client-changed")
            .with_detail("Client configuration changed while connecting"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("Invalid configuration path"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let existing = std::fs::metadata(path).ok();
    let backup_path = if existing.is_some() {
        let backup = path.with_extension(format!("{}.bak", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
            .map_err(error)?;
        file.write_all(before.as_bytes()).map_err(error)?;
        file.sync_all().map_err(error)?;
        Some(backup.to_string_lossy().into_owned())
    } else {
        None
    };
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    staged.write_all(after.as_bytes()).map_err(error)?;
    staged.as_file().sync_all().map_err(error)?;
    if let Some(meta) = existing {
        // Rechte der bisherigen Datei übernehmen (die temporäre Datei ist sonst nur für den Nutzer lesbar).
        std::fs::set_permissions(staged.path(), meta.permissions()).map_err(error)?;
    }
    staged.persist(path).map_err(error)?;
    Ok(backup_path)
}

/// Klasse `crawl` nach dem Haken setzen; `read` bleibt, `write`/`cost` unverändert (AGENT-003).
fn set_crawl(app: &App, state: &State, allow: bool) -> Result<(), CmdError> {
    let mut settings = super::load(app)?;
    if settings.classes.contains(&ToolClass::Crawl) == allow {
        return Ok(());
    }
    let mut names: Vec<String> = settings
        .classes
        .iter()
        .filter(|c| **c != ToolClass::Crawl)
        .map(|c| format!("{c:?}").to_ascii_lowercase())
        .collect();
    if allow {
        names.push("crawl".into());
    }
    settings.classes = os_mcp::permissions::Permissions::resolve(
        &names,
        false,
        &os_mcp::settings::McpSettings::default(),
    )
    .map_err(error)?
    .classes()
    .to_vec();
    super::save(app, state, &settings)
}

pub fn connect(
    app: &App,
    state: &State,
    kind: McpClientKind,
    allow_crawl: bool,
) -> Result<McpClientWritten, CmdError> {
    let (client, path) = target(app, kind)?;
    let (exe, args) = server_command()?;
    if is_symlink(&path) {
        return Err(symlink_error());
    }
    let before = read(&path)?;
    let after = client_config::connect(client, &before, &exe, &args).map_err(config_error)?;
    let changed = after != before;
    let backup_path = if changed {
        write_guarded(&path, &before, &after)?
    } else {
        None
    };
    set_crawl(app, state, allow_crawl)?;
    Ok(McpClientWritten {
        backup_path,
        changed,
    })
}

pub fn disconnect(app: &App, kind: McpClientKind) -> Result<McpClientWritten, CmdError> {
    let (client, path) = target(app, kind)?;
    if is_symlink(&path) {
        return Err(symlink_error());
    }
    let before = read(&path)?;
    let after = client_config::disconnect(client, &before).map_err(config_error)?;
    if after == before {
        return Ok(McpClientWritten {
            backup_path: None,
            changed: false,
        });
    }
    Ok(McpClientWritten {
        backup_path: write_guarded(&path, &before, &after)?,
        changed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AGENT-043
    #[test]
    fn a_file_changed_after_reading_is_not_overwritten_and_gets_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "a = 1\n").unwrap();
        let err = write_guarded(&path, "a = 0\n", "a = 2\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Stale);
        assert_eq!(err.message_key, "mcp-client-changed");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 1\n");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let backup = write_guarded(&path, "a = 1\n", "a = 2\n").unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "a = 1\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 2\n");
        // Neue Datei: keine Sicherung.
        let fresh = dir.path().join("sub").join("new.json");
        assert_eq!(write_guarded(&fresh, "", "{}\n").unwrap(), None);
        assert_eq!(std::fs::read_to_string(fresh).unwrap(), "{}\n");
    }

    // spec: AGENT-043
    #[cfg(unix)]
    #[test]
    fn permissions_of_the_previous_file_are_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "a = 1\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_guarded(&path, "a = 1\n", "a = 2\n").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    // spec: AGENT-043
    #[test]
    fn the_masked_diff_contains_only_changed_lines() {
        let before = "[mcp_servers.a]\ncommand = \"a\"\nenv = { TOKEN = \"TESTSECRET-1\" }\n";
        let after = client_config::connect(Client::Codex, before, "/x/openspider", &["mcp".into()])
            .unwrap();
        let diff = masked_diff(Client::Codex, before, &after).unwrap();
        assert!(!diff.contains("TESTSECRET"));
        let changed: Vec<_> = diff
            .lines()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .collect();
        assert_eq!(
            changed,
            [
                "+",
                "+[mcp_servers.openspider]",
                "+command = \"/x/openspider\"",
                "+args = [\"mcp\"]"
            ]
        );
        assert!(
            !diff.contains("command = \"a\""),
            "no context lines: {diff}"
        );
        assert_eq!(masked_diff(Client::Codex, before, before).unwrap(), "");
    }
}
