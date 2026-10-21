//! Basic-Auth: Zuordnung von URL-Präfixen zu Zugangsdaten (CRAWL-117).
//!
//! Zugangsdaten gehen nur an Anfragen, deren Ziel zum Präfix eines Eintrags passt:
//!
//! - Der **Host** muss übereinstimmen (kein Suffix-, kein Teilstring-Treffer). Weil der Fetcher keine
//!   Weiterleitungen verfolgt, bewertet jede Anfrage ihr eigenes Ziel: Nach einem Wechsel auf einen
//!   fremden Host (oder ein anderes Schema/einen anderen Port) fehlt der Header automatisch.
//! - Ein Präfix mit **Port** gilt nur für diesen Port, ohne Port für jeden.
//! - Der **Pfad** des Präfixes gilt an Segmentgrenzen (`/admin` trifft `/admin/x`, nicht `/administrator`).
//! - Über unverschlüsseltes **http** werden Zugangsdaten nur gesendet, wenn das Präfix ausdrücklich mit
//!   `http://` angegeben ist (das ist die Bestätigung des Nutzers, OS-Festlegung zu CRAWL-117).
//!   Präfixe ohne Schema (`host`, `host/pfad`) und mit `https://` gelten nur für https.
//!
//! Benutzername und Passwort werden UTF-8-kodiert (Referenz-Fix 20.3).

pub(crate) mod digest;
use url::Url;

use crate::client::BasicAuthEntry;

#[derive(Clone)]
struct Entry {
    scheme: Option<String>,
    host: String,
    port: Option<u16>,
    path: String,
    header: String,
    credentials: BasicAuthEntry,
}

/// Ordnet Anfrage-URLs den passenden Basic-Auth-Zugangsdaten zu.
#[derive(Clone, Default)]
pub struct AuthMatcher {
    entries: Vec<Entry>,
}

// Kein `Debug`-Ableitung: der Header enthält das Passwort (kodiert).
impl std::fmt::Debug for AuthMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthMatcher")
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl AuthMatcher {
    /// Baut den Matcher; ungültige Präfixe oder Benutzernamen mit `:` sind Konfigurationsfehler.
    pub fn new(entries: &[BasicAuthEntry]) -> Result<Self, String> {
        let mut out = Vec::with_capacity(entries.len());
        for e in entries {
            if e.username.contains(':') {
                return Err(format!(
                    "Basic-Auth für {:?}: Benutzername darf kein ':' enthalten (RFC 7617)",
                    crate::redact::url(&e.prefix)
                ));
            }
            let (scheme, host, port, path) = parse_prefix(&e.prefix)?;
            out.push(Entry {
                credentials: e.clone(),
                scheme,
                host,
                port,
                path,
                header: format!(
                    "Basic {}",
                    base64_encode(format!("{}:{}", e.username, e.password).as_bytes())
                ),
            });
        }
        Ok(AuthMatcher { entries: out })
    }

    /// Frozen credentials for a browser challenge, subject to the same URL matching.
    pub fn rendering_credentials(&self, url: &Url) -> Option<BasicAuthEntry> {
        self.credentials_for(url).cloned()
    }

    pub(crate) fn credentials_for(&self, url: &Url) -> Option<&BasicAuthEntry> {
        let header = self.authorization_for(url)?;
        self.entries
            .iter()
            .filter(|e| e.header == header)
            .filter(|e| {
                url.host_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&e.host))
                    && path_matches(&e.path, url.path())
            })
            .max_by_key(|e| e.path.len())
            .map(|e| &e.credentials)
    }

    /// Ob mindestens ein Eintrag konfiguriert ist.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Wert des `Authorization`-Headers für `url`, falls ein Eintrag passt (längster Pfad gewinnt).
    pub fn authorization_for(&self, url: &Url) -> Option<&str> {
        let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
        let is_http = url.scheme() == "http";
        self.entries
            .iter()
            .filter(|e| {
                e.host == host
                    && match &e.scheme {
                        Some(s) => s == url.scheme(),
                        None => !is_http,
                    }
                    && e.port
                        .is_none_or(|p| url.port_or_known_default() == Some(p))
                    && path_matches(&e.path, url.path())
            })
            .max_by_key(|e| e.path.len())
            .map(|e| e.header.as_str())
    }
}

type ParsedPrefix = (Option<String>, String, Option<u16>, String);

fn parse_prefix(prefix: &str) -> Result<ParsedPrefix, String> {
    let shown = crate::redact::url(prefix);
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return Err("Basic-Auth: leeres URL-Präfix".to_string());
    }
    let (scheme, to_parse) = if let Some((s, _)) = trimmed.split_once("://") {
        let s = s.to_ascii_lowercase();
        if s != "http" && s != "https" {
            return Err(format!(
                "Basic-Auth-Präfix {shown:?}: nur http und https erlaubt"
            ));
        }
        (Some(s), trimmed.to_string())
    } else {
        (None, format!("https://{trimmed}"))
    };
    let url = Url::parse(&to_parse).map_err(|e| format!("Basic-Auth-Präfix {shown:?}: {e}"))?;
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| format!("Basic-Auth-Präfix {shown:?}: Host fehlt"))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    // `Url::port` kennt den Standardport nicht: ein ausdrücklich genannter Standardport zählt als Port.
    let port = url.port().or_else(|| explicit_default_port(trimmed));
    Ok((scheme, host, port, url.path().to_string()))
}

/// `https://host:443/` – die url-Crate verwirft den Standardport; hier wird er wieder erkannt.
fn explicit_default_port(prefix: &str) -> Option<u16> {
    let after_scheme = prefix.split_once("://").map_or(prefix, |(_, r)| r);
    let authority = after_scheme.split('/').next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (_, port) = authority.rsplit_once(':')?;
    port.parse().ok()
}

fn path_matches(prefix_path: &str, path: &str) -> bool {
    let p = prefix_path.trim_end_matches('/');
    p.is_empty() || path == p || path.starts_with(&format!("{p}/"))
}

/// Base64 (RFC 4648, mit Auffüllung) – kleine eigene Fassung, um keine weitere Abhängigkeit zu brauchen.
pub(crate) fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(prefix: &str, user: &str, pass: &str) -> BasicAuthEntry {
        BasicAuthEntry {
            prefix: prefix.to_string(),
            username: user.to_string(),
            password: pass.to_string(),
        }
    }

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    // spec: CRAWL-117
    #[test]
    fn base64_matches_reference_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    // spec: CRAWL-117
    #[test]
    fn credentials_are_utf8_encoded() {
        let m = AuthMatcher::new(&[entry("https://basic.site.test/", "prüfer", "geheim")]).unwrap();
        let h = m
            .authorization_for(&u("https://basic.site.test/x"))
            .unwrap();
        // "prüfer:geheim" als UTF-8-Bytes (ü = C3 BC)
        let expected = format!("Basic {}", base64_encode("pr\u{fc}fer:geheim".as_bytes()));
        assert_eq!(h, expected);
        assert_eq!(h, "Basic cHLDvGZlcjpnZWhlaW0=");
        // nicht Latin-1
        assert_ne!(h, format!("Basic {}", base64_encode(b"pr\xfcfer:geheim")));
    }

    // spec: CRAWL-117
    #[test]
    fn only_the_matching_host_gets_credentials() {
        let m = AuthMatcher::new(&[entry("https://basic.site.test/", "u", "p")]).unwrap();
        assert!(m
            .authorization_for(&u("https://basic.site.test/"))
            .is_some());
        assert!(m
            .authorization_for(&u("https://BASIC.site.test/a/b?q=1"))
            .is_some());
        // fremder Host (z. B. nach einer Weiterleitung), Subdomain, Suffix-Trick
        assert!(m
            .authorization_for(&u("https://other.test/echo-headers"))
            .is_none());
        assert!(m
            .authorization_for(&u("https://x.basic.site.test/"))
            .is_none());
        assert!(m
            .authorization_for(&u("https://evilbasic.site.test/"))
            .is_none());
        assert!(m
            .authorization_for(&u("https://basic.site.test.evil.com/"))
            .is_none());
    }

    // spec: CRAWL-117
    #[test]
    fn plain_http_needs_an_explicit_http_prefix() {
        let secure = AuthMatcher::new(&[
            entry("https://a.test/", "u", "p"),
            entry("b.test", "u", "p"),
        ])
        .unwrap();
        assert!(secure.authorization_for(&u("http://a.test/")).is_none());
        assert!(secure.authorization_for(&u("http://b.test/")).is_none());
        assert!(secure.authorization_for(&u("https://b.test/")).is_some());

        let consent = AuthMatcher::new(&[entry("http://a.test/", "u", "p")]).unwrap();
        assert!(consent.authorization_for(&u("http://a.test/")).is_some());
        // die Zustimmung für http gilt nicht für https desselben Hosts
        assert!(consent.authorization_for(&u("https://a.test/")).is_none());
    }

    // spec: CRAWL-117
    #[test]
    fn port_and_path_prefix_are_respected() {
        let m = AuthMatcher::new(&[
            entry("http://h.test:8080/admin", "admin", "1"),
            entry("http://h.test/", "all", "2"),
        ])
        .unwrap();
        let admin = m
            .authorization_for(&u("http://h.test:8080/admin/users"))
            .unwrap();
        assert_eq!(admin, format!("Basic {}", base64_encode(b"admin:1")));
        // anderer Port: nur der portlose Eintrag mit dem Pfad „/“ greift
        let other = m
            .authorization_for(&u("http://h.test:9000/admin/users"))
            .unwrap();
        assert_eq!(other, format!("Basic {}", base64_encode(b"all:2")));
        // Segmentgrenze: /administrator gehört nicht zu /admin
        let adm = m
            .authorization_for(&u("http://h.test:8080/administrator"))
            .unwrap();
        assert_eq!(adm, format!("Basic {}", base64_encode(b"all:2")));
        // kein Eintrag für https
        assert!(m
            .authorization_for(&u("https://h.test:8080/admin"))
            .is_none());
    }

    // spec: CRAWL-117
    #[test]
    fn invalid_entries_are_configuration_errors() {
        assert!(AuthMatcher::new(&[entry("", "u", "p")]).is_err());
        assert!(AuthMatcher::new(&[entry("ftp://x.test/", "u", "p")]).is_err());
        assert!(AuthMatcher::new(&[entry("https://x.test/", "a:b", "p")]).is_err());
        assert!(AuthMatcher::new(&[entry("https://", "u", "p")]).is_err());
        let err = AuthMatcher::new(&[entry("https://u:pw@x.test/", "a:b", "p")]).unwrap_err();
        assert!(
            !err.contains("pw"),
            "Fehlertext darf kein Passwort enthalten: {err}"
        );
        assert!(AuthMatcher::new(&[]).unwrap().is_empty());
    }

    // spec: CRAWL-117, OUT-243
    #[test]
    fn debug_output_does_not_leak_the_password() {
        let m = AuthMatcher::new(&[entry("https://a.test/", "u", "SUPER-GEHEIM")]).unwrap();
        let dbg = format!("{m:?}");
        assert!(!dbg.contains("SUPER-GEHEIM"));
        assert!(!dbg.contains(&base64_encode(b"u:SUPER-GEHEIM")));
    }
}
