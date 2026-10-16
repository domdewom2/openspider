//! MCP-Server in der App (`mcp.*`, UI-129, OUT-285, K-17).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ein MCP-Werkzeug (`mcp.tools`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Name.
    pub name: String,
    /// Klasse: `read`, `crawl`, `write` oder `cost`.
    pub class: String,
    /// Freigegeben.
    pub enabled: bool,
    /// Beschreibung.
    pub description: String,
    /// JSON-Schema der Eingabe.
    pub schema: serde_json::Value,
}

/// Argument von `mcp.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct McpStartArgs {
    /// Port auf Loopback; `null` = gespeicherter oder freier Port.
    pub port: Option<u16>,
}

/// Argument von `mcp.setToolEnabled`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetToolEnabledArgs {
    /// Werkzeug.
    pub name: String,
    /// Freigeben oder sperren.
    pub enabled: bool,
}

/// Argument von `mcp.setClasses`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetClassesArgs {
    /// Freigegebene Klassen.
    pub classes: Vec<String>,
}

/// Argument von `mcp.token` und `mcp.regenerateToken`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpTokenArgs {
    /// Klartext mitliefern.
    pub reveal: bool,
}

/// Antwort zum Zugangstoken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct McpToken {
    /// Maskierte Darstellung.
    pub masked: String,
    /// Klartext, nur bei `reveal`.
    pub value: Option<String>,
}

/// Unterstützte MCP-Clients (`mcp.clientConfig*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum McpClientKind {
    /// Claude Desktop.
    ClaudeDesktop,
    /// Claude Code.
    ClaudeCode,
    /// Codex.
    Codex,
}

/// Argument von `mcp.clientConfigPreview` und `mcp.clientConfigWrite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpClientArgs {
    /// Client.
    pub client: McpClientKind,
}

/// Vorschau der Client-Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct McpClientPreview {
    /// Konfigurationsdatei des Clients.
    pub path: String,
    /// Inhalt vorher.
    pub before: String,
    /// Inhalt nachher.
    pub after: String,
    /// Unterschiede (Unified Diff).
    pub diff: String,
}

/// Ergebnis von `mcp.clientConfigWrite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct McpClientWritten {
    /// Sicherungskopie der alten Datei (`null`, wenn es keine gab).
    pub backup_path: Option<String>,
}

/// Eine Zeile des Aufrufprotokolls (ohne Geheimnisse).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CallLogRow {
    /// Zeitpunkt (ISO 8601 UTC).
    pub at: String,
    /// Werkzeug.
    pub tool: String,
    /// Klasse.
    pub class: String,
    /// Client.
    pub client: Option<String>,
    /// Dauer in ms.
    pub duration_ms: u32,
    /// Erfolgreich.
    pub ok: bool,
    /// Kurzfassung der Argumente (redigiert).
    pub args_summary: String,
}
