//! `os-analyze` – Filterlogik je Tab, Issues und Nachlauf-Analyse (Link Score, Duplikate per
//! SimHash/MinHash, Orphans, Ketten, hreflang-Rücklinks, Sitemaps).

/// Name dieses Crates (Platzhalter bis zur ersten echten Funktion).
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-analyze");
    }
}
