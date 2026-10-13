//! Inhaltsklasse aus dem Content-Type (CRAWL-019, ONPAGE-020).

use os_model::ContentClass;

/// Ordnet einen Content-Type einer Inhaltsklasse zu; ohne Angabe und mit `assume_html` gilt HTML.
// STUB(M1-P02): Stub kennt nur HTML und XML.
pub fn content_class(content_type: Option<&str>, assume_html: bool) -> ContentClass {
    let Some(ct) = content_type else {
        return if assume_html {
            ContentClass::Html
        } else {
            ContentClass::Unknown
        };
    };
    let essence = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "text/html" | "application/xhtml+xml" => ContentClass::Html,
        "text/xml" | "application/xml" => ContentClass::Xml,
        _ => ContentClass::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_is_recognised() {
        assert_eq!(
            content_class(Some("text/html; charset=utf-8"), false),
            ContentClass::Html
        );
        assert_eq!(content_class(None, true), ContentClass::Html);
    }
}
