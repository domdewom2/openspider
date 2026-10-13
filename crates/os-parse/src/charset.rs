//! Zeichensatzerkennung und Dekodierung (BOM > Header > `<meta>` > UTF-8 mit Windows-1252-Rückfall).
//!
//! STUB(M1-P04)

/// Dekodiert den Body; liefert den Text und den erkannten Zeichensatz.
// STUB(M1-P04): UTF-8 verlustbehaftet, Zeichensatz unbekannt.
pub fn decode(
    body: &[u8],
    _content_type: Option<&str>,
    _headers: &[(String, String)],
) -> (String, Option<String>) {
    (String::from_utf8_lossy(body).into_owned(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8() {
        let (text, charset) = decode("Grüße".as_bytes(), None, &[]);
        assert_eq!(text, "Grüße");
        assert_eq!(charset, None);
    }
}
