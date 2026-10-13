//! Redaktion sensibler Werte in Protokollen und gespeicherten Headern (OUT-243).
//!
//! Alles, was in Logs, Fehlermeldungen oder die Crawl-Datei gelangen kann, läuft durch diese
//! Funktionen: Header-Werte ([`header_value`]), einzelne URLs ([`url`]) und Freitext mit
//! eingebetteten URLs ([`text`]). Der Platzhalter ist überall `<redacted>`.

/// Platzhalter für geschwärzte Werte.
pub const REDACTED: &str = "<redacted>";

/// Header, deren Wert nie im Klartext gespeichert oder protokolliert wird (Namen klein).
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "set-cookie2",
    "x-api-key",
    "api-key",
    "x-auth-token",
    "x-access-token",
];

/// Query-Parameter, deren Wert geschwärzt wird (Namen klein).
const SENSITIVE_PARAMS: &[&str] = &[
    "key",
    "api_key",
    "token",
    "access_token",
    "password",
    "secret",
];

/// Ob der Header-Name einen geheimen Wert trägt.
pub fn is_sensitive_header(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    SENSITIVE_HEADERS.contains(&n.as_str())
}

/// Wert eines Headers; sensible Header (`Authorization`, `Proxy-Authorization`, `Cookie`,
/// `Set-Cookie` …) werden durch `<redacted>` ersetzt.
pub fn header_value(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

/// URL ohne Zugangsdaten und geheime Query-Parameter: `https://user:pass@host/?token=x`
/// wird zu `https://<redacted>@host/?token=<redacted>`.
///
/// Arbeitet rein textuell, damit auch nicht parsebare Adressen (Fehlerfälle) geschützt sind.
pub fn url(u: &str) -> String {
    let (before_fragment, fragment) = match u.find('#') {
        Some(i) => (&u[..i], Some(&u[i..])),
        None => (u, None),
    };
    let (before_query, query) = match before_fragment.find('?') {
        Some(i) => (&before_fragment[..i], Some(&before_fragment[i + 1..])),
        None => (before_fragment, None),
    };
    let mut out = redact_userinfo(before_query);
    if let Some(q) = query {
        out.push('?');
        out.push_str(&redact_query(q));
    }
    if let Some(f) = fragment {
        out.push_str(f);
    }
    out
}

/// Freitext (z. B. Fehlermeldung): jede eingebettete `http(s)://`-Adresse wird mit [`url`] bereinigt.
pub fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let next = find_scheme(rest);
        let Some(start) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '<' | '>' | ','))
            .unwrap_or(tail.len());
        out.push_str(&url(&tail[..end]));
        rest = &tail[end..];
    }
    out
}

fn find_scheme(s: &str) -> Option<usize> {
    let lower = s.to_ascii_lowercase();
    match (lower.find("http://"), lower.find("https://")) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Ersetzt `user:pass@` im Autoritätsteil durch `<redacted>@`.
fn redact_userinfo(base: &str) -> String {
    let Some(scheme_end) = base.find("://") else {
        return base.to_string();
    };
    let auth_start = scheme_end + 3;
    let auth_end = base[auth_start..]
        .find('/')
        .map_or(base.len(), |i| auth_start + i);
    let authority = &base[auth_start..auth_end];
    match authority.rfind('@') {
        Some(at) => format!(
            "{}{}{}{}",
            &base[..auth_start],
            REDACTED,
            &authority[at..],
            &base[auth_end..]
        ),
        None => base.to_string(),
    }
}

fn redact_query(q: &str) -> String {
    q.split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if is_sensitive_param(name) => format!("{name}={REDACTED}"),
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn is_sensitive_param(name: &str) -> bool {
    let decoded = percent_decode_lossy(name).to_ascii_lowercase();
    SENSITIVE_PARAMS.contains(&decoded.as_str())
}

/// Minimale Prozent-Dekodierung nur für den Vergleich von Parameternamen.
fn percent_decode_lossy(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-243
    #[test]
    fn sensitive_headers_are_masked() {
        for name in [
            "Authorization",
            "proxy-authorization",
            "COOKIE",
            "Set-Cookie",
        ] {
            assert_eq!(header_value(name, "geheim123"), "<redacted>", "{name}");
        }
        assert_eq!(header_value("Server", "nginx"), "nginx");
        assert_eq!(header_value("Content-Type", "text/html"), "text/html");
    }

    // spec: OUT-243
    #[test]
    fn userinfo_in_url_is_masked() {
        assert_eq!(
            url("https://user:pass@example.org/a/b"),
            "https://<redacted>@example.org/a/b"
        );
        assert_eq!(
            url("http://nur-name@example.org:8080/"),
            "http://<redacted>@example.org:8080/"
        );
        assert_eq!(url("https://example.org/a@b"), "https://example.org/a@b");
    }

    // spec: OUT-243
    #[test]
    fn secret_query_params_are_masked() {
        assert_eq!(
            url("https://x.test/p?key=AAA&a=1&API_KEY=BBB&token=CCC&access_token=DDD&password=EEE&secret=FFF&keys=ok"),
            "https://x.test/p?key=<redacted>&a=1&API_KEY=<redacted>&token=<redacted>&access_token=<redacted>&password=<redacted>&secret=<redacted>&keys=ok"
        );
        assert_eq!(
            url("https://x.test/p?%74oken=zzz#frag?token=1"),
            "https://x.test/p?%74oken=<redacted>#frag?token=1"
        );
    }

    // spec: OUT-243
    #[test]
    fn text_with_embedded_urls_is_cleaned() {
        let t = text(
            "error sending request for url (https://u:p@h.test/x?token=abc): connection refused",
        );
        assert!(!t.contains("u:p"));
        assert!(!t.contains("abc"));
        assert!(t.contains("https://<redacted>@h.test/x?token=<redacted>"));
        assert!(t.ends_with("): connection refused"));
        assert_eq!(text("nichts zu tun"), "nichts zu tun");
    }

    // spec: OUT-243
    #[test]
    fn urls_without_secrets_are_unchanged() {
        for u in ["https://example.org/", "/relativ?a=b", "mailto:a@b.de", ""] {
            assert_eq!(url(u), u);
        }
    }
}
