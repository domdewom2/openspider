//! Fehlerformat aller Commands (`docs/plan/M2-ui-concept.md` 4.2).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Fehlerklasse eines Commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ErrorCode {
    /// Eingabe ungültig (mit `field` bzw. Vorschlägen in `args.suggestions`).
    InvalidInput,
    /// Objekt nicht gefunden.
    NotFound,
    /// Versionskonflikt (View, Snapshot).
    Stale,
    /// Abgebrochen (`op.cancel`, SQLite-Interrupt).
    Cancelled,
    /// Ressource belegt.
    Busy,
    /// Zeitlimit überschritten.
    Timeout,
    /// Ein-/Ausgabefehler.
    Io,
    /// Berechtigung fehlt.
    Permission,
    /// Nicht unterstützt oder noch nicht implementiert (Stub).
    Unsupported,
    /// Kein Crawl geöffnet.
    NoCrawl,
    /// Während eines laufenden Crawls nicht möglich.
    CrawlRunning,
    /// Fehler der Crawl-Datei.
    Store,
    /// Fehler der Engine.
    Engine,
    /// Interner Fehler (Panic im Command wird abgefangen, geloggt).
    Internal,
}

/// Fehler eines Commands; die Oberfläche übersetzt `message_key` mit `args` (Fluent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
#[error("{code:?}: {message_key}")]
pub struct CmdError {
    /// Fehlerklasse.
    pub code: ErrorCode,
    /// i18n-Schlüssel, z. B. `err-export-dir-not-writable`.
    pub message_key: String,
    /// Argumente der Meldung, z. B. `{"path": "/Users/…/Exports"}`.
    pub args: BTreeMap<String, serde_json::Value>,
    /// Bei `InvalidInput`: TOML-Pfad bzw. Feldname.
    pub field: Option<String>,
    /// Technische Ursache (englisch, redigiert, OUT-243).
    pub detail: Option<String>,
    /// Erneuter Versuch sinnvoll.
    pub retryable: bool,
}

impl CmdError {
    /// Fehler mit Klasse und Schlüssel, ohne Argumente.
    pub fn new(code: ErrorCode, message_key: impl Into<String>) -> CmdError {
        CmdError {
            code,
            message_key: message_key.into(),
            args: BTreeMap::new(),
            field: None,
            detail: None,
            retryable: false,
        }
    }

    /// Stub-Antwort: der Command ist im Katalog, aber noch nicht umgesetzt (`STUB(M2-xx)`).
    pub fn unsupported(command: &str, owner: &str) -> CmdError {
        CmdError::new(ErrorCode::Unsupported, "err-not-implemented")
            .with_arg("command", command)
            .with_arg("owner", owner)
    }

    /// Ungültige Eingabe mit Feldname.
    pub fn invalid_input(message_key: impl Into<String>, field: impl Into<String>) -> CmdError {
        CmdError {
            field: Some(field.into()),
            ..CmdError::new(ErrorCode::InvalidInput, message_key)
        }
    }

    /// Hängt ein Argument an.
    pub fn with_arg(mut self, key: &str, value: impl Into<serde_json::Value>) -> CmdError {
        self.args.insert(key.to_string(), value.into());
        self
    }

    /// Hängt die technische Ursache an.
    pub fn with_detail(mut self, detail: impl Into<String>) -> CmdError {
        self.detail = Some(detail.into());
        self
    }

    /// Markiert den Fehler als wiederholbar.
    pub fn retryable(mut self) -> CmdError {
        self.retryable = true;
        self
    }
}
