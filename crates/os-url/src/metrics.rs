//! Kennzahlen einer URL: Ordnertiefe (A-8) und Länge in Zeichen.

use url::Url;

/// Ordnertiefe (Annahme A-8): Anzahl der nicht leeren Pfadsegmente, auf die ein `/` folgt.
///
/// `/` → 0, `/a/` → 1, `/a/b.html` → 1, `/a/b/` → 2, `/a/b` → 1. Query und Fragment zählen nicht;
/// leere Segmente (`/a//b`, `//`) zählen nicht: `/urls/a//b.html` → 2 (ONPAGE-014, K-60). Die URL bleibt
/// unverändert, die Normalisierung fasst `//` weiterhin nicht zusammen.
pub fn folder_depth(url: &Url) -> u32 {
    let path = url.path();
    // Alles bis zum letzten `/` sind Verzeichnissegmente; das Segment nach dem letzten `/` ist die Datei.
    let dirs = path.rfind('/').map_or("", |i| &path[..i]);
    let n = dirs.split('/').filter(|s| !s.is_empty()).count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Länge der Anzeigeform (`Address`) in Codepoints.
pub fn url_length(display: &str) -> u32 {
    u32::try_from(display.chars().count()).unwrap_or(u32::MAX)
}
