//! XML-Sitemap-Generator (OUT-120 ff.).
//!
//! STUB(M1-P17)

use std::path::Path;

use os_store::CrawlFile;

use crate::error::ExportError;
use crate::types::{SitemapOptions, SitemapReport};

/// Erzeugt `sitemap_N.xml` und bei Bedarf `sitemap_index.xml`.
// STUB(M1-P17)
pub fn generate_sitemap(
    _file: &CrawlFile,
    _opts: &SitemapOptions,
    _out_dir: &Path,
) -> Result<SitemapReport, ExportError> {
    Err(ExportError::NotImplemented("generate_sitemap (M1-P17)"))
}
