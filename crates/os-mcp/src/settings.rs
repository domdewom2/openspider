//! Einstellungen des MCP-Servers (`mcp.toml`, AGENT-001, AGENT-003): freigegebene Werkzeugklassen, einzelne
//! Werkzeuge, erlaubte Domains, Exportordner und der lokale HTTP-Zugang (M2).
//!
//! Die Datei liegt unter [`os_store::paths::AppPaths::settings`] (`mcp.toml`; mit `OPENSPIDER_HOME` ist
//! `settings` der Heimatordner selbst). Der Server liest die Datei nur ([`load`]); [`save`] benutzt ab M2
//! die App (AGENT-043). Es gibt genau diesen einen Parser. Die Einstellungen sind per MCP **nicht**
//! änderbar (AGENT-003).
//!
//! # Schema
//!
//! Alle Schlüssel sind optional; fehlende Schlüssel und eine fehlende Datei ergeben den
//! Auslieferungszustand (nur `read`).
//!
//! ```toml
//! # Freigegebene Werkzeugklassen: "read", "crawl", "write", "cost" ("read" gilt immer).
//! classes = ["read"]
//! # Domains, die Crawl-Werkzeuge ansteuern dürfen (leer = keine Einschränkung). Eintrag
//! # "example.com" erlaubt auch Subdomains.
//! allowed_domains = []
//! # Private und lokale Ziele (localhost, 10.x.x.x, 192.168.x.x, ::1 …) für Crawl-Werkzeuge erlauben.
//! allow_private_targets = false
//! # Ordner, in die Werkzeuge der Klasse "write" exportieren dürfen.
//! export_dirs = []
//!
//! # Einzelne Werkzeuge abschalten (true ändert nichts an der Klassenfreigabe).
//! [tools]
//! # query_urls = false
//!
//! # Lokaler HTTP-Zugang (ab M2).
//! [http]
//! # port = 7421
//! autostart = false
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::McpError;
use crate::tool::ToolClass;

/// Einstellungen des MCP-Servers. Fehlende Felder einer Datei erhalten die Standardwerte.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct McpSettings {
    /// Freigegebene Werkzeugklassen (Standard: nur `read`).
    pub classes: Vec<ToolClass>,
    /// Einzelne Werkzeuge ein- oder ausschalten (`[tools] <name> = false`).
    pub tools: BTreeMap<String, bool>,
    /// Domains, die Crawl-Werkzeuge ansteuern dürfen.
    pub allowed_domains: Vec<String>,
    /// Private und lokale Ziele erlauben.
    pub allow_private_targets: bool,
    /// Ordner, in die Schreibwerkzeuge exportieren dürfen.
    pub export_dirs: Vec<PathBuf>,
    /// Lokaler HTTP-Zugang (Streamable HTTP auf Loopback).
    pub http: HttpSettings,
}

impl Default for McpSettings {
    /// Nur lesende Werkzeuge freigegeben, sonst nichts.
    fn default() -> Self {
        McpSettings {
            classes: vec![ToolClass::Read],
            tools: BTreeMap::new(),
            allowed_domains: Vec::new(),
            allow_private_targets: false,
            export_dirs: Vec::new(),
            http: HttpSettings::default(),
        }
    }
}

/// Einstellungen des lokalen HTTP-Zugangs.
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct HttpSettings {
    /// Port auf Loopback; `None` = nicht festgelegt.
    pub port: Option<u16>,
    /// Beim Start der Anwendung automatisch starten.
    pub autostart: bool,
}

/// Name der Einstellungsdatei im Einstellungsordner der Anwendung.
pub const FILE_NAME: &str = "mcp.toml";

/// Pfad der Einstellungsdatei: `<AppPaths.settings>/mcp.toml`. Der Ordner kommt ausschließlich aus
/// [`os_store::paths::AppPaths`] (mit `OPENSPIDER_HOME` ist er der Heimatordner selbst).
pub fn default_path() -> PathBuf {
    path_in(&os_store::paths::app_paths())
}

/// Pfad der Einstellungsdatei zu gegebenen Anwendungsordnern.
pub fn path_in(paths: &os_store::paths::AppPaths) -> PathBuf {
    paths.settings.join(FILE_NAME)
}

/// Lädt die Einstellungen; fehlt die Datei, gilt der Standard (nur `read`).
///
/// Eine vorhandene, aber nicht lesbare oder syntaktisch ungültige Datei ist ein Fehler
/// ([`McpError::Settings`] bzw. [`McpError::Io`]) und wird nie stillschweigend durch den Standard ersetzt:
/// Ein Tippfehler darf Freigaben nicht unbemerkt verändern.
pub fn load(path: &Path) -> Result<McpSettings, McpError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(McpSettings::default()),
        Err(e) => return Err(McpError::io(path, e)),
    };
    toml::from_str(&text)
        .map_err(|e| McpError::Settings(format!("{}: {}", path.display(), e.message())))
}

/// Speichert die Einstellungen atomar: erst in eine Temp-Datei im selben Ordner, dann umbenennen. Ein
/// abgebrochener Schreibvorgang hinterlässt nie eine halbe `mcp.toml`. Fehlende Ordner werden angelegt.
pub fn save(path: &Path, s: &McpSettings) -> Result<(), McpError> {
    use std::io::Write;

    let body = toml::to_string_pretty(s)
        .map_err(|e| McpError::Settings(format!("{}: {e}", path.display())))?;
    let text =
        format!("# OpenSpider MCP-Einstellungen. Nicht per MCP änderbar (AGENT-003).\n{body}");
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&dir).map_err(|e| McpError::io(&dir, e))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_NAME.to_string());
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| {
        // Aufräumen ist bestmöglich; der ursprüngliche Fehler zählt.
        let _ = std::fs::remove_file(&tmp);
        McpError::io(path, e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn customized() -> McpSettings {
        McpSettings {
            classes: vec![ToolClass::Read, ToolClass::Write],
            tools: BTreeMap::from([
                ("export_tab".to_string(), false),
                ("query_urls".to_string(), true),
            ]),
            allowed_domains: vec!["example.com".to_string()],
            allow_private_targets: true,
            export_dirs: vec![PathBuf::from("exports"), PathBuf::from("more").join("out")],
            http: HttpSettings {
                port: Some(7421),
                autostart: true,
            },
        }
    }

    // spec: AGENT-001, AGENT-003
    #[test]
    fn default_is_read_only_and_roundtrips_losslessly() {
        let d = McpSettings::default();
        assert_eq!(d.classes, [ToolClass::Read]);
        assert_eq!(d.http, HttpSettings::default());
        assert_eq!(d.http.port, None);
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<McpSettings>(&json).unwrap(), d);
        let toml_text = toml::to_string(&d).unwrap();
        assert_eq!(toml::from_str::<McpSettings>(&toml_text).unwrap(), d);
    }

    // spec: AGENT-001, AGENT-003
    #[test]
    fn customized_settings_roundtrip_as_json_and_toml() {
        let s = customized();
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<McpSettings>(&json).unwrap(), s);
        let toml_text = toml::to_string(&s).unwrap();
        assert!(toml_text.contains("\"read\""), "{toml_text}");
        assert!(toml_text.contains("export_tab = false"), "{toml_text}");
        assert_eq!(toml::from_str::<McpSettings>(&toml_text).unwrap(), s);
    }

    // spec: AGENT-001, AGENT-003
    #[test]
    fn partial_file_gets_defaults_for_missing_fields() {
        let s: McpSettings = toml::from_str("allow_private_targets = true\n").unwrap();
        assert_eq!(s.classes, [ToolClass::Read]);
        assert!(s.allow_private_targets);
        let s: McpSettings = toml::from_str("[http]\nport = 9000\n").unwrap();
        assert_eq!(s.http.port, Some(9000));
        assert!(!s.http.autostart);
    }

    // spec: AGENT-001, AGENT-003
    #[test]
    fn missing_file_is_the_read_only_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.toml");
        assert_eq!(load(&path).unwrap(), McpSettings::default());
        assert_eq!(load(&path).unwrap().classes, [ToolClass::Read]);
    }

    // spec: AGENT-001
    #[test]
    fn save_then_load_roundtrips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("mcp.toml");
        let s = customized();
        save(&path, &s).unwrap();
        assert_eq!(load(&path).unwrap(), s);
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["mcp.toml"], "keine Temp-Datei übrig");
        // Überschreiben ersetzt den Inhalt vollständig.
        save(&path, &McpSettings::default()).unwrap();
        assert_eq!(load(&path).unwrap(), McpSettings::default());
    }

    // spec: AGENT-001
    #[test]
    fn invalid_file_is_an_error_not_a_silent_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.toml");
        std::fs::write(&path, "classes = [\"read\", \"bogus\"]\n").unwrap();
        assert!(matches!(load(&path), Err(McpError::Settings(_))));
        std::fs::write(&path, "das ist kein toml ===").unwrap();
        assert!(matches!(load(&path), Err(McpError::Settings(_))));
    }

    // spec: AGENT-001
    #[test]
    fn documented_schema_example_parses_into_the_expected_values() {
        let text = r#"
classes = ["read", "crawl"]
allowed_domains = ["example.com"]
allow_private_targets = true
export_dirs = ["exports"]

[tools]
query_urls = false

[http]
port = 7421
autostart = true
"#;
        let s: McpSettings = toml::from_str(text).unwrap();
        assert_eq!(s.classes, [ToolClass::Read, ToolClass::Crawl]);
        assert_eq!(s.tools.get("query_urls"), Some(&false));
        assert_eq!(s.allowed_domains, ["example.com"]);
        assert!(s.allow_private_targets);
        assert_eq!(s.export_dirs, [PathBuf::from("exports")]);
        assert_eq!(s.http.port, Some(7421));
        assert!(s.http.autostart);
    }

    // spec: AGENT-001
    #[test]
    fn path_is_built_from_app_paths_only() {
        let home = std::ffi::OsStr::new("custom-home");
        let paths = os_store::paths::app_paths_with_home(Some(home));
        assert_eq!(path_in(&paths), Path::new("custom-home").join("mcp.toml"));
    }
}
