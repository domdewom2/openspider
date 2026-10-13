//! Zeichensatzerkennung und Dekodierung (CRAWL-021).
//!
//! Reihenfolge nach WHATWG-Encoding-Standard: BOM > `charset` im `Content-Type` > `<meta charset>` bzw.
//! `<meta http-equiv="Content-Type">` in den ersten 1024 Bytes > UTF-8, bei ungültigen Sequenzen
//! Windows-1252 (OS-Festlegung). Ein unbekannter Name auf einer Stufe führt zur nächsten Stufe; Labels
//! werden über die WHATWG-Aliastabelle von `encoding_rs` aufgelöst (`latin1` → windows-1252).
//! Ungültige Bytes einer erklärten Kodierung werden durch U+FFFD ersetzt, nie ein Fehler.

use encoding_rs::{Encoding, REPLACEMENT, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252, X_USER_DEFINED};

/// So viele Bytes am Dokumentanfang werden nach einer `<meta>`-Angabe durchsucht.
const META_PRESCAN_BYTES: usize = 1024;

/// Dekodiert den Body; liefert den Text und den erkannten Zeichensatz (WHATWG-Name, z. B. `UTF-8`,
/// `windows-1252`, `Shift_JIS`).
pub fn decode(
    body: &[u8],
    content_type: Option<&str>,
    headers: &[(String, String)],
) -> (String, Option<String>) {
    // 1. BOM
    if let Some((encoding, _)) = Encoding::for_bom(body) {
        let (text, _) = encoding.decode_with_bom_removal(body);
        return (text.into_owned(), Some(encoding.name().to_string()));
    }

    // 2. Header
    let header_ct = content_type.or_else(|| header_value(headers, "content-type"));
    if let Some(encoding) = header_ct
        .and_then(charset_param)
        .and_then(|label| label_to_encoding(&label))
    {
        return finish(body, encoding);
    }

    // 3. <meta> in den ersten 1024 Bytes
    let head = &body[..body.len().min(META_PRESCAN_BYTES)];
    if let Some(encoding) = prescan_meta(head) {
        return finish(body, encoding);
    }

    // 4. UTF-8, sonst Windows-1252
    let encoding = if looks_like_utf8(body) {
        UTF_8
    } else {
        WINDOWS_1252
    };
    finish(body, encoding)
}

/// Dekodiert einen Textbody ohne Header und ohne `<meta>`-Suche (BOM > UTF-8 mit Windows-1252-Rückfall),
/// z. B. für CSS-Dateien.
pub(crate) fn decode_plain(body: &[u8]) -> String {
    decode_without_meta(body).0
}

fn decode_without_meta(body: &[u8]) -> (String, &'static Encoding) {
    if let Some((encoding, _)) = Encoding::for_bom(body) {
        let (text, _) = encoding.decode_with_bom_removal(body);
        return (text.into_owned(), encoding);
    }
    let encoding = if looks_like_utf8(body) {
        UTF_8
    } else {
        WINDOWS_1252
    };
    let (text, _) = encoding.decode_without_bom_handling(body);
    (text.into_owned(), encoding)
}

fn finish(body: &[u8], encoding: &'static Encoding) -> (String, Option<String>) {
    let (text, _) = encoding.decode_without_bom_handling(body);
    (text.into_owned(), Some(encoding.name().to_string()))
}

/// Ist der Body gültiges UTF-8? Eine abgeschnittene Sequenz ganz am Ende (z. B. durch das Größenlimit)
/// gilt noch als gültig.
fn looks_like_utf8(body: &[u8]) -> bool {
    match std::str::from_utf8(body) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none(),
    }
}

/// Wert des ersten Headers mit diesem Namen (ASCII-case-insensitiv).
fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Wert des `charset`-Parameters eines `Content-Type`-Werts.
fn charset_param(content_type: &str) -> Option<String> {
    let mut rest = content_type;
    // Parameter nach dem Medientyp; Anführungszeichen schützen Semikola.
    let first_semicolon = rest.find(';')?;
    rest = &rest[first_semicolon + 1..];
    let mut params: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in rest.chars() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    current.push(c);
                }
                ';' => params.push(std::mem::take(&mut current)),
                _ => current.push(c),
            },
        }
    }
    params.push(current);
    for param in params {
        let Some((name, value)) = param.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("charset") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Löst ein Label über die WHATWG-Aliastabelle auf. Das Ersatz-„Encoding“ (`replacement`, z. B. für
/// `iso-2022-kr`) zählt als unbekannt.
fn label_to_encoding(label: &str) -> Option<&'static Encoding> {
    let encoding = Encoding::for_label(label.trim().as_bytes())?;
    (encoding != REPLACEMENT).then_some(encoding)
}

fn is_space(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

/// `<meta>`-Vorabsuche nach dem Algorithmus „prescan a byte stream“ des HTML-Standards.
fn prescan_meta(bytes: &[u8]) -> Option<&'static Encoding> {
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
        if bytes[i..].starts_with(b"<!--") {
            // `<!-->` und `<!--->` sind bereits vollständige Kommentare.
            let p = find(&bytes[i + 2..], b"-->")?;
            i = i + 2 + p + 3;
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &bytes[i + 1..];
        let is_meta = rest.len() > 4
            && rest[..4].eq_ignore_ascii_case(b"meta")
            && (is_space(rest[4]) || rest[4] == b'/');
        if is_meta {
            let mut pos = i + 1 + 4;
            let mut seen: Vec<Vec<u8>> = Vec::new();
            let mut got_pragma = false;
            let mut need_pragma: Option<bool> = None;
            let mut charset: Option<&'static Encoding> = None;
            while let Some((name, value)) = next_attribute(bytes, &mut pos) {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name.clone());
                match name.as_slice() {
                    b"http-equiv" => {
                        if value == b"content-type" {
                            got_pragma = true;
                        }
                    }
                    b"content" => {
                        if charset.is_none() {
                            if let Some(label) = charset_from_meta_content(&value) {
                                if let Some(enc) = meta_encoding(&label) {
                                    charset = Some(enc);
                                    need_pragma = Some(true);
                                }
                            }
                        }
                    }
                    b"charset" => {
                        let label = String::from_utf8_lossy(&value).into_owned();
                        if let Some(enc) = meta_encoding(&label) {
                            charset = Some(enc);
                            need_pragma = Some(false);
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(enc), Some(need)) = (charset, need_pragma) {
                if !need || got_pragma {
                    return Some(enc);
                }
            }
            i = pos.max(i + 1);
            continue;
        }
        let tag_like = rest.first().is_some_and(u8::is_ascii_alphabetic)
            || (rest.first() == Some(&b'/') && rest.get(1).is_some_and(u8::is_ascii_alphabetic));
        if tag_like {
            // Tag-Namen überspringen, dann alle Attribute (Anführungszeichen beachten).
            let mut pos = i + 1;
            while pos < n && !is_space(bytes[pos]) && bytes[pos] != b'>' {
                pos += 1;
            }
            while next_attribute(bytes, &mut pos).is_some() {}
            i = pos.max(i + 1);
            continue;
        }
        if matches!(rest.first(), Some(b'!') | Some(b'/') | Some(b'?')) {
            let p = bytes[i..].iter().position(|b| *b == b'>')?;
            i += p + 1;
            continue;
        }
        i += 1;
    }
    None
}

/// Kodierung aus einer `<meta>`-Angabe: UTF-16 ist hier nicht möglich (die Bytes waren ASCII-lesbar) und
/// wird zu UTF-8; `x-user-defined` wird zu windows-1252; unbekannte Namen ergeben `None`.
fn meta_encoding(label: &str) -> Option<&'static Encoding> {
    let encoding = label_to_encoding(label)?;
    Some(if encoding == UTF_16LE || encoding == UTF_16BE {
        UTF_8
    } else if encoding == X_USER_DEFINED {
        WINDOWS_1252
    } else {
        encoding
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Liest das nächste Attribut (Name und Wert in Kleinbuchstaben). `None` am Ende des Tags (`>`) oder des
/// Puffers; `pos` steht dann hinter dem `>`.
fn next_attribute(bytes: &[u8], pos: &mut usize) -> Option<(Vec<u8>, Vec<u8>)> {
    let n = bytes.len();
    while *pos < n && (is_space(bytes[*pos]) || bytes[*pos] == b'/') {
        *pos += 1;
    }
    if *pos >= n {
        return None;
    }
    if bytes[*pos] == b'>' {
        *pos += 1;
        return None;
    }
    let mut name = Vec::new();
    while *pos < n {
        let b = bytes[*pos];
        if b == b'=' && !name.is_empty() {
            break;
        }
        if is_space(b) || b == b'/' || b == b'>' {
            break;
        }
        name.push(b.to_ascii_lowercase());
        *pos += 1;
    }
    while *pos < n && is_space(bytes[*pos]) {
        *pos += 1;
    }
    if *pos >= n || bytes[*pos] != b'=' {
        return Some((name, Vec::new()));
    }
    *pos += 1;
    while *pos < n && is_space(bytes[*pos]) {
        *pos += 1;
    }
    let mut value = Vec::new();
    if *pos < n && (bytes[*pos] == b'"' || bytes[*pos] == b'\'') {
        let quote = bytes[*pos];
        *pos += 1;
        while *pos < n && bytes[*pos] != quote {
            value.push(bytes[*pos].to_ascii_lowercase());
            *pos += 1;
        }
        *pos += 1; // schließendes Anführungszeichen
    } else {
        while *pos < n && !is_space(bytes[*pos]) && bytes[*pos] != b'>' {
            value.push(bytes[*pos].to_ascii_lowercase());
            *pos += 1;
        }
    }
    Some((name, value))
}

/// „Algorithm for extracting a character encoding from a meta element“ (HTML-Standard).
fn charset_from_meta_content(value: &[u8]) -> Option<String> {
    let n = value.len();
    let mut pos = 0usize;
    loop {
        let found = find(&value[pos..], b"charset")?;
        pos += found + b"charset".len();
        while pos < n && is_space(value[pos]) {
            pos += 1;
        }
        if pos >= n || value[pos] != b'=' {
            continue;
        }
        pos += 1;
        while pos < n && is_space(value[pos]) {
            pos += 1;
        }
        if pos >= n {
            return None;
        }
        let label: &[u8] = if value[pos] == b'"' || value[pos] == b'\'' {
            let quote = value[pos];
            let rest = &value[pos + 1..];
            let end = rest.iter().position(|b| *b == quote)?;
            &rest[..end]
        } else {
            let rest = &value[pos..];
            let end = rest
                .iter()
                .position(|b| is_space(*b) || *b == b';')
                .unwrap_or(rest.len());
            &rest[..end]
        };
        return Some(String::from_utf8_lossy(label).into_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8() {
        let (text, charset) = decode("Grüße".as_bytes(), None, &[]);
        assert_eq!(text, "Grüße");
        assert_eq!(charset.as_deref(), Some("UTF-8"));
    }

    #[test]
    fn content_type_params() {
        assert_eq!(
            charset_param("text/html; charset=UTF-8").as_deref(),
            Some("UTF-8")
        );
        assert_eq!(
            charset_param("text/html;charset=\"iso-8859-1\"; x=y").as_deref(),
            Some("iso-8859-1")
        );
        assert_eq!(charset_param("text/html"), None);
        assert_eq!(charset_param("text/html; boundary=\"a;charset=x\""), None);
    }

    #[test]
    fn meta_prescan_variants() {
        let enc = |s: &str| prescan_meta(s.as_bytes()).map(Encoding::name);
        assert_eq!(enc("<meta charset=utf-8>"), Some("UTF-8"));
        assert_eq!(enc("<META CHARSET='Shift_JIS'>"), Some("Shift_JIS"));
        assert_eq!(
            enc("<meta http-equiv=\"Content-Type\" content=\"text/html; charset=latin1\">"),
            Some("windows-1252")
        );
        // content ohne http-equiv zählt nicht
        assert_eq!(enc("<meta content=\"text/html; charset=latin1\">"), None);
        // Kommentar wird übersprungen
        assert_eq!(
            enc("<!-- <meta charset=koi8-r> --><meta charset=gbk>"),
            Some("GBK")
        );
        // UTF-16 per meta ist UTF-8
        assert_eq!(enc("<meta charset=utf-16le>"), Some("UTF-8"));
        // unbekannt: nächstes meta
        assert_eq!(
            enc("<meta charset=foo><meta charset=euc-kr>"),
            Some("EUC-KR")
        );
        assert_eq!(enc("<meta charset=foo>"), None);
    }

    #[test]
    fn truncated_utf8_tail_is_still_utf8() {
        // Das Größenlimit kann ein Mehrbytezeichen am Ende abschneiden: weiterhin UTF-8, nicht Windows-1252.
        let mut body = "ok ü".as_bytes().to_vec();
        body.pop();
        let (text, charset) = decode(&body, None, &[]);
        assert_eq!(charset.as_deref(), Some("UTF-8"));
        assert!(text.starts_with("ok "));
        // Ein ungültiges Byte mitten im Text führt dagegen zum Windows-1252-Rückfall.
        let (text, charset) = decode(b"Gr\xfc\xdfe", None, &[]);
        assert_eq!(charset.as_deref(), Some("windows-1252"));
        assert_eq!(text, "Grüße");
    }
}
