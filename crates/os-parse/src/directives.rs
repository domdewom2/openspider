//! Robots-Direktiven aus `<meta name=robots|<token>>` (TECH-051 ff.).
//!
//! STUB(M1-P06)

use os_model::Directive;

use crate::dom::Document;

/// Extrahiert die Direktiven für die angegebenen Agent-Token.
// STUB(M1-P06)
pub fn extract(_doc: &Document, _tokens: &[String]) -> Vec<Directive> {
    Vec::new()
}
