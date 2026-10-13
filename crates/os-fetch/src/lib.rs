//! `os-fetch` – HTTP-Client, robots.txt, Rate-Limit, Authentifizierung, Proxy, Cookies, HTTP/2.

/// Name dieses Crates (Platzhalter bis zur ersten echten Funktion).
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches() {
        assert_eq!(NAME, "os-fetch");
    }
}
