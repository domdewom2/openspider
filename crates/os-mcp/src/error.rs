//! Fehler des Servers (Transport, Protokoll, Freigaben).
//!
//! STUB(M1-P18): Modul wird von M1-P18 umgesetzt; M1-P00c legt nur den Fehlertyp `McpError` an, den
//! `settings::{load, save}` brauchen.

use std::path::PathBuf;

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
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}
