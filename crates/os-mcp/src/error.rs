//! Fehler des Servers (Transport, Protokoll, Freigaben, Einstellungen).
//!
//! `McpError` ist der Fehlertyp von `settings::{load, save}`, der Freigabeauflösung (`permissions`), der
//! Start-Kommandozeile (`cli`) und des Transports. [`McpError::exit_code`] ordnet jeden Fehler einem
//! Prozess-Exit-Code zu: Aufruffehler (widersprüchliche oder unbekannte Schalter) ergeben 2, alles andere 1.

use std::path::PathBuf;

/// Exit-Code für Aufruffehler (Widerspruch `--read-only`/`--allow`, unbekannte Klasse, …).
pub const EXIT_USAGE: i32 = 2;
/// Exit-Code für Laufzeitfehler (Einstellungen unlesbar, Transport gestört, …).
pub const EXIT_FAILURE: i32 = 1;

/// Fehler des MCP-Servers und seiner Einstellungen.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Dateisystemfehler.
    #[error("Dateifehler bei {path}: {source}")]
    Io {
        /// Betroffener Pfad.
        path: PathBuf,
        /// Ursache.
        #[source]
        source: std::io::Error,
    },
    /// Einstellungsdatei nicht lesbar oder nicht schreibbar (TOML).
    #[error("Einstellungen ungültig: {0}")]
    Settings(String),
    /// Fehlerhafter Aufruf: widersprüchliche oder unbekannte Schalter (Exit-Code 2).
    #[error("{0}")]
    Usage(String),
    /// Transport- oder Protokollfehler beim Betrieb des Servers.
    #[error("MCP-Transport: {0}")]
    Transport(String),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}

impl McpError {
    /// Dateisystemfehler mit Pfad.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        McpError::Io {
            path: path.into(),
            source,
        }
    }

    /// Prozess-Exit-Code zu diesem Fehler: 2 für Aufruffehler, sonst 1.
    pub fn exit_code(&self) -> i32 {
        match self {
            McpError::Usage(_) => EXIT_USAGE,
            _ => EXIT_FAILURE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: AGENT-001
    #[test]
    fn usage_errors_exit_with_two_everything_else_with_one() {
        assert_eq!(McpError::Usage("x".into()).exit_code(), 2);
        assert_eq!(McpError::Settings("x".into()).exit_code(), 1);
        assert_eq!(McpError::Transport("x".into()).exit_code(), 1);
        assert_eq!(McpError::NotImplemented("x").exit_code(), 1);
        let io = McpError::io("a", std::io::Error::other("kaputt"));
        assert_eq!(io.exit_code(), 1);
        assert!(io.to_string().contains("kaputt"));
    }
}
