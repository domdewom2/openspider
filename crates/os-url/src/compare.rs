//! URL-Vergleich für Canonical, Refresh und Indexierbarkeit (TECH-002).

/// Gleichheit zweier URLs im Sinne von TECH-002.
// STUB(M1-P01): Stub vergleicht die Zeichenketten exakt.
pub fn eq_for_compare(a: &str, b: &str) -> bool {
    a == b
}

/// Vergleichsschlüssel einer URL (`None` bei ungültiger URL).
// STUB(M1-P01)
pub fn compare_key(url: &str) -> Option<String> {
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_urls_are_equal() {
        assert!(eq_for_compare("https://site.test/a", "https://site.test/a"));
    }
}
