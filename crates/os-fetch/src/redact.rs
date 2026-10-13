//! Redaktion sensibler Werte in Protokollen und gespeicherten Headern (OUT-243).

/// Wert eines Headers; sensible Header (Authorization, Cookie …) werden maskiert.
// STUB(M1-P02): Stub maskiert die vier genannten Header; P02 ergänzt URL-Redaktion und Randfälle.
pub fn header_value(name: &str, value: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie" => {
            "[redacted]".to_string()
        }
        _ => value.to_string(),
    }
}

/// URL ohne Zugangsdaten und geheime Query-Parameter.
// STUB(M1-P02)
pub fn url(u: &str) -> String {
    u.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_is_masked() {
        assert_eq!(header_value("Authorization", "Basic abc"), "[redacted]");
        assert_eq!(header_value("Server", "x"), "x");
    }
}
