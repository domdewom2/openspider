//! Ausgabeordner (OUT-013) und Auswahl des Tabellenschreibers je Format (`open_writer`).
//!
//! * [`OutputMode::Overwrite`]: Ordner wird bei Bedarf angelegt; die Schreiber ersetzen nur gleichnamige
//!   Dateien, andere Dateien bleiben unangetastet.
//! * [`OutputMode::Timestamped`]: Unterordner `YYYY-MM-DD_HH-MM-SS` in lokaler Zeit; existiert er schon
//!   (gleiche Sekunde), folgt `_2`, `_3` ….
//! * [`OutputMode::RequireEmpty`]: Fehler, wenn der Ordner existiert und nicht leer ist.
//!
//! Alle Fehler sind [`ExportError::Output`]; die CLI bildet sie auf Exit-Code 2 ab, bevor der Crawl beginnt.

use std::path::{Path, PathBuf};

use os_model::Clock;
use time::UtcOffset;

use crate::csv::CsvWriter;
use crate::error::ExportError;
use crate::fs_paths;
use crate::types::{Format, TableWriter, WriterOptions};
use crate::xlsx::XlsxWriter;

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
///
/// Die Uhr kommt von außen (`Clock`), damit die Ordnernamen deterministisch testbar sind; den lokalen
/// UTC-Offset liefert `os_model::time::init_local_offset` (bzw. das System).
pub fn prepare(base: &Path, mode: OutputMode, clock: &dyn Clock) -> Result<PathBuf, ExportError> {
    prepare_impl(base, mode, |_| clock.now().to_export())
}

/// Wie [`prepare`], mit ausdrücklichem UTC-Offset statt der lokalen Zeitzone (Tests, feste Zeitzonen).
pub fn prepare_with_offset(
    base: &Path,
    mode: OutputMode,
    clock: &dyn Clock,
    offset: UtcOffset,
) -> Result<PathBuf, ExportError> {
    prepare_impl(base, mode, |_| clock.now().to_export_with(offset))
}

fn prepare_impl(
    base: &Path,
    mode: OutputMode,
    now_export: impl Fn(()) -> String,
) -> Result<PathBuf, ExportError> {
    match mode {
        OutputMode::Overwrite => {
            ensure_dir(base)?;
            Ok(base.to_path_buf())
        }
        OutputMode::RequireEmpty => {
            let real = fs_paths::fs_path(base);
            if real.exists() {
                if !real.is_dir() {
                    return Err(ExportError::Output(format!(
                        "{} ist kein Ordner",
                        base.display()
                    )));
                }
                let mut entries = std::fs::read_dir(&real).map_err(|e| {
                    ExportError::Output(format!("{} nicht lesbar: {e}", base.display()))
                })?;
                if entries.next().is_some() {
                    return Err(ExportError::Output(format!(
                        "Der Ausgabeordner {} ist nicht leer (require-empty)",
                        base.display()
                    )));
                }
                Ok(base.to_path_buf())
            } else {
                ensure_dir(base)?;
                Ok(base.to_path_buf())
            }
        }
        OutputMode::Timestamped => {
            ensure_dir(base)?;
            let stamp = dir_stamp(&now_export(()));
            let mut n = 1u32;
            loop {
                let name = if n == 1 {
                    stamp.clone()
                } else {
                    format!("{stamp}_{n}")
                };
                let candidate = base.join(&name);
                // `create_dir` (nicht `_all`) schlägt fehl, wenn der Ordner existiert: atomar gegen
                // gleichzeitige Läufe.
                match std::fs::create_dir(fs_paths::fs_path(&candidate)) {
                    Ok(()) => return Ok(candidate),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                    Err(e) => {
                        return Err(ExportError::Output(format!(
                            "Ordner {} nicht anlegbar: {e}",
                            candidate.display()
                        )))
                    }
                }
            }
        }
    }
}

/// `2026-10-13T09:35:00+02:00` → `2026-10-13_09-35-00`.
fn dir_stamp(export: &str) -> String {
    export
        .chars()
        .take(19)
        .map(|c| match c {
            'T' => '_',
            ':' => '-',
            other => other,
        })
        .collect()
}

fn ensure_dir(dir: &Path) -> Result<(), ExportError> {
    fs_paths::create_dir_all(dir).map_err(|e| {
        ExportError::Output(format!(
            "Ausgabeordner {} nicht anlegbar: {e}",
            dir.display()
        ))
    })?;
    if !fs_paths::fs_path(dir).is_dir() {
        return Err(ExportError::Output(format!(
            "{} ist kein Ordner",
            dir.display()
        )));
    }
    Ok(())
}

/// Öffnet einen Tabellenschreiber für das Format.
pub fn open_writer(
    path: &Path,
    format: Format,
    sheet: &str,
    opts: &WriterOptions,
) -> Result<Box<dyn TableWriter>, ExportError> {
    match format {
        Format::Csv => Ok(Box::new(CsvWriter::create(path, opts)?)),
        Format::Xlsx => Ok(Box::new(XlsxWriter::create(path, sheet, opts)?)),
    }
}
