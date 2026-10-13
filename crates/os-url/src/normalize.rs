//! Normalisierung von URLs (CRAWL-001/003): konservative Form, Schlüssel und Anzeigeform.

use url::Url;

/// Fehler beim Verarbeiten einer URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    /// Die Eingabe ist keine gültige absolute URL.
    #[error("ungültige URL {0:?}")]
    Invalid(String),
    /// Noch nicht umgesetzt (Stub).
    #[error("nicht implementiert")]
    NotImplemented,
}

/// Normalisierte URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormUrl {
    /// Geparste, normalisierte URL.
    pub url: Url,
    /// URL Encoded Address (Schlüssel der Crawl-Datei).
    pub encoded: String,
    /// Anzeigeform (Address; IDN/Non-ASCII dekodiert).
    pub display: String,
    /// `scheme://host[:port]`.
    pub origin: String,
}

/// Parst eine Start-URL und normalisiert sie.
// STUB(M1-P01): P01 ergänzt Schema-Ergänzung (`https://`), Punkt am Hostende, Prozent-Normalisierung.
pub fn parse_start_url(input: &str) -> Result<NormUrl, UrlError> {
    let url = Url::parse(input.trim()).map_err(|_| UrlError::Invalid(input.to_string()))?;
    Ok(normalize(&url))
}

/// Löst einen `href` gegen die Basis-URL auf (absolut, noch nicht normalisiert).
// STUB(M1-P01)
pub fn resolve(base: &Url, href: &str) -> Option<Url> {
    base.join(href.trim()).ok()
}

/// Normalisiert eine URL (CRAWL-003).
// STUB(M1-P01): Stub entfernt nur das Fragment.
pub fn normalize(url: &Url) -> NormUrl {
    let mut url = url.clone();
    url.set_fragment(None);
    let encoded = url.as_str().to_string();
    let origin = url.origin().ascii_serialization();
    NormUrl {
        display: encoded.clone(),
        encoded,
        origin,
        url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_fragment() {
        let url = Url::parse("https://example.com/a?x=1#top").unwrap();
        let norm = normalize(&url);
        assert_eq!(norm.encoded, "https://example.com/a?x=1");
        assert_eq!(norm.origin, "https://example.com");
    }

    #[test]
    fn start_url_rejects_garbage() {
        assert!(parse_start_url("kein url").is_err());
    }
}
