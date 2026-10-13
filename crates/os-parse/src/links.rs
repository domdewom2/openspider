//! Links aus dem Dokument (CRAWL-052): Hyperlinks und Ressourcen.
//!
//! STUB(M1-P04)

use os_model::ExtractedLink;

use crate::dom::Document;
use crate::ParseConfig;

/// Extrahiert alle Links in Dokumentreihenfolge.
// STUB(M1-P04)
pub fn extract(_doc: &Document, _base: &url::Url, _cfg: &ParseConfig) -> Vec<ExtractedLink> {
    Vec::new()
}
