//! Ausgabeordner (OUT-013) und Auswahl des Tabellenschreibers je Format (`open_writer`).
//!
//! STUB(M1-P08)

use std::path::{Path, PathBuf};

use os_model::Clock;

use crate::error::ExportError;
use crate::types::{Format, TableWriter, WriterOptions};

/// Umgang mit vorhandenen Dateien im Ausgabeordner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Gleichnamige Dateien ersetzen.
    Overwrite,
    /// Unterordner mit Zeitstempel `YYYY-MM-DD_HH-MM-SS` (lokal).
    Timestamped,
    /// Fehler, wenn der Ordner nicht leer ist.
    RequireEmpty,
}

/// Bereitet den Ausgabeordner vor und liefert den tatsächlichen Pfad.
// STUB(M1-P08)
pub fn prepare(
    _base: &Path,
    _mode: OutputMode,
    _clock: &dyn Clock,
) -> Result<PathBuf, ExportError> {
    Err(ExportError::NotImplemented("output::prepare (M1-P08)"))
}

/// Öffnet einen Tabellenschreiber für das Format.
// STUB(M1-P08)
pub fn open_writer(
    _path: &Path,
    _format: Format,
    _sheet: &str,
    _opts: &WriterOptions,
) -> Result<Box<dyn TableWriter>, ExportError> {
    Err(ExportError::NotImplemented("open_writer (M1-P08)"))
}
