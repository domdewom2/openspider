//! Detail-Export (Inlinks/Outlinks) für URL-Listen.
//!
//! STUB(M1-P16)

use std::path::Path;

use os_model::UrlId;
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::types::{DetailKind, ExportFileInfo, Format};

/// Exportiert Inlinks bzw. Outlinks der angegebenen URLs im Standard-Linkformat (ONPAGE-192).
// STUB(M1-P16)
pub fn export_detail(
    _file: &CrawlFile,
    _detail: DetailKind,
    _urls: &[UrlId],
    _out: &Path,
    _format: Format,
) -> Result<ExportFileInfo, ExportError> {
    Err(ExportError::NotImplemented("export_detail (M1-P16)"))
}
