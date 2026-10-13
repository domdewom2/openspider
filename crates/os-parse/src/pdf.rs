//! Dokumenteigenschaften von PDF-Dateien (ONPAGE-166).
//!
//! STUB(M1-P05)

use os_model::PdfMeta;

/// Liest Titel und Schlüsselwörter aus dem Info-Dictionary.
// STUB(M1-P05)
pub fn meta(_body: &[u8]) -> PdfMeta {
    PdfMeta::default()
}
