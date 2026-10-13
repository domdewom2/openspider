//! Onpage-Elemente: Titel, Meta-Descriptions, Meta-Keywords, H1, H2 (ONPAGE-090 ff.).
//!
//! STUB(M1-P05)

use os_model::ExtractedText;

use crate::dom::Document;

/// Vollständige Listen der erfassten Elemente in Dokumentreihenfolge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementSets {
    /// `<title>`.
    pub titles: Vec<ExtractedText>,
    /// `<meta name="description">`.
    pub meta_descriptions: Vec<ExtractedText>,
    /// `<meta name="keywords">`.
    pub meta_keywords: Vec<ExtractedText>,
    /// `<h1>`.
    pub h1: Vec<ExtractedText>,
    /// `<h2>`.
    pub h2: Vec<ExtractedText>,
}

/// Extrahiert die Elemente.
// STUB(M1-P05)
pub fn extract(_doc: &Document) -> ElementSets {
    ElementSets::default()
}
