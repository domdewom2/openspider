//! Gemeinsame Typen des Exports (M1-Plan 3.6).

use std::path::PathBuf;
use std::str::FromStr;

use os_analyze::ColumnKind;
use os_model::{LineEnding, Value};

use crate::error::ExportError;

/// Ausgabeformat. `xls` und `gsheet` sind in M1 nicht unterstützt (Exit 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// CSV (UTF-8, RFC 4180).
    Csv,
    /// Excel (`.xlsx`).
    Xlsx,
}

impl Format {
    /// Dateiendung ohne Punkt.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Xlsx => "xlsx",
        }
    }
}

impl FromStr for Format {
    type Err = ExportError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => Ok(Format::Csv),
            "xlsx" => Ok(Format::Xlsx),
            other => Err(ExportError::Unsupported(other.to_string())),
        }
    }
}

/// Optionen der Tabellenschreiber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriterOptions {
    /// UTF-8-BOM am Dateianfang (CSV).
    pub utf8_bom: bool,
    /// Zeilenende (CSV).
    pub line_ending: LineEnding,
    /// Höchste Zahl Datenzeilen je Blatt (XLSX); danach folgt ein Folgeblatt „<Name> (2)“.
    pub max_rows_per_sheet: u32,
}

impl Default for WriterOptions {
    fn default() -> Self {
        WriterOptions {
            utf8_bom: true,
            line_ending: LineEnding::Crlf,
            max_rows_per_sheet: 1_048_575,
        }
    }
}

/// Angaben zu einer geschriebenen Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFileInfo {
    /// Pfad der Datei.
    pub path: PathBuf,
    /// Anzahl der Datenzeilen (ohne Kopfzeile).
    pub rows: u64,
    /// Größe der Datei in Bytes.
    pub bytes: u64,
    /// Hinweise zu dieser Datei (OUT-003): auf 32 767 Zeichen gekürzte Zellen, Fortsetzung auf Folgeblättern.
    /// Dieselben Meldungen stehen zusätzlich im Log.
    pub warnings: Vec<String>,
}

/// Zeilenweiser Tabellenschreiber (konstanter Speicher).
pub trait TableWriter {
    /// Schreibt die Kopfzeile.
    fn header(&mut self, columns: &[String]) -> Result<(), ExportError>;
    /// Schreibt eine Datenzeile; `kinds` bestimmt die Zellart je Spalte.
    fn row(&mut self, kinds: &[ColumnKind], values: &[Value]) -> Result<(), ExportError>;
    /// Schließt die Datei ab.
    fn finish(self: Box<Self>) -> Result<ExportFileInfo, ExportError>;
}

/// Art des Detail-Exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailKind {
    /// Eingehende Links.
    Inlinks,
    /// Ausgehende Links.
    Outlinks,
}

/// Ergebnis von `save_report`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReportResult {
    /// Geschriebene Dateien.
    pub files: Vec<ExportFileInfo>,
    /// Hinweise, z. B. fehlende Voraussetzungen (OUT-112).
    pub warnings: Vec<String>,
}

/// Optionen des XML-Sitemap-Generators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitemapOptions {
    /// Höchste Zahl URLs je Sitemap-Datei.
    pub max_urls_per_file: u32,
    /// Höchste Größe je Sitemap-Datei in Bytes.
    pub max_bytes_per_file: u64,
    /// Basis-URL für die `<loc>` des Sitemap-Index (OUT-129), z. B. `https://example.com/`. `None` = Ursprung
    /// der Start-URL des Crawls.
    pub base_url: Option<String>,
}

impl Default for SitemapOptions {
    fn default() -> Self {
        SitemapOptions {
            max_urls_per_file: 49_999,
            max_bytes_per_file: 50 * 1024 * 1024,
            base_url: None,
        }
    }
}

/// Ergebnis des Sitemap-Generators.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SitemapReport {
    /// Geschriebene Dateien (`sitemap_N.xml`, `sitemap_index.xml`).
    pub files: Vec<ExportFileInfo>,
    /// Aufgenommene URLs.
    pub urls: u64,
    /// Wegen Überlänge (> 2 048 Zeichen) ausgelassene URLs.
    pub skipped_too_long: Vec<String>,
}

/// Ziel für einzelne Blätter eines gemeinsamen Workbooks (OUT-075).
pub trait WorkbookTarget {
    fn sheet(&mut self, name: &str) -> Result<&mut dyn TableWriter, ExportError>;
}
/// Gemeinsame Steuerung aller Exportpfade (OUT-010).
pub struct ExportCtx<'a> {
    pub cancel: &'a os_model::CancelToken,
    pub progress: &'a dyn Fn(u64),
    pub skip_empty: bool,
    pub workbook: Option<&'a mut dyn WorkbookTarget>,
    pub skipped: Vec<String>,
}
impl ExportCtx<'_> {
    pub fn checkpoint(&self) -> Result<(), ExportError> {
        self.cancel
            .checkpoint()
            .map_err(|_| ExportError::Write("Export abgebrochen".into()))
    }
}
