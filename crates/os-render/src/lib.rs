//! `os-render` – JS-Rendering über Chromium (CDP), Screenshots und gerendertes DOM.

/// Name dieses Crates (Platzhalter bis zur ersten echten Funktion).
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-render");
    }
}
