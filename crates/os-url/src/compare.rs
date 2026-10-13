//! URL-Vergleich für Canonical, Refresh und Indexierbarkeit (TECH-002).

use url::Url;

use crate::normalize::normalize;

/// Gleichheit zweier URLs im Sinne von TECH-002.
///
/// Verglichen wird der [`compare_key`]; ist eine der beiden URLs ungültig, sind sie nie gleich
/// (auch nicht, wenn beide dieselbe ungültige Zeichenkette sind).
pub fn eq_for_compare(a: &str, b: &str) -> bool {
    match (compare_key(a), compare_key(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Vergleichsschlüssel einer absoluten URL (`None` bei ungültiger URL).
///
/// Schema und Host klein, Standardport und Fragment entfernt, Prozent-Kodierung normalisiert;
/// Pfad und Query bleiben case-sensitiv, die Query-Reihenfolge bleibt erhalten.
pub fn compare_key(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    Some(normalize(&parsed).encoded)
}
