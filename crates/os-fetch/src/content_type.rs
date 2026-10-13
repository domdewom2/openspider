//! Inhaltsklasse aus dem Content-Type (CRAWL-019, ONPAGE-020).
//!
//! Die Zuordnung folgt der Tabelle aus ONPAGE-020: Parameter (`; charset=…`) werden ignoriert, der
//! Vergleich ist case-insensitiv und der Wert wird getrimmt. Kein Content-Sniffing am Body.

use os_model::ContentClass;

/// Ordnet einen Content-Type einer Inhaltsklasse zu.
///
/// - Fehlender oder leerer Header: `Unknown`, mit `assume_html` („Assume pages are HTML“) `Html`.
/// - Syntaktisch ungültiger Wert (z. B. `foo` ohne `/`): `Unknown`.
/// - Jeder andere gültige MIME-Typ, der in keiner Klasse vorkommt: `Other`.
pub fn content_class(content_type: Option<&str>, assume_html: bool) -> ContentClass {
    let essence = content_type
        .map(|ct| {
            ct.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();
    if essence.is_empty() {
        return if assume_html {
            ContentClass::Html
        } else {
            ContentClass::Unknown
        };
    }
    let Some((top, sub)) = split_mime(&essence) else {
        return ContentClass::Unknown;
    };
    match essence.as_str() {
        "text/html" | "application/xhtml+xml" => ContentClass::Html,
        "application/javascript"
        | "text/javascript"
        | "application/x-javascript"
        | "application/ecmascript"
        | "text/ecmascript"
        | "application/x-ecmascript"
        | "text/x-javascript" => ContentClass::JavaScript,
        "text/css" => ContentClass::Css,
        "application/pdf" => ContentClass::Pdf,
        "application/x-shockwave-flash" | "application/futuresplash" => ContentClass::Plugins,
        "application/vnd.ms-fontobject" => ContentClass::Fonts,
        "application/xml" | "text/xml" | "application/rss+xml" | "application/atom+xml" => {
            ContentClass::Xml
        }
        _ => match top {
            "image" => ContentClass::Images,
            "video" | "audio" => ContentClass::Media,
            "font" => ContentClass::Fonts,
            // `application/font-woff`, `application/font-woff2`, `application/x-font-ttf` …
            "application" if sub.starts_with("font-") || sub.starts_with("x-font-") => {
                ContentClass::Fonts
            }
            _ => ContentClass::Other,
        },
    }
}

/// Zerlegt `type/subtype`; `None`, wenn der Wert syntaktisch kein MIME-Typ ist (RFC 6838-Token).
fn split_mime(essence: &str) -> Option<(&str, &str)> {
    let (top, sub) = essence.split_once('/')?;
    let valid = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(
                        b,
                        b'!' | b'#'
                            | b'$'
                            | b'&'
                            | b'^'
                            | b'_'
                            | b'.'
                            | b'+'
                            | b'-'
                            | b'*'
                            | b'\''
                            | b'`'
                            | b'|'
                            | b'~'
                            | b'%'
                    )
            })
    };
    (valid(top) && valid(sub)).then_some((top, sub))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ContentClass::*;

    fn cc(ct: &str) -> ContentClass {
        content_class(Some(ct), false)
    }

    // spec: ONPAGE-020, CRAWL-019
    #[test]
    fn html_with_parameters_and_case() {
        assert_eq!(cc("text/html; charset=UTF-8"), Html);
        assert_eq!(cc("TEXT/HTML"), Html);
        assert_eq!(cc("Text/HTML; Charset=UTF-8"), Html);
        assert_eq!(cc("  text/html  ;charset=x"), Html);
        assert_eq!(cc("application/xhtml+xml"), Html);
    }

    // spec: ONPAGE-020
    #[test]
    fn every_row_of_the_class_table() {
        for ct in [
            "application/javascript",
            "text/javascript",
            "application/x-javascript",
            "application/ecmascript",
            "text/ecmascript",
            "application/x-ecmascript",
            "text/x-javascript",
        ] {
            assert_eq!(cc(ct), JavaScript, "{ct}");
        }
        assert_eq!(cc("text/css"), Css);
        for ct in ["image/png", "image/webp", "image/avif", "image/jpeg"] {
            assert_eq!(cc(ct), Images, "{ct}");
        }
        assert_eq!(cc("application/pdf"), Pdf);
        assert_eq!(cc("application/x-shockwave-flash"), Plugins);
        assert_eq!(cc("application/futuresplash"), Plugins);
        for ct in ["video/mp4", "audio/mpeg", "video/webm", "audio/ogg"] {
            assert_eq!(cc(ct), Media, "{ct}");
        }
        for ct in [
            "font/woff2",
            "font/ttf",
            "application/font-woff",
            "application/font-woff2",
            "application/x-font-ttf",
            "application/x-font-otf",
            "application/vnd.ms-fontobject",
        ] {
            assert_eq!(cc(ct), Fonts, "{ct}");
        }
        for ct in [
            "application/xml",
            "text/xml",
            "application/rss+xml",
            "application/atom+xml",
        ] {
            assert_eq!(cc(ct), Xml, "{ct}");
        }
        for ct in [
            "text/plain",
            "application/json",
            "application/zip",
            "application/msword",
            "application/octet-stream",
        ] {
            assert_eq!(cc(ct), Other, "{ct}");
        }
    }

    // spec: ONPAGE-020
    #[test]
    fn svg_is_images_not_xml_and_json_is_other() {
        assert_eq!(cc("image/svg+xml"), Images);
        assert_eq!(cc("image/svg+xml; charset=utf-8"), Images);
        assert_eq!(cc("application/json"), Other);
    }

    // spec: ONPAGE-020, CRAWL-019
    #[test]
    fn missing_empty_and_invalid_are_unknown() {
        assert_eq!(content_class(None, false), Unknown);
        assert_eq!(content_class(Some(""), false), Unknown);
        assert_eq!(content_class(Some("   "), false), Unknown);
        assert_eq!(cc("foo"), Unknown);
        assert_eq!(cc("text/"), Unknown);
        assert_eq!(cc("/html"), Unknown);
        assert_eq!(cc("te xt/html"), Unknown);
        assert_eq!(cc("a/b/c"), Unknown);
    }

    // spec: ONPAGE-020
    #[test]
    fn assume_html_only_applies_without_content_type() {
        assert_eq!(content_class(None, true), Html);
        assert_eq!(content_class(Some(""), true), Html);
        // ein vorhandener, aber ungültiger oder fremder Wert bleibt, wie er ist
        assert_eq!(content_class(Some("foo"), true), Unknown);
        assert_eq!(content_class(Some("text/plain"), true), Other);
    }
}
