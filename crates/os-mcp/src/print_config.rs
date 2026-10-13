//! `--print-config claude-desktop|claude-code|codex|cursor` mit absolutem Binärpfad (AGENT-001).
//!
//! Die Ausgabe ist ein fertiger Eintrag für die Konfiguration des jeweiligen Clients: JSON für Claude
//! Desktop, Claude Code und Cursor, TOML für Codex. Der Server heißt `openspider`; Befehl ist der absolute
//! Pfad der laufenden Binärdatei, Argumente sind der Unterbefehl `mcp` (beim schlanken Starter
//! `openspider-mcp` entfällt er) und die beim Aufruf angegebenen Schalter. JSON-Schlüssel stehen
//! alphabetisch, damit die Ausgabe stabil und als Snapshot prüfbar ist.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::McpError;

/// Name des Servers in den Client-Konfigurationen.
pub const SERVER_NAME: &str = "openspider";

/// MCP-Clients, für die eine Konfiguration ausgegeben werden kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    /// Claude Desktop (`claude_desktop_config.json`).
    ClaudeDesktop,
    /// Claude Code (`.mcp.json` bzw. `claude mcp add-json`).
    ClaudeCode,
    /// OpenAI Codex (`~/.codex/config.toml`).
    Codex,
    /// Cursor (`.cursor/mcp.json`).
    Cursor,
}

impl Client {
    /// Alle Clients.
    pub const ALL: [Client; 4] = [
        Client::ClaudeDesktop,
        Client::ClaudeCode,
        Client::Codex,
        Client::Cursor,
    ];

    /// Kennung wie in `--print-config`.
    pub fn name(self) -> &'static str {
        match self {
            Client::ClaudeDesktop => "claude-desktop",
            Client::ClaudeCode => "claude-code",
            Client::Codex => "codex",
            Client::Cursor => "cursor",
        }
    }

    /// Liest die Kennung aus `--print-config`.
    pub fn parse(name: &str) -> Option<Client> {
        Client::ALL.into_iter().find(|c| c.name() == name.trim())
    }

    /// Dateiendung der Ausgabe (`json` oder `toml`).
    pub fn extension(self) -> &'static str {
        match self {
            Client::Codex => "toml",
            _ => "json",
        }
    }

    /// Hinweis, wohin die Ausgabe gehört (geht auf stderr, nicht in die Konfiguration).
    pub fn placement_hint(self) -> &'static str {
        match self {
            Client::ClaudeDesktop => {
                "In claude_desktop_config.json einfügen (Claude Desktop: Einstellungen > Entwickler > \
                 Konfiguration bearbeiten); \"mcpServers\" mit vorhandenen Einträgen zusammenführen."
            }
            Client::ClaudeCode => {
                "In die Datei .mcp.json des Projekts einfügen oder mit `claude mcp add-json openspider \
                 '<Eintrag>'` anlegen."
            }
            Client::Codex => "An ~/.codex/config.toml anhängen.",
            Client::Cursor => {
                "In .cursor/mcp.json (Projekt) oder ~/.cursor/mcp.json (global) einfügen; \"mcpServers\" mit \
                 vorhandenen Einträgen zusammenführen."
            }
        }
    }
}

/// Erzeugt die Konfiguration für `client`.
///
/// `exe` ist der absolute Pfad der Binärdatei (wird unverändert übernommen), `args` sind die
/// Befehlszeilenargumente des Servers.
///
/// # Errors
/// [`McpError::Settings`], wenn sich die Konfiguration nicht serialisieren lässt (praktisch nicht möglich).
pub fn render(client: Client, exe: &str, args: &[String]) -> Result<String, McpError> {
    match client {
        Client::Codex => render_toml(exe, args),
        Client::ClaudeDesktop | Client::Cursor => {
            render_json(json!({ "command": exe, "args": args }))
        }
        Client::ClaudeCode => render_json(json!({ "type": "stdio", "command": exe, "args": args })),
    }
}

fn render_json(server: Value) -> Result<String, McpError> {
    let doc = json!({ "mcpServers": { SERVER_NAME: server } });
    let mut text =
        serde_json::to_string_pretty(&doc).map_err(|e| McpError::Settings(e.to_string()))?;
    text.push('\n');
    Ok(text)
}

fn render_toml(exe: &str, args: &[String]) -> Result<String, McpError> {
    let mut server = toml::Table::new();
    server.insert("command".into(), toml::Value::String(exe.to_string()));
    server.insert(
        "args".into(),
        toml::Value::Array(
            args.iter()
                .map(|a| toml::Value::String(a.clone()))
                .collect(),
        ),
    );
    let mut servers = toml::Table::new();
    servers.insert(SERVER_NAME.into(), toml::Value::Table(server));
    let mut root = toml::Table::new();
    root.insert("mcp_servers".into(), toml::Value::Table(servers));
    toml::to_string(&root).map_err(|e| McpError::Settings(e.to_string()))
}

/// Absoluter Pfad der laufenden Binärdatei.
///
/// # Errors
/// [`McpError::Io`], wenn der Pfad nicht bestimmbar ist.
pub fn current_exe_absolute() -> Result<PathBuf, McpError> {
    let exe = std::env::current_exe().map_err(|e| McpError::io("current_exe", e))?;
    // `canonicalize` löst Symlinks auf (z. B. Homebrew-Verknüpfungen) und liefert unter Windows
    // `\\?\`-Pfade, die Clients schlecht vertragen; deshalb nur absolut machen.
    let abs = std::path::absolute(&exe).unwrap_or(exe);
    Ok(abs)
}

/// Pfad als Zeichenkette für die Ausgabe.
pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: &str = "/Applications/OpenSpider.app/Contents/MacOS/openspider";
    const WIN: &str = "C:\\Program Files\\OpenSpider\\openspider.exe";

    fn mcp() -> Vec<String> {
        vec!["mcp".to_string()]
    }

    // spec: AGENT-001
    #[test]
    fn client_names_roundtrip() {
        for c in Client::ALL {
            assert_eq!(Client::parse(c.name()), Some(c));
        }
        assert_eq!(Client::parse("vscode"), None);
    }

    // spec: AGENT-001
    #[test]
    fn json_clients_produce_valid_json_with_the_absolute_path() {
        for client in [Client::ClaudeDesktop, Client::ClaudeCode, Client::Cursor] {
            for exe in [MAC, WIN] {
                let text = render(client, exe, &mcp()).unwrap();
                let v: Value = serde_json::from_str(&text).unwrap();
                let server = &v["mcpServers"][SERVER_NAME];
                assert_eq!(server["command"], exe, "{client:?}");
                assert_eq!(server["args"], json!(["mcp"]));
                assert!(text.ends_with('\n'));
            }
        }
        let code: Value =
            serde_json::from_str(&render(Client::ClaudeCode, MAC, &mcp()).unwrap()).unwrap();
        assert_eq!(code["mcpServers"][SERVER_NAME]["type"], "stdio");
    }

    // spec: AGENT-001
    #[test]
    fn codex_produces_valid_toml_with_the_absolute_path() {
        for exe in [MAC, WIN] {
            let text = render(Client::Codex, exe, &mcp()).unwrap();
            let v: toml::Table = toml::from_str(&text).unwrap();
            let server = &v["mcp_servers"][SERVER_NAME];
            assert_eq!(server["command"].as_str(), Some(exe));
            assert_eq!(server["args"].as_array().unwrap().len(), 1);
            assert!(text.contains("[mcp_servers.openspider]"), "{text}");
        }
    }

    // spec: AGENT-001
    #[test]
    fn extra_arguments_are_carried_into_the_config() {
        let args: Vec<String> = [
            "mcp",
            "--allow",
            "crawl",
            "--crawl-dir",
            "D:\\Crawls Ordner",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let v: Value = serde_json::from_str(&render(Client::Cursor, WIN, &args).unwrap()).unwrap();
        assert_eq!(v["mcpServers"][SERVER_NAME]["args"], json!(args));
        let t: toml::Table = toml::from_str(&render(Client::Codex, WIN, &args).unwrap()).unwrap();
        assert_eq!(
            t["mcp_servers"][SERVER_NAME]["args"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
    }

    // spec: AGENT-001
    #[test]
    fn current_exe_is_absolute() {
        let exe = current_exe_absolute().unwrap();
        assert!(exe.is_absolute(), "{}", exe.display());
    }
}
