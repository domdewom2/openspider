//! Kennzahlen einer URL: Ordnertiefe (A-8) und Länge in Zeichen.

use url::Url;

/// Anzahl der Pfadsegmente vor dem letzten (Ordnertiefe).
// STUB(M1-P01)
pub fn folder_depth(url: &Url) -> u32 {
    let segments = url.path().split('/').filter(|s| !s.is_empty()).count() as u32;
    segments.saturating_sub(1)
}

/// Länge der Anzeigeform in Codepoints.
// STUB(M1-P01)
pub fn url_length(display: &str) -> u32 {
    display.chars().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_codepoints() {
        assert_eq!(url_length("a/ü"), 3);
    }
}
