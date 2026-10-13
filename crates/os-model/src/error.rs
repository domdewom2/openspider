//! Fehlertyp von `os-model`.

/// Fehler beim Parsen und (De-)Serialisieren von Modellwerten.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// Unbekannter Wert für ein Enum (`kind` = Enum-Name).
    #[error("unbekannter Wert {value:?} für {kind}")]
    UnknownValue {
        /// Name des Enums.
        kind: &'static str,
        /// Der abgelehnte Wert.
        value: String,
    },
    /// Zeitstempel nicht lesbar.
    #[error("ungültiger Zeitstempel {0:?}")]
    InvalidTimestamp(String),
    /// Ungültige Crawl-ID.
    #[error("ungültige Crawl-ID {0:?}")]
    InvalidCrawlId(String),
    /// Konfiguration nicht lesbar (TOML-Syntax oder Typfehler).
    #[error("Konfiguration nicht lesbar: {0}")]
    Config(String),
}
