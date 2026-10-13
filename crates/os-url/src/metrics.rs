//! Kennzahlen einer URL: Ordnertiefe (A-8) und Länge in Zeichen.

use url::Url;

/// Ordnertiefe (Annahme A-8): Anzahl der Pfadsegmente, auf die ein `/` folgt.
///
/// `/` → 0, `/a/` → 1, `/a/b.html` → 1, `/a/b/` → 2, `/a/b` → 1. Query und Fragment zählen nicht;
/// ein leeres Segment (`/a//b`) zählt wie jedes andere (die Normalisierung fasst `//` nicht zusammen).
pub fn folder_depth(url: &Url) -> u32 {
    let slashes = url.path().bytes().filter(|&b| b == b'/').count();
    u32::try_from(slashes.saturating_sub(1)).unwrap_or(u32::MAX)
}

/// Länge der Anzeigeform (`Address`) in Codepoints.
pub fn url_length(display: &str) -> u32 {
    u32::try_from(display.chars().count()).unwrap_or(u32::MAX)
}
