//! Programme mit einem Klick verbinden (AGENT-043, Nutzerentscheidung 04.11.2026): eigenen Eintrag
//! formatbewahrend schreiben bzw. entfernen, Sicherung, Klassen-Freigabe `crawl` (AGENT-003).
//!
//! Die Oberfläche erhält nie den Inhalt der fremden Datei: geänderte Zeilen des eigenen Eintrags
//! vollständig, alle anderen geänderten Zeilen nur als `…` (AGENT-043, Nachtrag Review).
use super::{error, State};
use crate::{dto::*, App, CmdError};
use os_mcp::client_config::{self, ConfigError, EntryState};
use os_mcp::print_config::Client;
use os_mcp::ToolClass;
use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
};

/// Absoluter Pfad aus einer Umgebungsvariable; leer oder relativ zählt nicht.
fn env_dir(env: &dyn Fn(&str) -> Option<OsString>, name: &str) -> Option<PathBuf> {
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// Konfigurationsdatei je Programm (AGENT-043): `CODEX_HOME` bzw. `CLAUDE_CONFIG_DIR`, wenn absolut
/// gesetzt, sonst unter `home`; Claude Desktop im Konfigurationsverzeichnis `config_dir`; Cursor global
/// unter `home/.cursor/mcp.json` (macOS und Windows gleich, ohne Umgebungsvariable).
pub fn client_path(
    kind: McpClientKind,
    home: &Path,
    config_dir: &Path,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> (Client, PathBuf) {
    match kind {
        McpClientKind::Codex => (
            Client::Codex,
            env_dir(env, "CODEX_HOME")
                .unwrap_or_else(|| home.join(".codex"))
                .join("config.toml"),
        ),
        McpClientKind::ClaudeCode => (
            Client::ClaudeCode,
            env_dir(env, "CLAUDE_CONFIG_DIR")
                .unwrap_or_else(|| home.to_path_buf())
                .join(".claude.json"),
        ),
        McpClientKind::ClaudeDesktop => (
            Client::ClaudeDesktop,
            config_dir.join("Claude").join("claude_desktop_config.json"),
        ),
        McpClientKind::Cursor => (Client::Cursor, home.join(".cursor").join("mcp.json")),
    }
}

fn target(app: &App, kind: McpClientKind) -> Result<(Client, PathBuf), CmdError> {
    // Explicit app-home overrides isolate tests and portable installations from the real user directory;
    // environment variables then do not apply either.
    if app.config().documents_dir.is_some() {
        let home = &app.config().home;
        return Ok(client_path(kind, home, home, &|_| None));
    }
    let home = dirs::home_dir().ok_or_else(|| error("Home directory unavailable"))?;
    let config = dirs::config_dir().ok_or_else(|| error("Config directory unavailable"))?;
    Ok(client_path(kind, &home, &config, &|name| {
        std::env::var_os(name)
    }))
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

/// Vorhandene Datei mit Schreibschutz: unter Windows das Attribut „Schreibgeschützt“, unter Unix für niemanden
/// ein Schreibbit (`Permissions::readonly`). Das ist ein Signal für „bewusst geschützt“, nicht die Prüfung, ob
/// geschrieben werden darf (eine `0644`-Datei eines anderen Besitzers gilt als nicht geschützt). Eine fehlende Datei zählt nicht.
///
/// AGENT-043 (Nachtrag 05.11.2026): Solche Dateien schreibt OpenSpider nicht. Der Schreibschutz ist eine
/// bewusste Entscheidung des Nutzers; ihn aufzuheben und wieder zu setzen wäre nicht atomar, und unter
/// Windows scheitert das atomare Ersetzen einer schreibgeschützten Datei ohnehin (bisher erst nach dem
/// Anlegen der Sicherung).
fn is_read_only(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly())
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

fn read_only_error() -> CmdError {
    CmdError::new(ErrorCode::Permission, "mcp-client-problem-readOnly")
        .with_detail("Client configuration is read-only")
}

/// Vorschau ohne Geheimnisse (AGENT-043): nur geänderte Zeilen (ohne Kontext, mit Zeilennummern). Eine
/// Zeile erscheint vollständig, wenn sie ganz zum eigenen Eintrag gehört und wörtlich im neu erzeugten
/// Eintrag (`connected`, Ergebnis von „Verbinden“) vorkommt; Leerzeilen als Leerzeilen. Jede andere
/// geänderte Zeile – fremder Inhalt, aber auch Zusätze des Nutzers im vorhandenen eigenen Eintrag (z. B.
/// `env`) – erscheint nur als `…` bzw. `…,`, weil Wortregeln allein nicht jedes Geheimnis erkennen.
pub fn preview_diff(
    client: Client,
    before: &str,
    after: &str,
    connected: &str,
) -> Result<String, ConfigError> {
    use similar::ChangeTag;
    if before == after {
        return Ok(String::new());
    }
    let old_own = client_config::own_lines(client, before)?;
    let new_own = client_config::own_lines(client, after)?;
    let generated: std::collections::HashSet<&str> = client_config::own_lines(client, connected)?
        .map(|range| {
            connected
                .split_inclusive('\n')
                .skip(range.start)
                .take(range.len())
                .map(|l| l.trim_end_matches(['\n', '\r']))
                .collect()
        })
        .unwrap_or_default();
    let diff = similar::TextDiff::from_lines(before, after);
    let mut out = String::new();
    for group in diff.grouped_ops(0) {
        out.push_str(&format!(
            "{}\n",
            similar::udiff::UnifiedHunkHeader::new(&group)
        ));
        for op in &group {
            for change in diff.iter_changes(op) {
                let (sign, index, own) = match change.tag() {
                    ChangeTag::Delete => ('-', change.old_index(), &old_own),
                    ChangeTag::Insert => ('+', change.new_index(), &new_own),
                    ChangeTag::Equal => continue,
                };
                let line = change.value().trim_end_matches(['\n', '\r']);
                let is_own = index.is_some_and(|i| own.as_ref().is_some_and(|r| r.contains(&i)))
                    && generated.contains(line);
                out.push(sign);
                if is_own || line.trim().is_empty() {
                    out.push_str(line);
                } else if line.trim_end().ends_with(',') {
                    out.push_str("\u{2026},");
                } else {
                    out.push('\u{2026}');
                }
                out.push('\n');
            }
        }
    }
    Ok(out)
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
        extra_keys: Vec::new(),
    };
    if is_symlink(&path) {
        result.problem = Some(McpClientProblem::Symlink);
        return Ok(result);
    }
    if is_read_only(&path) {
        result.problem = Some(McpClientProblem::ReadOnly);
        return Ok(result);
    }
    let before = read(&path)?;
    let analysed = (|| {
        let state = client_config::entry_state(client, &before, &exe, &args)?;
        let connected = client_config::connect(client, &before, &exe, &args)?;
        let disconnected = client_config::disconnect(client, &before)?;
        Ok::<_, ConfigError>((
            state,
            preview_diff(client, &before, &connected, &connected)?,
            preview_diff(client, &before, &disconnected, &connected)?,
            client_config::own_extra_keys(client, &before, &exe, &args)?,
        ))
    })();
    match analysed {
        Ok((state, connect_diff, disconnect_diff, extra_keys)) => {
            result.state = match state {
                EntryState::Absent => McpClientState::NotConnected,
                EntryState::Current => McpClientState::Connected,
                EntryState::Outdated => McpClientState::Outdated,
            };
            result.connect_diff = connect_diff;
            result.disconnect_diff = disconnect_diff;
            result.extra_keys = extra_keys;
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
/// `Stale`), schreibgeschützte Datei ablehnen (vor der Sicherung), Sicherung der bisherigen Datei,
/// atomarer Austausch mit den Rechten der alten Datei.
pub fn write_guarded(path: &Path, before: &str, after: &str) -> Result<Option<String>, CmdError> {
    write_guarded_with(path, before, after, |staged, path| {
        staged.persist(path).map(drop).map_err(|e| e.error)
    })
}

/// Wie [`write_guarded`] mit austauschbarem letztem Schritt (Tests simulieren damit ein scheiterndes Ersetzen).
///
/// B3-FLAKES (Fable-Befund aus B3-CURSOR): Scheitert ein Schritt nach dem Anlegen der Sicherung – auch das
/// Schreiben der Sicherung selbst –, bleibt die Originaldatei unverändert. Die Sicherung enthält fremde
/// Geheimnisse und wird dann bestmöglich wieder entfernt; der Fehler hat einen eigenen Schlüssel
/// (`mcp-client-write-failed`, bzw. `mcp-client-write-failed-backup` mit dem Pfad, wenn die Sicherung bleibt).
fn write_guarded_with(
    path: &Path,
    before: &str,
    after: &str,
    persist: impl FnOnce(tempfile::NamedTempFile, &Path) -> std::io::Result<()>,
) -> Result<Option<String>, CmdError> {
    if is_symlink(path) {
        return Err(symlink_error());
    }
    if read(path)? != before {
        return Err(CmdError::new(ErrorCode::Stale, "mcp-client-changed")
            .with_detail("Client configuration changed while connecting"));
    }
    if is_read_only(path) {
        return Err(read_only_error());
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("Invalid configuration path"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let existing = std::fs::metadata(path).ok();
    let backup = match &existing {
        Some(meta) => {
            // Die Originale enthalten fremde Geheimnisse (oft 0600): Die Sicherung erhält ihre Rechte, bevor
            // der Inhalt geschrieben wird – unabhängig von der umask.
            let backup = path.with_extension(format!("{}.bak", uuid::Uuid::new_v4()));
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
                options.mode(meta.permissions().mode() & 0o777);
            }
            // `create_new`: Scheitert das Öffnen, gibt es keine Sicherung, die aufzuräumen wäre.
            let file = options.open(&backup).map_err(error)?;
            if let Err(e) = fill_backup(file, meta, before) {
                return Err(replace_failed(e, discard_backup(&backup)));
            }
            Some(backup)
        }
        None => None,
    };
    match replace(parent, path, after, existing.as_ref(), persist) {
        Ok(()) => Ok(backup.map(|b| b.to_string_lossy().into_owned())),
        Err(e) => Err(replace_failed(
            e,
            backup.as_deref().and_then(discard_backup),
        )),
    }
}

/// Schreibt die Sicherung (Rechte vor dem Inhalt) und schließt sie.
fn fill_backup(
    mut file: std::fs::File,
    meta: &std::fs::Metadata,
    before: &str,
) -> std::io::Result<()> {
    #[cfg(unix)]
    file.set_permissions(meta.permissions())?;
    file.write_all(before.as_bytes())?;
    file.sync_all()?;
    #[cfg(not(unix))]
    file.set_permissions(meta.permissions())?;
    Ok(())
}

/// Neue Datei daneben schreiben und atomar an die Stelle von `path` setzen.
fn replace(
    parent: &Path,
    path: &Path,
    after: &str,
    existing: Option<&std::fs::Metadata>,
    persist: impl FnOnce(tempfile::NamedTempFile, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(after.as_bytes())?;
    staged.as_file().sync_all()?;
    if let Some(meta) = existing {
        // Rechte der bisherigen Datei übernehmen (die temporäre Datei ist sonst nur für den Nutzer lesbar).
        std::fs::set_permissions(staged.path(), meta.permissions())?;
    }
    persist(staged, path)
}

/// Entfernt eine Sicherung, deren Ersetzen gescheitert ist; liefert den Pfad, wenn sie bleibt.
fn discard_backup(backup: &Path) -> Option<&Path> {
    match std::fs::remove_file(backup) {
        Ok(()) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(backup),
    }
}

/// Die Datei ist unverändert; `left` ist eine Sicherung, die sich nicht entfernen ließ (ihr Pfad wird genannt,
/// weil sie fremde Geheimnisse enthält). Details enthalten nie Inhalte der Datei.
fn replace_failed(e: std::io::Error, left: Option<&Path>) -> CmdError {
    let err = match left {
        None => CmdError::new(ErrorCode::Io, "mcp-client-write-failed"),
        Some(backup) => CmdError::new(ErrorCode::Io, "mcp-client-write-failed-backup")
            .with_arg("path", backup.to_string_lossy().into_owned()),
    };
    err.with_detail(format!("Client configuration could not be replaced: {e}"))
        .retryable()
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
    // Wie die Vorschau: eine geschützte Datei wird abgelehnt, auch wenn nichts zu ändern wäre.
    if is_read_only(&path) {
        return Err(read_only_error());
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
    if is_read_only(&path) {
        return Err(read_only_error());
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

    /// Dateien im Ordner außer `keep` (Sicherungen, temporäre Dateien).
    fn others(dir: &Path, keep: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != keep)
            .collect()
    }

    // spec: AGENT-043
    #[test]
    fn a_failed_replace_removes_the_backup_and_reports_its_own_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let before = "{\"mcpServers\": {\"x\": {\"env\": {\"TOKEN\": \"sk-TEST-B3-FLAKES\"}}}}\n";
        std::fs::write(&path, before).unwrap();
        let mut backup_seen = None;
        let err = write_guarded_with(&path, before, "{}\n", |staged, target| {
            // Zum Zeitpunkt des Ersetzens liegt die Sicherung schon neben der Datei.
            let backups: Vec<_> = others(target.parent().unwrap(), target)
                .into_iter()
                .filter(|p| p != staged.path())
                .collect();
            assert_eq!(backups.len(), 1, "{backups:?}");
            assert_eq!(std::fs::read_to_string(&backups[0]).unwrap(), before);
            backup_seen = Some(backups[0].clone());
            Err(std::io::Error::other("simulated persist failure"))
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
        assert_eq!(err.message_key, "mcp-client-write-failed");
        assert!(err.args.is_empty(), "{:?}", err.args);
        assert!(err.retryable);
        assert!(!err.detail.unwrap_or_default().contains("sk-TEST"));
        assert!(!backup_seen.unwrap().exists(), "Sicherung entfernt");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before,
            "Original unverändert"
        );
        assert_eq!(
            others(dir.path(), &path),
            Vec::<PathBuf>::new(),
            "keine Reste"
        );
    }

    // spec: AGENT-043
    #[test]
    fn a_backup_that_cannot_be_removed_is_named_in_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "a = 1\n").unwrap();
        let err = write_guarded_with(&path, "a = 1\n", "a = 2\n", |staged, target| {
            let backup = others(target.parent().unwrap(), target)
                .into_iter()
                .find(|p| p != staged.path())
                .unwrap();
            // Plattformneutral unlöschbar: an Stelle der Sicherung steht ein nicht leerer Ordner.
            std::fs::remove_file(&backup).unwrap();
            std::fs::create_dir(&backup).unwrap();
            std::fs::write(backup.join("inhalt"), "x").unwrap();
            Err(std::io::Error::other("simulated persist failure"))
        })
        .unwrap_err();
        assert_eq!(err.message_key, "mcp-client-write-failed-backup");
        let left = err.args["path"].as_str().unwrap().to_owned();
        assert!(Path::new(&left).exists());
        assert!(left.ends_with(".bak"), "{left}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 1\n");
    }

    // spec: AGENT-043
    #[cfg(unix)]
    #[test]
    fn permissions_of_the_previous_file_are_kept() {
        use std::os::unix::fs::PermissionsExt;
        let mode_of = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        // 0600: Originale mit fremden Geheimnisse (~/.claude.json, config.toml) – auch die Sicherung.
        for mode in [0o600, 0o644, 0o640] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "a = 1\n").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            let backup = write_guarded(&path, "a = 1\n", "a = 2\n").unwrap().unwrap();
            assert_eq!(mode_of(&path), mode, "new file");
            assert_eq!(mode_of(Path::new(&backup)), mode, "backup");
            assert_eq!(std::fs::read_to_string(&backup).unwrap(), "a = 1\n");
        }
    }

    // spec: AGENT-043
    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // Aufräumen: Windows löscht schreibgeschützte Dateien nicht.
    fn read_only_files_are_refused_before_any_backup() {
        // Plattformneutral über `Permissions::set_readonly`: Windows-Attribut bzw. Unix ohne Schreibbits.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        std::fs::write(&path, "{}\n").unwrap();
        assert!(!is_read_only(&path));
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions.clone()).unwrap();
        assert!(is_read_only(&path));
        let err = write_guarded(&path, "{}\n", "{\"mcpServers\": {}}\n").unwrap_err();
        assert_eq!(err.code, ErrorCode::Permission);
        assert_eq!(err.message_key, "mcp-client-problem-readOnly");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no backup, no staged file"
        );
        assert!(is_read_only(&path), "the protection stays");
        // Eine geänderte Datei bleibt `Stale` (Prüfreihenfolge wie bisher).
        let stale = write_guarded(&path, "[]\n", "{}\n").unwrap_err();
        assert_eq!(stale.code, ErrorCode::Stale);
        // Fehlende Datei ist nicht schreibgeschützt.
        assert!(!is_read_only(&dir.path().join("missing.json")));
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        assert!(write_guarded(&path, "{}\n", "{\"a\": 1}\n")
            .unwrap()
            .is_some());
    }

    // spec: AGENT-043
    #[test]
    fn foreign_lines_next_to_the_own_entry_appear_only_as_ellipsis() {
        let exe = "/x/openspider";
        let args = ["mcp".to_string()];
        // Werte ohne Geheimwort: Wortregeln maskieren sie nicht, die Vorschau zeigt sie trotzdem nicht.
        for (client, before) in [
            (
                Client::ClaudeCode,
                "{\n  \"theme\": \"dark\",\n  \"lastHandle\": \"ZZOPAQUE-41\"\n}\n",
            ),
            (
                Client::ClaudeDesktop,
                "{\n  \"mcpServers\": {\n    \"foreign\": {\"command\": \"helper\", \"args\": [\"ZZOPAQUE-42\"]}\n  }\n}\n",
            ),
            (Client::ClaudeDesktop, "{\"mcpServers\":{\"f\":{\"args\":[\"ZZOPAQUE-43\"]}}}"),
            (
                Client::Cursor,
                "{\n  \"mcpServers\": {\n    \"foreign\": {\"command\": \"helper\", \"args\": [\"ZZOPAQUE-45\"]}\n  }\n}\n",
            ),
            (Client::Codex, "handle = \"ZZOPAQUE-44\""),
        ] {
            let after = client_config::connect(client, before, exe, &args).unwrap();
            let diff = preview_diff(client, before, &after, &after).unwrap();
            assert!(!diff.contains("ZZOPAQUE"), "{client:?}: {diff}");
            let disconnect = client_config::disconnect(client, &after).unwrap();
            let back = preview_diff(client, &after, &disconnect, &after).unwrap();
            assert!(!back.contains("ZZOPAQUE"), "{client:?}: {back}");
            if before.contains('\n') && client != Client::Codex {
                assert!(back.contains(exe), "removed own lines stay visible: {back}");
                assert!(diff.contains("\n-\u{2026}\n+\u{2026},\n"), "{client:?}: {diff}");
                assert!(diff.contains(exe), "own lines stay visible: {diff}");
            }
        }
    }

    // spec: AGENT-043
    #[test]
    fn user_additions_in_the_existing_own_entry_are_not_shown() {
        let exe = "/x/openspider";
        let args = ["mcp".to_string()];
        let before = "[mcp_servers.openspider]\ncommand = \"/old\"\nargs = [\"mcp\"]\nenv = { TOKENLESS = \"ZZOPAQUE-51\" }\n";
        let connected = client_config::connect(Client::Codex, before, exe, &args).unwrap();
        let disconnected = client_config::disconnect(Client::Codex, before).unwrap();
        for after in [&connected, &disconnected] {
            let diff = preview_diff(Client::Codex, before, after, &connected).unwrap();
            assert!(
                !diff.contains("ZZOPAQUE") && !diff.contains("/old"),
                "{diff}"
            );
        }
        let diff = preview_diff(Client::Codex, before, &connected, &connected).unwrap();
        assert!(diff.contains("+command = \"/x/openspider\""), "{diff}");
        assert!(diff.contains("-\u{2026}\n"), "{diff}");
        // Unveränderte erzeugte Zeilen des alten Eintrags (args) bleiben sichtbar beim Trennen.
        let diff = preview_diff(Client::Codex, before, &disconnected, &connected).unwrap();
        assert!(
            diff.contains("-args = [\"mcp\"]") && diff.contains("-[mcp_servers.openspider]"),
            "{diff}"
        );
    }

    // spec: AGENT-043
    #[test]
    fn codex_home_and_claude_config_dir_are_honoured_when_absolute() {
        let home = Path::new("/home/anna");
        let config = Path::new("/home/anna/.config");
        let abs = if cfg!(windows) { "C:\\cfg" } else { "/cfg" };
        let env = |set: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                set.iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(*v))
            }
        };
        let none = env(&[]);
        assert_eq!(
            client_path(McpClientKind::Codex, home, config, &none).1,
            home.join(".codex").join("config.toml")
        );
        assert_eq!(
            client_path(McpClientKind::ClaudeCode, home, config, &none).1,
            home.join(".claude.json")
        );
        let set: &'static [(&str, &str)] = if cfg!(windows) {
            &[("CODEX_HOME", "C:\\cfg"), ("CLAUDE_CONFIG_DIR", "C:\\cfg")]
        } else {
            &[("CODEX_HOME", "/cfg"), ("CLAUDE_CONFIG_DIR", "/cfg")]
        };
        let with = env(set);
        assert_eq!(
            client_path(McpClientKind::Codex, home, config, &with).1,
            Path::new(abs).join("config.toml")
        );
        assert_eq!(
            client_path(McpClientKind::ClaudeCode, home, config, &with).1,
            Path::new(abs).join(".claude.json")
        );
        // Leer oder relativ: Standardpfad.
        let bad = env(&[("CODEX_HOME", ""), ("CLAUDE_CONFIG_DIR", "relative/dir")]);
        assert_eq!(
            client_path(McpClientKind::Codex, home, config, &bad).1,
            home.join(".codex").join("config.toml")
        );
        assert_eq!(
            client_path(McpClientKind::ClaudeCode, home, config, &bad).1,
            home.join(".claude.json")
        );
        assert_eq!(
            client_path(McpClientKind::ClaudeDesktop, home, config, &with).1,
            config.join("Claude").join("claude_desktop_config.json")
        );
        // Cursor: globale Datei im Benutzerordner, keine Umgebungsvariable (Nachtrag 05.11.2026).
        for env in [&none as &dyn Fn(&str) -> Option<OsString>, &with, &bad] {
            assert_eq!(
                client_path(McpClientKind::Cursor, home, config, env),
                (Client::Cursor, home.join(".cursor").join("mcp.json"))
            );
        }
    }

    // spec: AGENT-043
    #[test]
    fn the_masked_diff_contains_only_changed_lines() {
        let before = "[mcp_servers.a]\ncommand = \"a\"\nenv = { TOKEN = \"TESTSECRET-1\" }\n";
        let after = client_config::connect(Client::Codex, before, "/x/openspider", &["mcp".into()])
            .unwrap();
        let diff = preview_diff(Client::Codex, before, &after, &after).unwrap();
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
        assert_eq!(
            preview_diff(Client::Codex, before, before, before).unwrap(),
            ""
        );
    }
}
