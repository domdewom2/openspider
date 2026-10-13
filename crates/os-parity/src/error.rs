//! Fehlertyp von `os-parity`.

/// Fehler der Unterbefehle.
#[derive(Debug, thiserror::Error)]
pub enum ParityError {
    /// Ein-/Ausgabefehler.
    #[error("Ein-/Ausgabefehler: {0}")]
    Io(#[from] std::io::Error),
    /// Ungültige Eingabedatei (CSV, TOML, JSON).
    #[error("ungültige Eingabe: {0}")]
    Invalid(String),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-P11)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}
