//! Host-Auflösung `network.resolve` (CRAWL-123).
//!
//! Regeln der Form `host=ip`, `host=ip:port` oder `*.domain=ip[:port]` ersetzen die DNS-Auflösung
//! (wie ein Eintrag in der Hosts-Datei). `Host`-Header und TLS-SNI behalten den Hostnamen, weil nur
//! die Zieladresse der Verbindung getauscht wird. Der Port ist eine Test-Erweiterung (K-11): Er gilt
//! nur für URLs ohne expliziten Port; steht in der URL ein Port, gewinnt dieser.
//!
//! `*.domain` trifft alle Subdomains **und** `domain` selbst (wie `address=/domain/` in dnsmasq),
//! damit eine einzige Regel alle Fixture-Hosts einer Test-Domain abdeckt. Eine exakte Regel geht
//! einer Wildcard-Regel vor, unter mehreren Wildcards gewinnt die längste Domain.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Eine geparste Auflösungsregel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveRule {
    /// Kleingeschriebener Host bzw. bei Wildcard die Domain ohne `*.`.
    pub host: String,
    /// Ob die Regel als `*.domain` angegeben wurde.
    pub wildcard: bool,
    /// Ziel-IP.
    pub ip: IpAddr,
    /// Zielport; `None` = Port der URL bzw. Standardport des Schemas.
    pub port: Option<u16>,
}

impl ResolveRule {
    /// Parst `host=ip[:port]` bzw. `*.domain=ip[:port]`.
    pub fn parse(rule: &str) -> Result<Self, String> {
        let (left, right) = rule
            .split_once('=')
            .ok_or_else(|| format!("Resolve-Regel {rule:?}: `host=ip[:port]` erwartet"))?;
        let left = left.trim().trim_end_matches('.').to_ascii_lowercase();
        let (wildcard, host) = match left.strip_prefix("*.") {
            Some(domain) => (true, domain.to_string()),
            None => (false, left),
        };
        if host.is_empty()
            || host.contains('*')
            || host.contains('/')
            || host.contains(char::is_whitespace)
        {
            return Err(format!("Resolve-Regel {rule:?}: ungültiger Host"));
        }
        let right = right.trim();
        let (ip, port) = if let Ok(sa) = right.parse::<SocketAddr>() {
            (sa.ip(), Some(sa.port()))
        } else if let Ok(ip) = right.parse::<IpAddr>() {
            (ip, None)
        } else {
            return Err(format!(
                "Resolve-Regel {rule:?}: `{right}` ist keine IP-Adresse (mit optionalem Port)"
            ));
        };
        Ok(ResolveRule {
            host,
            wildcard,
            ip,
            port,
        })
    }

    fn matches(&self, host: &str) -> bool {
        if self.wildcard {
            host == self.host
                || host
                    .strip_suffix(self.host.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
        } else {
            host == self.host
        }
    }

    fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port.unwrap_or(0))
    }
}

/// Fehler der Namensauflösung (wird vom Fetcher zu „DNS Lookup Failed“ bzw. „Connection Timeout“).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// Name nicht auflösbar.
    Failed {
        /// Host.
        host: String,
        /// Einzelheit des Systems.
        detail: String,
    },
    /// Die Auflösung dauerte länger als das Verbindungs-Timeout.
    Timeout {
        /// Host.
        host: String,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Failed { host, detail } => {
                write!(f, "dns error: {host} nicht auflösbar ({detail})")
            }
            ResolveError::Timeout { host } => write!(f, "dns error: Zeitüberschreitung bei {host}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Ob `host` zur reservierten TLD `invalid` gehört (RFC 6761 6.4, K-51): `invalid` selbst und
/// `*.invalid`, Groß-/Kleinschreibung und abschließender Punkt egal.
fn is_reserved_invalid(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "invalid" || host.ends_with(".invalid")
}

/// Löst Hosts auf: zuerst die Überschreibungen, dann die reservierte TLD `.invalid` (nie
/// auflösbar, ohne System-Resolver beantwortet), sonst das System.
#[derive(Debug, Clone)]
pub struct HostResolver {
    rules: Vec<ResolveRule>,
    timeout: Duration,
}

impl HostResolver {
    /// Baut den Resolver aus Regeltexten; `timeout` begrenzt die Systemauflösung.
    pub fn new(rules: &[String], timeout: Duration) -> Result<Self, String> {
        Ok(HostResolver {
            rules: rules
                .iter()
                .map(|r| ResolveRule::parse(r))
                .collect::<Result<_, _>>()?,
            timeout,
        })
    }

    /// Ob mindestens eine Regel aktiv ist.
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }

    /// Adressen aus den Regeln für `host`; `None`, wenn keine Regel passt.
    pub fn lookup_override(&self, host: &str) -> Option<Vec<SocketAddr>> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let exact: Vec<SocketAddr> = self
            .rules
            .iter()
            .filter(|r| !r.wildcard && r.matches(&host))
            .map(ResolveRule::addr)
            .collect();
        if !exact.is_empty() {
            return Some(exact);
        }
        let best = self
            .rules
            .iter()
            .filter(|r| r.wildcard && r.matches(&host))
            .map(|r| r.host.len())
            .max()?;
        Some(
            self.rules
                .iter()
                .filter(|r| r.wildcard && r.host.len() == best && r.matches(&host))
                .map(ResolveRule::addr)
                .collect(),
        )
    }
}

impl Resolve for HostResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        if let Some(addrs) = self.lookup_override(&host) {
            return Box::pin(async move { Ok(Box::new(addrs.into_iter()) as Addrs) });
        }
        // K-51: `invalid`/`*.invalid` (RFC 6761) sind nie auflösbar. Sofortige Antwort statt
        // System-Resolver, damit "DNS Lookup Failed" auch ohne Netz und auf trägen CI-Resolvern
        // deterministisch ist (kein Verbindungs-Timeout).
        if is_reserved_invalid(&host) {
            return Box::pin(async move {
                Err(Box::new(ResolveError::Failed {
                    host,
                    detail: "reservierte TLD .invalid (RFC 6761)".to_string(),
                }) as _)
            });
        }
        let timeout = self.timeout;
        Box::pin(async move {
            let found = {
                let lookup = tokio::net::lookup_host((host.clone(), 0));
                tokio::time::timeout(timeout, lookup).await
            };
            match found {
                Err(_) => Err(Box::new(ResolveError::Timeout { host }) as _),
                Ok(Err(e)) => Err(Box::new(ResolveError::Failed {
                    host,
                    detail: e.to_string(),
                }) as _),
                Ok(Ok(found)) => {
                    let addrs: Vec<SocketAddr> = found.collect();
                    if addrs.is_empty() {
                        Err(Box::new(ResolveError::Failed {
                            host,
                            detail: "keine Adresse".to_string(),
                        }) as _)
                    } else {
                        Ok(Box::new(addrs.into_iter()) as Addrs)
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver(rules: &[&str]) -> HostResolver {
        let rules: Vec<String> = rules.iter().map(|s| s.to_string()).collect();
        HostResolver::new(&rules, Duration::from_secs(5)).unwrap()
    }

    fn addrs(r: &HostResolver, host: &str) -> Option<Vec<String>> {
        r.lookup_override(host)
            .map(|v| v.iter().map(ToString::to_string).collect())
    }

    // spec: CRAWL-123
    #[test]
    fn wildcard_maps_subdomains_and_apex() {
        let r = resolver(&["*.site.test=127.0.0.1"]);
        assert_eq!(addrs(&r, "a.site.test"), Some(vec!["127.0.0.1:0".into()]));
        assert_eq!(addrs(&r, "x.y.site.test"), Some(vec!["127.0.0.1:0".into()]));
        assert_eq!(addrs(&r, "SITE.test"), Some(vec!["127.0.0.1:0".into()]));
        assert_eq!(addrs(&r, "site.test."), Some(vec!["127.0.0.1:0".into()]));
        assert_eq!(addrs(&r, "notsite.test"), None);
        assert_eq!(addrs(&r, "site.test.evil.com"), None);
        assert_eq!(addrs(&r, "example.org"), None);
    }

    // spec: CRAWL-123
    #[test]
    fn port_extension_is_carried_into_the_address() {
        let r = resolver(&["*.site.test=127.0.0.1:8123", "plain.test=::1"]);
        assert_eq!(
            addrs(&r, "a.site.test"),
            Some(vec!["127.0.0.1:8123".into()])
        );
        assert_eq!(addrs(&r, "plain.test"), Some(vec!["[::1]:0".into()]));
    }

    // spec: CRAWL-123
    #[test]
    fn exact_rule_beats_wildcard_and_longer_wildcard_beats_shorter() {
        let r = resolver(&[
            "*.test=10.0.0.1",
            "*.site.test=10.0.0.2",
            "special.site.test=10.0.0.3",
        ]);
        assert_eq!(
            addrs(&r, "special.site.test"),
            Some(vec!["10.0.0.3:0".into()])
        );
        assert_eq!(
            addrs(&r, "other.site.test"),
            Some(vec!["10.0.0.2:0".into()])
        );
        assert_eq!(addrs(&r, "x.test"), Some(vec!["10.0.0.1:0".into()]));
    }

    // spec: CRAWL-123
    #[test]
    fn invalid_rules_are_rejected() {
        for bad in [
            "",
            "kein-gleichheitszeichen",
            "=127.0.0.1",
            "host=keine-ip",
            "*.=127.0.0.1",
            "a*b.test=127.0.0.1",
            "host=127.0.0.1:99999",
        ] {
            assert!(
                ResolveRule::parse(bad).is_err(),
                "{bad:?} muss abgelehnt werden"
            );
        }
        assert!(HostResolver::new(&["x=1.2.3.4".into(), "kaputt".into()], Duration::ZERO).is_err());
    }

    // spec: CRAWL-123
    #[tokio::test]
    async fn resolve_trait_returns_override_and_fails_for_unknown_names() {
        let r = resolver(&["*.site.test=127.0.0.1:9"]);
        let found: Vec<SocketAddr> = r
            .resolve("a.site.test".parse().unwrap())
            .await
            .unwrap()
            .collect();
        assert_eq!(found, vec!["127.0.0.1:9".parse::<SocketAddr>().unwrap()]);
        // `.invalid` ist laut RFC 6761 nie auflösbar – Fehlertyp muss ResolveError sein
        let err = match r.resolve("nicht-existent.invalid".parse().unwrap()).await {
            Err(e) => e,
            Ok(_) => panic!("`.invalid` darf nicht auflösbar sein"),
        };
        assert!(err.downcast_ref::<ResolveError>().is_some(), "{err}");
    }

    // spec: ONPAGE-054, CRAWL-123
    #[tokio::test]
    async fn reserved_tld_invalid_fails_immediately_without_the_system_resolver() {
        // Verbindungs-Timeout 1 ms: Wäre der System-Resolver gefragt, käme `Timeout`.
        let r = HostResolver::new(&[], Duration::from_millis(1)).unwrap();
        for host in ["a.invalid", "INVALID.", "x.y.Invalid.", "invalid"] {
            let err = match r.resolve(host.parse().unwrap()).await {
                Err(e) => e,
                Ok(_) => panic!("{host}: `.invalid` darf nicht auflösbar sein"),
            };
            match err.downcast_ref::<ResolveError>() {
                Some(ResolveError::Failed { detail, .. }) => {
                    assert!(detail.contains(".invalid"), "{host}: {detail}")
                }
                other => panic!("{host}: ResolveError::Failed erwartet, bekam {other:?}"),
            }
        }
        // Regeln aus network.resolve gehen vor: eine Regel für x.invalid gilt.
        let r = HostResolver::new(
            &["x.invalid=127.0.0.1".to_string()],
            Duration::from_millis(1),
        )
        .unwrap();
        let found: Vec<SocketAddr> = r
            .resolve("x.invalid".parse().unwrap())
            .await
            .unwrap()
            .collect();
        assert_eq!(found, vec!["127.0.0.1:0".parse::<SocketAddr>().unwrap()]);
        // Andere Namen unter `invalid` ohne Regel bleiben Fehler.
        assert!(r.resolve("y.invalid".parse().unwrap()).await.is_err());
    }
}
