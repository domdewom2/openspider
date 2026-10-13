//! Pixelbreiten-Engine (UI-038): Schriftmetriken von Liberation Sans/Arimo, Regular und Bold.
//!
//! STUB(M1-P05)

use os_model::ExtractedText;

/// Breite eines Textes in Pixeln bei gegebener Schriftgröße.
// STUB(M1-P05): 0
pub fn width_px(_text: &str, _size_px: f64, _bold: bool) -> u32 {
    0
}

/// Setzt `ExtractedText.px` für alle Texte.
// STUB(M1-P05): lässt `px` unverändert.
pub fn annotate(_texts: &mut Vec<ExtractedText>, _size_px: f64, _bold: bool) {}
