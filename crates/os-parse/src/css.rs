//! Verweise aus CSS: `<style>`, `style=` und CSS-Dateien.
//!
//! STUB(M1-P04)

use os_model::ExtractedLink;

use crate::dom::Document;

/// Verweise aus `<style>`-Elementen und `style=`-Attributen.
// STUB(M1-P04)
pub fn extract_inline(_doc: &Document, _base: &url::Url) -> Vec<ExtractedLink> {
    Vec::new()
}

/// Verweise aus einer CSS-Datei.
// STUB(M1-P04)
pub fn extract_sheet(_base: &url::Url, _body: &[u8]) -> Vec<ExtractedLink> {
    Vec::new()
}
