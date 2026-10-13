//! Export-Pipeline: Ansicht → `TableWriter` in konstantem Speicher; Bulk-Exporte über den Katalog.
//!
//! STUB(M1-P16): P16 setzt die Pipeline und die Links-Bulk-Exporte um, P17 die Issues-Exporte.

use std::path::Path;

use os_model::CrawlConfig;
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::types::{ExportFileInfo, Format};

/// Führt einen Bulk-Export aus dem Katalog aus (z. B. „All Inlinks“); kann mehrere Dateien liefern.
// STUB(M1-P16)
pub fn export_bulk(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _name: &str,
    _out_dir: &Path,
    _format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    Err(ExportError::NotImplemented("export_bulk (M1-P16/P17)"))
}
