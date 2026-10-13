//! Muster und Pfade für den Regelvergleich: Prozent-Kodierung nach RFC 9309 und Platzhalter `*`/`$`.

/// Höchstzahl ausgewerteter Bytes einer robots.txt (RFC 9309, Abschnitt 2.5).
pub(crate) const MAX_BYTES: usize = 500 * 1024;

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn push_percent(out: &mut String, b: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    out.push('%');
    out.push(char::from(HEX[usize::from(b >> 4)]));
    out.push(char::from(HEX[usize::from(b & 0x0F)]));
}

/// Bringt Pfad bzw. Muster in die Vergleichsform (RFC 9309, Abschnitt 2.2.2):
///
/// * Zeichen außerhalb von ASCII sowie Steuer-, Leer- und für URLs unzulässige Zeichen werden
///   (UTF-8-weise) prozentkodiert, Hexziffern immer großgeschrieben;
/// * `%XX` für ein nicht reserviertes ASCII-Zeichen (Buchstabe, Ziffer, `-._~`) wird dekodiert,
///   `%XX` für reservierte Zeichen bleibt kodiert (z. B. `%2F` bleibt von `/` verschieden);
/// * ein einzelnes `%` ohne zwei Hexziffern wird zu `%25`.
///
/// `*` und `$` bleiben unverändert (im Muster sind sie Platzhalter bzw. Ende-Anker).
pub(crate) fn canonical(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            let decoded = match (bytes.get(i + 1), bytes.get(i + 2)) {
                (Some(&h), Some(&l)) => hex_value(h).zip(hex_value(l)).map(|(h, l)| h * 16 + l),
                _ => None,
            };
            match decoded {
                Some(v) if is_unreserved(v) => out.push(char::from(v)),
                Some(v) => push_percent(&mut out, v),
                None => push_percent(&mut out, b'%'),
            }
            i += if decoded.is_some() { 3 } else { 1 };
            continue;
        }
        let must_encode = b >= 0x80
            || b <= 0x20
            || b == 0x7F
            || matches!(
                b,
                b'"' | b'<' | b'>' | b'\\' | b'^' | b'`' | b'{' | b'|' | b'}'
            );
        if must_encode {
            push_percent(&mut out, b);
        } else {
            out.push(char::from(b));
        }
        i += 1;
    }
    out
}

/// Ein Muster aus einer `Allow`-/`Disallow`-Zeile in Vergleichsform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pattern {
    /// Teile zwischen den `*`-Platzhaltern (mindestens einer).
    segments: Vec<String>,
    /// Muster endete auf `$`.
    anchored: bool,
    /// Länge des kanonischen Musters inklusive `*` und `$` (Vorrang bei mehreren Treffern).
    len: usize,
}

impl Pattern {
    /// Baut das Muster; leere Muster ergeben `None` (leeres `Disallow:` erlaubt alles).
    pub(crate) fn new(raw: &str) -> Option<Pattern> {
        let canon = canonical(raw);
        if canon.is_empty() {
            return None;
        }
        let len = canon.len();
        let (body, anchored) = match canon.strip_suffix('$') {
            Some(rest) => (rest, true),
            None => (canon.as_str(), false),
        };
        Some(Pattern {
            segments: body.split('*').map(str::to_owned).collect(),
            anchored,
            len,
        })
    }

    /// Länge für den Vorrangvergleich.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Passt das Muster auf den (kanonischen) Pfad inklusive Query?
    ///
    /// Das Muster ist immer am Pfadanfang verankert; ohne `$` genügt ein Präfix. Die Auswertung ist
    /// linear in Muster- und Pfadlänge (kein Backtracking), pathologische Muster sind unkritisch.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let segs = &self.segments;
        let last_idx = segs.len() - 1;
        if last_idx == 0 {
            return if self.anchored {
                path == segs[0]
            } else {
                path.starts_with(segs[0].as_str())
            };
        }
        if !path.starts_with(segs[0].as_str()) {
            return false;
        }
        let mut pos = segs[0].len();
        for seg in &segs[1..last_idx] {
            match path[pos..].find(seg.as_str()) {
                Some(i) => pos += i + seg.len(),
                None => return false,
            }
        }
        let last = segs[last_idx].as_str();
        if self.anchored {
            path.len() >= pos + last.len() && path.ends_with(last)
        } else {
            path[pos..].contains(last)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pattern: &str, path: &str) -> bool {
        Pattern::new(pattern)
            .expect("Muster")
            .matches(&canonical(path))
    }

    // spec: CRAWL-098
    #[test]
    fn canonical_form_follows_rfc_9309() {
        assert_eq!(canonical("/a b"), "/a%20b");
        assert_eq!(canonical("/ä"), "/%C3%A4");
        assert_eq!(canonical("/%c3%a4"), "/%C3%A4");
        assert_eq!(canonical("/%41bc"), "/Abc");
        assert_eq!(canonical("/a%2fb"), "/a%2Fb");
        assert_eq!(canonical("/100%"), "/100%25");
        assert_eq!(canonical("/p?a=1&b=2"), "/p?a=1&b=2");
        assert_eq!(canonical("/*.pdf$"), "/*.pdf$");
    }

    // spec: CRAWL-098
    #[test]
    fn prefix_wildcard_and_anchor() {
        assert!(m("/privat/", "/privat/x"));
        assert!(!m("/privat/", "/privat"));
        assert!(m("/*.pdf$", "/a/b.pdf"));
        assert!(m("/*.pdf$", "/a.pdf.pdf"));
        assert!(!m("/*.pdf$", "/a.pdf?x=1"));
        assert!(!m("/*.pdf$", "/a.pdfx"));
        assert!(m("/*?sessionid=", "/p?sessionid=1"));
        assert!(!m("/*?sessionid=", "/p?x=1&sessionid=1"));
        assert!(m("/seite$", "/seite"));
        assert!(!m("/seite$", "/seite2"));
        assert!(m("/seite", "/seite2"));
        assert!(m("/a*b*c$", "/aXbYc"));
        assert!(!m("/a*b*c$", "/aXbYcZ"));
        // Überlappung: der Endteil darf nicht in bereits verbrauchten Zeichen liegen.
        assert!(!m("/ab*b$", "/ab"));
        assert!(m("/ab*b$", "/abb"));
        assert!(m("/*", "/irgendwas"));
        assert!(m("/**/x", "/a/x"));
    }

    // spec: CRAWL-098
    #[test]
    fn encoded_forms_compare_equal() {
        assert!(m("/caf\u{e9}", "/caf%C3%A9"));
        assert!(m("/caf%c3%a9", "/caf\u{e9}"));
        assert!(m("/%7Euser", "/~user"));
        assert!(!m("/a/b", "/a%2Fb"));
    }

    // spec: CRAWL-098
    #[test]
    fn empty_pattern_is_none() {
        assert!(Pattern::new("").is_none());
    }

    // spec: CRAWL-098
    #[test]
    fn pathological_patterns_stay_fast() {
        let pattern = format!("/{}", "*a".repeat(2_000));
        let path = format!("/{}", "a".repeat(100_000));
        let p = Pattern::new(&pattern).expect("Muster");
        assert!(p.matches(&path));
        let miss = format!("/{}", "b".repeat(100_000));
        assert!(!p.matches(&miss));
    }
}
