//! Einstellungen des MCP-Servers (`mcp.toml`, AGENT-010 bis AGENT-012): freigegebene Werkzeugklassen,
//! einzelne Werkzeuge, erlaubte Domains, Exportordner und der lokale HTTP-Zugang (M2).
//!
//! STUB(M1-P18): M1-P00c legt die Typen samt Serde-Verhalten an; `load` liefert ohne Dateizugriff den
//! Standard, `save` ist noch nicht umgesetzt. P18 setzt beide um (`save` atomar: temp + rename).

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

/// Lädt die Einstellungen; fehlt die Datei, gilt der Standard (nur `read`).
// STUB(M1-P18): liefert immer den Standard.
pub fn load(_path: &Path) -> Result<McpSettings, McpError> {
    Ok(McpSettings::default())
}

/// Speichert die Einstellungen atomar (temp + rename).
// STUB(M1-P18)
pub fn save(_path: &Path, _s: &McpSettings) -> Result<(), McpError> {
    Err(McpError::NotImplemented("settings::save (M1-P18)"))
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

    #[test]
    fn partial_file_gets_defaults_for_missing_fields() {
        let s: McpSettings = toml::from_str("allow_private_targets = true\n").unwrap();
        assert_eq!(s.classes, [ToolClass::Read]);
        assert!(s.allow_private_targets);
        let s: McpSettings = toml::from_str("[http]\nport = 9000\n").unwrap();
        assert_eq!(s.http.port, Some(9000));
        assert!(!s.http.autostart);
    }

    #[test]
    fn stub_load_returns_default_and_save_is_not_implemented() {
        let path = Path::new("does-not-exist").join("mcp.toml");
        assert_eq!(load(&path).unwrap(), McpSettings::default());
        assert!(matches!(
            save(&path, &McpSettings::default()),
            Err(McpError::NotImplemented(_))
        ));
    }
}
