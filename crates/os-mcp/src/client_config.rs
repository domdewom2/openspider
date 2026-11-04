//! Eigenen Eintrag `openspider` in fremde Client-Konfigurationen schreiben, ohne den Rest der Datei
//! anzufassen (AGENT-043, Nutzerentscheidung 04.11.2026). Nur für die lokale Oberfläche der App.
//!
//! Grundsatz: Alle Bytes außerhalb des eigenen Eintrags bleiben unverändert. Die Änderung ist immer ein
//! einzelner Ausschnitt der Datei, der eingefügt, ersetzt oder entfernt wird; Einrückung, Trenner und
//! Zeilenenden richten sich nach der Datei. Jedes Ergebnis wird vor der Rückgabe neu geparst und gegen
//! die Ausgangsdatei geprüft (fremde Werte gleich, eigener Eintrag gleich dem erzeugten). Was sich so
//! nicht ändern lässt, wird abgelehnt ([`ConfigError`]), nie umformatiert.
//!
//! [`mask`] liefert eine Fassung zum Anzeigen, in der Geheimnisse fremder Einträge maskiert sind; der
//! Diff der Vorschau entsteht aus zwei maskierten Fassungen.

mod json_file;
mod toml_file;

use std::fmt;

use crate::{
    print_config::{self, Client, SERVER_NAME},
    McpError,
};

/// Ersatztext für maskierte Werte (als gültiges JSON/TOML-String-Literal).
pub const MASK: &str = "\"\u{2022}\u{2022}\u{2022}\u{2022}\"";

/// Zustand des eigenen Eintrags in einer Client-Datei.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    /// Kein Eintrag `openspider`.
    Absent,
    /// Eintrag vorhanden und gleich dem erzeugten (Werte, nicht Schreibweise).
    Current,
    /// Eintrag vorhanden, aber abweichend (z. B. alter Programmpfad).
    Outdated,
}

/// Gründe, warum eine Datei nicht formatbewahrend geändert werden kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// JSON-Wurzel ist kein Objekt.
    RootNotObject,
    /// `mcpServers` bzw. `mcp_servers` ist kein Objekt bzw. keine Tabelle.
    ServersNotObject,
    /// `mcp_servers = { … }` als Inline-Tabelle auf Wurzelebene.
    InlineServers,
    /// Eigener Eintrag in einer Form, die sich nicht als ein Ausschnitt ändern lässt.
    OwnEntryShape,
    /// Doppelter Schlüssel `mcpServers` oder `openspider`.
    DuplicateKey,
    /// Das neu geparste Ergebnis weicht von der erwarteten Änderung ab.
    VerificationFailed,
}

/// Fehler beim formatbewahrenden Ändern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// Kein gültiges JSON/TOML; Zeile (1-basiert). Bewusst ohne Ausschnitt der Datei (Geheimnisse).
    Syntax {
        /// Zeile des Fehlers.
        line: usize,
    },
    /// Gültige Datei, die sich nicht formatbewahrend ändern lässt.
    Unsupported(Unsupported),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Syntax { line } => write!(f, "Datei ist nicht gültig (Zeile {line})"),
            ConfigError::Unsupported(u) => {
                let reason = match u {
                    Unsupported::RootNotObject => "Wurzel ist kein Objekt",
                    Unsupported::ServersNotObject => "Server-Liste ist kein Objekt",
                    Unsupported::InlineServers => "Server-Liste ist eine Inline-Tabelle",
                    Unsupported::OwnEntryShape => {
                        "Eintrag openspider liegt in einer anderen Form vor"
                    }
                    Unsupported::DuplicateKey => "doppelter Schlüssel",
                    Unsupported::VerificationFailed => "Prüfung des Ergebnisses misslungen",
                };
                write!(f, "Datei lässt sich nicht formatbewahrend ändern: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<ConfigError> for McpError {
    fn from(e: ConfigError) -> McpError {
        McpError::Settings(e.to_string())
    }
}

fn unsupported(u: Unsupported) -> ConfigError {
    ConfigError::Unsupported(u)
}

/// Zeile (1-basiert) eines Byte-Offsets.
fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// Zeilenende der Datei: CRLF, wenn sie welche enthält, sonst LF.
fn newline_of(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Erzeugter eigener Eintrag als JSON-Wert (`mcpServers.openspider`), wie `--print-config` (AGENT-001).
fn own_json(client: Client, exe: &str, args: &[String]) -> Result<serde_json::Value, ConfigError> {
    let text = print_config::render(client, exe, args)
        .map_err(|_| unsupported(Unsupported::VerificationFailed))?;
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| unsupported(Unsupported::VerificationFailed))?;
    Ok(doc["mcpServers"][SERVER_NAME].clone())
}

/// Erzeugter eigener Eintrag als TOML-Wert (`mcp_servers.openspider`), wie `--print-config codex`.
fn own_toml(exe: &str, args: &[String]) -> Result<toml::Value, ConfigError> {
    let text = print_config::render(Client::Codex, exe, args)
        .map_err(|_| unsupported(Unsupported::VerificationFailed))?;
    let doc: toml::Table =
        toml::from_str(&text).map_err(|_| unsupported(Unsupported::VerificationFailed))?;
    doc.get("mcp_servers")
        .and_then(|s| s.get(SERVER_NAME))
        .cloned()
        .ok_or_else(|| unsupported(Unsupported::VerificationFailed))
}

/// Zustand des eigenen Eintrags (AGENT-043).
///
/// # Errors
/// [`ConfigError`], wenn die Datei nicht parsebar ist oder ihr Aufbau nicht unterstützt wird.
pub fn entry_state(
    client: Client,
    before: &str,
    exe: &str,
    args: &[String],
) -> Result<EntryState, ConfigError> {
    if client == Client::Codex {
        toml_file::state(before, &own_toml(exe, args)?)
    } else {
        json_file::state(before, &own_json(client, exe, args)?)
    }
}

/// Fügt den eigenen Eintrag ein bzw. ersetzt ihn; alle übrigen Bytes bleiben gleich (AGENT-043).
///
/// # Errors
/// [`ConfigError`], wenn die Datei nicht parsebar ist, ihr Aufbau nicht unterstützt wird oder die
/// Prüfung des Ergebnisses misslingt.
pub fn connect(
    client: Client,
    before: &str,
    exe: &str,
    args: &[String],
) -> Result<String, ConfigError> {
    if client == Client::Codex {
        toml_file::connect(before, exe, args, &own_toml(exe, args)?)
    } else {
        json_file::connect(before, &own_json(client, exe, args)?)
    }
}

/// Entfernt nur den eigenen Eintrag; alle übrigen Bytes bleiben gleich (AGENT-043). Ohne Eintrag
/// bleibt der Text unverändert.
///
/// # Errors
/// Wie [`connect`].
pub fn disconnect(client: Client, before: &str) -> Result<String, ConfigError> {
    if client == Client::Codex {
        toml_file::disconnect(before)
    } else {
        json_file::disconnect(before)
    }
}

/// Fassung zum Anzeigen: Geheimnisse außerhalb des eigenen Eintrags sind durch [`MASK`] ersetzt, alle
/// anderen Bytes unverändert (AGENT-043, „Vorschau ohne Geheimnisse“).
///
/// # Errors
/// [`ConfigError::Syntax`], wenn die Datei nicht parsebar ist.
pub fn mask(client: Client, text: &str) -> Result<String, ConfigError> {
    if client == Client::Codex {
        toml_file::mask(text)
    } else {
        json_file::mask(text)
    }
}

/// Wörter, die einen Schlüssel als geheim kennzeichnen (Teilwort, Groß-/Kleinschreibung egal).
const SECRET_WORDS: [&str; 8] = [
    "key",
    "token",
    "secret",
    "password",
    "passwd",
    "authorization",
    "cookie",
    "credential",
];

/// Schlüssel, dessen ganzer Wert (Teilbaum) maskiert wird.
pub(crate) fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower == "env" || lower == "headers" || SECRET_WORDS.iter().any(|w| lower.contains(w))
}

/// Befehlszeilenschalter, dessen Wert geheim ist (`--token`, `--api-key`, `--password=…`).
pub(crate) fn is_secret_flag(arg: &str) -> bool {
    let Some(name) = arg.strip_prefix('-') else {
        return false;
    };
    let name = name
        .split('=')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    SECRET_WORDS.iter().any(|w| name.contains(w))
}

/// Zeichenkette, die unabhängig vom Schlüssel geheim aussieht.
pub(crate) fn is_secret_string(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("bearer ") || lower.contains("authorization:") {
        return true;
    }
    if value.starts_with('-') && value.contains('=') && is_secret_flag(value) {
        return true;
    }
    if let Some(scheme_end) = lower.find("://") {
        let rest = &lower[scheme_end + 3..];
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.contains('@') {
            return true;
        }
        if let Some((_, query)) = rest.split_once('?') {
            if query
                .split(['&', ';', '#'])
                .any(|pair| pair.split('=').next().is_some_and(is_secret_key))
            {
                return true;
            }
        }
    }
    false
}

/// Ersetzt die (nicht überlappenden) Bereiche durch [`MASK`].
fn splice_masks(text: &str, mut spans: Vec<std::ops::Range<usize>>) -> String {
    spans.sort_by_key(|r| r.start);
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for r in spans {
        if r.start < pos {
            continue;
        }
        out.push_str(&text[pos..r.start]);
        out.push_str(MASK);
        pos = r.end;
    }
    out.push_str(&text[pos..]);
    out
}

#[cfg(test)]
mod tests;
