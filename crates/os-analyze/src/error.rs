//! Fehlertypen von `os-analyze`.

use os_store::StoreError;

/// Fehler bei der Namensauflösung `Tab:Filter`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// Unbekannter Name; `suggestions` enthält ähnliche gültige Namen (Levenshtein ≤ 3).
    #[error("unbekannter Name {input:?}")]
    Unknown {
        /// Eingabe.
        input: String,
        /// Vorschläge.
        suggestions: Vec<String>,
    },
}

/// Fehler in Ansichten (`views`).
#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    /// Fehler der Crawl-Datei.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// SQL-Fehler.
    #[error("SQLite-Fehler: {0}")]
    Sql(#[from] rusqlite::Error),
    /// Name nicht auflösbar.
    #[error(transparent)]
    Name(#[from] NameError),
    /// Cursor gehört zu einer anderen Abfrage oder einem anderen Crawl.
    #[error("Cursor gehört zu einer anderen Abfrage")]
    ForeignCursor,
    /// `limit` außerhalb 1–500.
    #[error("limit muss zwischen 1 und 500 liegen (war {0})")]
    InvalidLimit(u32),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}

/// Fehler der Crawl-Analyse.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzeError {
    /// Fehler der Crawl-Datei.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// SQL-Fehler.
    #[error("SQLite-Fehler: {0}")]
    Sql(#[from] rusqlite::Error),
    /// Noch nicht umgesetzt (Stub, `STUB(M1-Pxx)`).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
}
