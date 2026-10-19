//! Fehlerklassifikation „Keine Antwort“ (CRAWL-013, ONPAGE-065).
//!
//! Der Fehler von reqwest wird entlang seiner Ursachenkette untersucht; die Entscheidung selbst
//! trifft die reine Funktion [`classify_parts`], damit sie ohne Netz testbar ist.

use std::error::Error;
use std::io::ErrorKind;

use os_model::NoResponseReason;

use crate::redact;
use crate::resolve::ResolveError;

/// Ein Fehlschlag mit Ursache und Detailtext für das Protokoll (bereits redigiert).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// CONNECT was rejected with HTTP 407.
    pub proxy_auth_required: bool,
    /// Klassifizierte Ursache.
    pub reason: NoResponseReason,
    /// Technische Einzelheit.
    pub detail: String,
}

/// Auswertbare Merkmale eines Fehlers.
#[derive(Debug, Default, Clone)]
pub struct ErrorParts {
    /// reqwest meldet eine Zeitüberschreitung.
    pub timeout: bool,
    /// Der Fehler trat beim Verbindungsaufbau auf.
    pub connect: bool,
    /// Die Anfrage konnte gar nicht gebaut werden (ungültige URL).
    pub builder: bool,
    /// Fehler der Namensauflösung in der Kette.
    pub resolve: Option<ResolveError>,
    /// Alle `io::Error`-Arten (und OS-Fehlernummern) in der Kette.
    pub io: Vec<(ErrorKind, Option<i32>)>,
    /// Die Meldungen der Kette, außen nach innen.
    pub messages: Vec<String>,
}

/// Klassifiziert einen reqwest-Fehler.
pub fn classify(err: &reqwest::Error) -> Failure {
    let mut parts = ErrorParts {
        timeout: err.is_timeout(),
        connect: err.is_connect(),
        builder: err.is_builder(),
        ..ErrorParts::default()
    };
    let mut cur: Option<&(dyn Error + 'static)> = Some(err);
    while let Some(e) = cur {
        parts.messages.push(e.to_string());
        if let Some(r) = e.downcast_ref::<ResolveError>() {
            parts.resolve = Some(r.clone());
        }
        if let Some(io) = e.downcast_ref::<std::io::Error>() {
            parts.io.push((io.kind(), io.raw_os_error()));
        }
        cur = e.source();
    }
    classify_parts(&parts)
}

const TLS_HINTS: &[&str] = &[
    "certificate",
    "handshake",
    "rustls",
    "tls ",
    "tls:",
    "(tls)",
    "unknownissuer",
    "invalid peer",
    "corrupt message",
    "invalidcontenttype",
    "fatal alert",
    "peer is incompatible",
    "peer sent no certificates",
];

/// Entscheidet anhand der Merkmale (Reihenfolge ist Teil der Festlegung).
pub fn classify_parts(p: &ErrorParts) -> Failure {
    let detail = redact::text(&dedup_join(&p.messages));
    let reason = decide(p);
    // reqwest 0.13.5 does not expose CONNECT status as a typed error.
    // Keep its tunnel diagnostic dependency here, covered by the CONNECT fixture.
    let proxy_auth_required = p
        .messages
        .iter()
        .any(|m| strip_urls(m).contains("tunnel error: proxy authorization required"));
    Failure {
        reason,
        detail,
        proxy_auth_required,
    }
}

fn decide(p: &ErrorParts) -> NoResponseReason {
    if p.builder {
        return NoResponseReason::Malformed;
    }
    match &p.resolve {
        Some(ResolveError::Timeout { .. }) => return NoResponseReason::ConnectionTimeout,
        Some(ResolveError::Failed { .. }) => return NoResponseReason::DnsLookupFailed,
        None => {}
    }
    // Adressen aus den Meldungen entfernen, damit z. B. `/certificate` im Pfad nichts auslöst.
    let lower: Vec<String> = p
        .messages
        .iter()
        .map(|m| strip_urls(&m.to_ascii_lowercase()))
        .collect();
    if lower
        .iter()
        .any(|m| TLS_HINTS.iter().any(|hint| m.contains(hint)))
    {
        return NoResponseReason::TlsError;
    }
    if p.timeout {
        return if p.connect {
            NoResponseReason::ConnectionTimeout
        } else {
            NoResponseReason::ResponseTimeout
        };
    }
    if lower
        .iter()
        .any(|m| m.contains("dns error") || m.contains("failed to lookup address"))
    {
        return NoResponseReason::DnsLookupFailed;
    }
    for (kind, _) in &p.io {
        match kind {
            ErrorKind::ConnectionRefused => return NoResponseReason::ConnectionRefused,
            ErrorKind::TimedOut => {
                return if p.connect {
                    NoResponseReason::ConnectionTimeout
                } else {
                    NoResponseReason::ResponseTimeout
                };
            }
            _ => {}
        }
    }
    NoResponseReason::ConnectionError
}

fn strip_urls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("://") {
        // Schema davor (Buchstaben) mit entfernen
        let scheme_start = rest[..start]
            .rfind(|c: char| !c.is_ascii_alphabetic())
            .map_or(0, |i| i + 1);
        out.push_str(&rest[..scheme_start]);
        let tail = &rest[start + 3..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Meldungen der Kette ohne unmittelbare Wiederholungen, mit `: ` verbunden.
fn dedup_join(messages: &[String]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for m in messages {
        if out.last() != Some(&m.as_str()) && !m.is_empty() {
            out.push(m);
        }
    }
    out.join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> ErrorParts {
        ErrorParts::default()
    }

    // spec: CRAWL-013, ONPAGE-065
    #[test]
    fn resolve_errors_map_to_dns_and_connect_timeout() {
        let mut p = parts();
        p.connect = true;
        p.resolve = Some(ResolveError::Failed {
            host: "x.invalid".into(),
            detail: "nodename nor servname provided".into(),
        });
        assert_eq!(classify_parts(&p).reason, NoResponseReason::DnsLookupFailed);
        p.resolve = Some(ResolveError::Timeout {
            host: "x.test".into(),
        });
        assert_eq!(
            classify_parts(&p).reason,
            NoResponseReason::ConnectionTimeout
        );
    }

    // spec: CRAWL-013, ONPAGE-065
    #[test]
    fn refused_reset_and_timeouts() {
        let mut p = parts();
        p.connect = true;
        p.io = vec![(ErrorKind::ConnectionRefused, Some(61))];
        assert_eq!(
            classify_parts(&p).reason,
            NoResponseReason::ConnectionRefused
        );

        let mut p = parts();
        p.io = vec![(ErrorKind::ConnectionReset, Some(54))];
        assert_eq!(classify_parts(&p).reason, NoResponseReason::ConnectionError);

        let mut p = parts();
        p.timeout = true;
        p.connect = true;
        assert_eq!(
            classify_parts(&p).reason,
            NoResponseReason::ConnectionTimeout
        );

        let mut p = parts();
        p.timeout = true;
        assert_eq!(classify_parts(&p).reason, NoResponseReason::ResponseTimeout);

        let mut p = parts();
        p.connect = true;
        p.io = vec![(ErrorKind::TimedOut, Some(60))];
        assert_eq!(
            classify_parts(&p).reason,
            NoResponseReason::ConnectionTimeout
        );
    }

    // spec: CRAWL-013, ONPAGE-065
    #[test]
    fn tls_problems_win_over_generic_connect_errors() {
        for msg in [
            "invalid peer certificate: UnknownIssuer",
            "invalid peer certificate: Expired",
            "received corrupt message of type InvalidContentType",
            "peer sent no certificates",
            "received fatal alert: HandshakeFailure",
        ] {
            let mut p = parts();
            p.connect = true;
            p.messages = vec!["error sending request".into(), msg.into()];
            p.io = vec![(ErrorKind::InvalidData, None)];
            assert_eq!(
                classify_parts(&p).reason,
                NoResponseReason::TlsError,
                "{msg}"
            );
        }
    }

    // spec: CRAWL-013, ONPAGE-065
    #[test]
    fn invalid_requests_and_unknown_errors() {
        let mut p = parts();
        p.builder = true;
        assert_eq!(classify_parts(&p).reason, NoResponseReason::Malformed);
        let mut p = parts();
        p.messages = vec!["error decoding response body".into()];
        assert_eq!(classify_parts(&p).reason, NoResponseReason::ConnectionError);
    }

    // spec: CRAWL-013
    #[test]
    fn words_inside_the_url_do_not_count_as_tls_hints() {
        let mut p = parts();
        p.connect = true;
        p.messages = vec![
            "error sending request for url (https://x.test/certificate/handshake)".into(),
            "tcp connect error".into(),
        ];
        p.io = vec![(ErrorKind::ConnectionRefused, None)];
        assert_eq!(
            classify_parts(&p).reason,
            NoResponseReason::ConnectionRefused
        );
    }

    // spec: OUT-243
    #[test]
    fn detail_is_redacted_and_deduplicated() {
        let mut p = parts();
        p.messages = vec![
            "error sending request for url (https://u:pw@h.test/x?token=abc)".into(),
            "client error (Connect)".into(),
            "client error (Connect)".into(),
        ];
        let f = classify_parts(&p);
        assert!(!f.detail.contains("pw"), "{}", f.detail);
        assert!(!f.detail.contains("abc"), "{}", f.detail);
        assert_eq!(f.detail.matches("client error (Connect)").count(), 1);
    }
}
