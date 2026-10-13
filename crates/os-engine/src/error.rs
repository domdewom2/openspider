//! Fehlertyp von `os-engine`.

use os_model::ModelError;
use os_store::StoreError;

/// Fehler der Engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Fehler der Crawl-Datei.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Modellfehler (Konfiguration, Zeitstempel).
    #[error(transparent)]
    Model(#[from] ModelError),
    /// Dateisystemfehler.
    #[error("Dateifehler: {0}")]
    Io(#[from] std::io::Error),
    /// Ungültige Eingabe (Start-URL, Konfiguration).
    #[error("ungültige Eingabe: {0}")]
    Invalid(String),
    /// Ein Hintergrund-Task ist abgebrochen oder in Panik geraten.
    #[error("interner Fehler: {0}")]
    Join(String),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}
