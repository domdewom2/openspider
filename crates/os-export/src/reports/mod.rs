//! Reports: Crawl Overview, Redirects, Issues (OUT-080 ff.).
//!
//! STUB(M1-P17)

use std::path::Path;

use os_model::CrawlConfig;
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::types::{Format, ReportResult};

/// Speichert einen Report aus dem Katalog.
// STUB(M1-P17)
pub fn save_report(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _name: &str,
    _out_dir: &Path,
    _format: Format,
) -> Result<ReportResult, ExportError> {
    Err(ExportError::NotImplemented("save_report (M1-P17)"))
}
