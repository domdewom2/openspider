//! Tab-Export: eine Ansicht (Tab und Filter) als CSV/XLSX.
//!
//! STUB(M1-P16)

use std::path::Path;

use os_analyze::views::ViewQuery;
use os_model::CrawlConfig;
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::types::{ExportFileInfo, Format};

/// Exportiert die Ansicht in den Ausgabeordner.
// STUB(M1-P16)
pub fn export_tab(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _q: &ViewQuery,
    _out_dir: &Path,
    _format: Format,
) -> Result<ExportFileInfo, ExportError> {
    Err(ExportError::NotImplemented("export_tab (M1-P16)"))
}
