//! Normalisierung von URLs (CRAWL-001/003): konservative Form, Schlüssel und Anzeigeform.
//!
//! Die Normalisierung ändert nichts, was ein Server anders interpretieren könnte: Schema und
//! Host werden kleingeschrieben, Standardports und ein Punkt am Hostende entfallen, unreservierte
//! Zeichen in `%XX` werden dekodiert, alle übrigen Escapes erhalten Großbuchstaben-Hex. Pfad
//! (auch `//`), Groß-/Kleinschreibung, Query-Reihenfolge und eine leere Query bleiben erhalten.

use std::fmt::Write as _;

use url::{Host, Url};

/// Fehler beim Verarbeiten einer URL.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    /// Die Eingabe ist keine gültige absolute URL.
    #[error("ungültige URL {0:?}")]
    Invalid(String),
    /// Reserviert für noch nicht umgesetzte Funktionen; wird von `os-url` nicht mehr erzeugt.
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

/// Parst eine Start-URL und normalisiert sie (CRAWL-001).
///
/// Führende und abschließende Leerzeichen entfallen. Fehlt das Schema, wird `http://` ergänzt
/// (`site.test` → `http://site.test/`); `host:port` gilt ebenfalls als Eingabe ohne Schema.
/// Abgewiesen werden alle Eingaben, die kein `http`/`https` mit nicht leerem Host ergeben.
pub fn parse_start_url(input: &str) -> Result<NormUrl, UrlError> {
    let invalid = || UrlError::Invalid(input.to_string());
    let text = input.trim();
    if text.is_empty() {
        return Err(invalid());
    }
    let with_scheme = match split_scheme(text) {
        SchemeSplit::Has => text.to_string(),
        SchemeSplit::Missing => format!("http://{}", text.trim_start_matches('/')),
        SchemeSplit::Broken => return Err(invalid()),
    };
    // Leere Authority (`http:///x`) behandelt der URL-Standard als Host `x`; hier ist sie ungültig.
    let authority = with_scheme
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""))
        .unwrap_or("");
    if authority.is_empty() {
        return Err(invalid());
    }
    let url = Url::parse(&with_scheme).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid());
    }
    match url.host_str() {
        Some(host) if !host.is_empty() => {}
        _ => return Err(invalid()),
    }
    Ok(normalize(&url))
}

enum SchemeSplit {
    /// `scheme://…`
    Has,
    /// Kein Schema (auch `host:port`, `[::1]:80`, `//host`).
    Missing,
    /// `scheme:` ohne `//` (z. B. `htp:/x`, `mailto:a@b`).
    Broken,
}

fn split_scheme(text: &str) -> SchemeSplit {
    if text.starts_with('[') || text.starts_with("//") {
        return SchemeSplit::Missing;
    }
    let Some(colon) = text.find(':') else {
        return SchemeSplit::Missing;
    };
    let (head, tail) = (&text[..colon], &text[colon + 1..]);
    let scheme_like = !head.is_empty()
        && head.starts_with(|c: char| c.is_ascii_alphabetic())
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if scheme_like && tail.starts_with("//") {
        return SchemeSplit::Has;
    }
    // `host:8080/pfad` – Doppelpunkt vor einer Portnummer ist kein Schema.
    let port_end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
    let port = &tail[..port_end];
    if !head.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
        return SchemeSplit::Missing;
    }
    SchemeSplit::Broken
}

/// Löst einen `href` gegen die Basis-URL auf (absolut, noch nicht normalisiert; CRAWL-002).
///
/// Der URL-Standard entfernt dabei Leerraum an den Rändern sowie Tabs und Zeilenumbrüche im
/// Wert; ein leerer `href` ergibt die Basis-URL (TECH-002).
pub fn resolve(base: &Url, href: &str) -> Option<Url> {
    base.join(href).ok()
}

/// Normalisiert eine URL (CRAWL-003).
pub fn normalize(url: &Url) -> NormUrl {
    let mut url = url.clone();
    url.set_fragment(None);

    if let Some(host) = url.host_str() {
        let trimmed = host.trim_end_matches('.');
        if trimmed.len() != host.len() && !trimmed.is_empty() {
            let trimmed = trimmed.to_string();
            let _ = url.set_host(Some(&trimmed));
        }
    }

    if url.has_host() || !url.cannot_be_a_base() {
        let path = normalize_percent(url.path());
        if path != url.path() {
            url.set_path(&path);
        }
    }
    if let Some(query) = url.query() {
        let query = normalize_percent(query);
        if Some(query.as_str()) != url.query() {
            url.set_query(Some(&query));
        }
    }

    let encoded = url.as_str().to_string();
    let display = display_form(&url);
    let origin = match url.host_str() {
        Some(host) => {
            let mut origin = format!("{}://{}", url.scheme(), host);
            if let Some(port) = url.port() {
                let _ = write!(origin, ":{port}");
            }
            origin
        }
        None => url.origin().ascii_serialization(),
    };
    NormUrl {
        url,
        encoded,
        display,
        origin,
    }
}

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Dekodiert unreservierte `%XX`, schreibt die übrigen Escapes mit Großbuchstaben-Hex.
/// Ein `%` ohne zwei Hexziffern bleibt unverändert; es wird nie doppelt kodiert.
fn normalize_percent(s: &str) -> String {
    const UPPER: &[u8; 16] = b"0123456789ABCDEF";
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            if let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex_val),
                bytes.get(i + 2).copied().and_then(hex_val),
            ) {
                let v = h * 16 + l;
                if is_unreserved(v) {
                    out.push(v as char);
                } else {
                    out.push('%');
                    out.push(UPPER[usize::from(h)] as char);
                    out.push(UPPER[usize::from(l)] as char);
                }
                i += 3;
                continue;
            }
        }
        // Mehrbyte-Zeichen unverändert übernehmen (die URL ist hier normalerweise ASCII).
        let ch_len = s[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Anzeigeform: IDN-Host als Unicode, kodierte Nicht-ASCII-Zeichen in Pfad/Query dekodiert.
/// Kodierte ASCII-Zeichen (z. B. `%20`) bleiben, damit die Anzeige eindeutig bleibt.
fn display_form(url: &Url) -> String {
    let Some(host) = url.host() else {
        return decode_non_ascii(url.as_str());
    };
    let mut out = String::new();
    out.push_str(url.scheme());
    out.push_str("://");
    if !url.username().is_empty() || url.password().is_some() {
        out.push_str(url.username());
        if let Some(password) = url.password() {
            out.push(':');
            out.push_str(password);
        }
        out.push('@');
    }
    match host {
        Host::Domain(domain) => {
            let (unicode, result) = idna::domain_to_unicode(domain);
            out.push_str(if result.is_ok() { &unicode } else { domain });
        }
        Host::Ipv4(_) | Host::Ipv6(_) => out.push_str(url.host_str().unwrap_or("")),
    }
    if let Some(port) = url.port() {
        let _ = write!(out, ":{port}");
    }
    out.push_str(&decode_non_ascii(url.path()));
    if let Some(query) = url.query() {
        out.push('?');
        out.push_str(&decode_non_ascii(query));
    }
    out
}

fn push_escaped(out: &mut String, bytes: &[u8]) {
    for b in bytes {
        let _ = write!(out, "%{b:02X}");
    }
}

fn displayable(c: char) -> bool {
    !(c.is_control()
        || c.is_whitespace()
        || c == '\u{FFFD}'
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'))
}

fn decode_non_ascii(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        // Lauf aufeinanderfolgender `%XX` mit Wert >= 0x80 einsammeln.
        let mut run: Vec<u8> = Vec::new();
        let mut j = i;
        while bytes.get(j) == Some(&b'%') {
            let (Some(h), Some(l)) = (
                bytes.get(j + 1).copied().and_then(hex_val),
                bytes.get(j + 2).copied().and_then(hex_val),
            ) else {
                break;
            };
            let v = h * 16 + l;
            if v < 0x80 {
                break;
            }
            run.push(v);
            j += 3;
        }
        if run.is_empty() {
            let ch_len = s[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&s[i..i + ch_len]);
            i += ch_len;
            continue;
        }
        let mut rest = run.as_slice();
        while !rest.is_empty() {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    push_chars(&mut out, text);
                    break;
                }
                Err(err) => {
                    let (valid, after) = rest.split_at(err.valid_up_to());
                    // Gültiger Präfix ist per Definition UTF-8.
                    push_chars(&mut out, std::str::from_utf8(valid).unwrap_or(""));
                    let bad = err.error_len().unwrap_or(after.len());
                    push_escaped(&mut out, &after[..bad]);
                    rest = &after[bad..];
                }
            }
        }
        i = j;
    }
    out
}

fn push_chars(out: &mut String, text: &str) {
    for c in text.chars() {
        if displayable(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            push_escaped(out, c.encode_utf8(&mut buf).as_bytes());
        }
    }
}
