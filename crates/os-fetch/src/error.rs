//! Fehlertyp von `os-fetch`.

/// Fehler beim Aufbau des Abrufers (Abrufe selbst liefern `FetchOutcome::NoResponse`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    /// Ungültige Konfiguration (z. B. Resolve-Regel, Zertifikat).
    #[error("ungültige Abruf-Konfiguration: {0}")]
    Config(String),
    /// Noch nicht umgesetzt (Stub).
    #[error("nicht implementiert")]
    NotImplemented,
}
