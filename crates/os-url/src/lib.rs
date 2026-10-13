//! `os-url` – URL-Normalisierung, Umschreibung (URL Rewriting), Include/Exclude und Segmente.

use url::Url;

/// Parst eine URL und entfernt das Fragment (`#...`).
pub fn normalize(input: &str) -> Result<Url, url::ParseError> {
    let mut url = Url::parse(input)?;
    url.set_fragment(None);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_fragment() {
        let url = normalize("https://example.com/a?x=1#top").unwrap();
        assert_eq!(url.as_str(), "https://example.com/a?x=1");
    }
}
