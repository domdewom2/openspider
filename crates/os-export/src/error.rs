//! Fehlertyp von `os-export`.

use std::path::PathBuf;

use os_analyze::{NameError, ViewError};
use os_store::StoreError;

/// Fehler beim Exportieren.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// Dateisystemfehler.
    #[error("Dateifehler bei {path}: {source}")]
    Io {
        /// Betroffener Pfad.
        path: PathBuf,
        /// Ursache.
        #[source]
        source: std::io::Error,
    },
    /// Format wird in M1 nicht unterstützt (xls, gsheet) – die CLI beendet sich mit Exit 2.
    #[error("Format nicht unterstützt: {0}")]
    Unsupported(String),
    /// Fehler beim Schreiben eines Formats.
    #[error("Schreibfehler: {0}")]
    Write(String),
    /// Der Ausgabeordner ist nicht leer (`--require-empty`) oder nicht anlegbar.
    #[error("Ausgabeordner ungeeignet: {0}")]
    Output(String),
    /// Fehler einer Ansicht.
    #[error(transparent)]
    View(#[from] ViewError),
    /// Fehler der Crawl-Datei.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Name nicht auflösbar.
    #[error(transparent)]
    Name(#[from] NameError),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}
