//! Canonical-Angaben (`<link rel=canonical>`, TECH-011 ff.).
//!
//! STUB(M1-P06)

use os_model::CanonicalRef;

use crate::dom::Document;

/// Extrahiert alle Canonical-Angaben.
// STUB(M1-P06)
pub fn extract(_doc: &Document, _base: &url::Url) -> Vec<CanonicalRef> {
    Vec::new()
}
