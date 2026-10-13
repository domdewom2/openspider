//! `os-parse` – HTML-Extraktion: Links, Meta, Headings, Canonicals, hreflang, Bilder, Directives,
//! strukturierte Daten, Custom Search/Extraction (Streaming-Parser, DOM nur bei Bedarf).

/// Name dieses Crates (Platzhalter bis zur ersten echten Funktion).
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-parse");
    }
}
