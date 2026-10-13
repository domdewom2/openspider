//! Inhaltsbereich, Wortzahl und Textanteil (ONPAGE-140 ff.).
//!
//! STUB(M1-P05)

use os_model::ContentAreaConfig;

use crate::dom::Document;

/// Kennzahlen des Inhaltsbereichs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContentStats {
    /// Anzahl der Wörter im Inhaltsbereich.
    pub word_count: u32,
    /// Textanteil in Prozent.
    pub text_ratio: f32,
}

/// Berechnet Wortzahl und Textanteil.
// STUB(M1-P05): 0 / 0.0
pub fn analyze(_doc: &Document, _area: &ContentAreaConfig) -> ContentStats {
    ContentStats::default()
}
