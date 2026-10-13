//! Freigaben der Werkzeugklassen (AGENT-001, AGENT-003): `--allow`, `--read-only` und `mcp.toml`.
//!
//! Regeln (AGENT-001, OS-Festlegung):
//!
//! * Ohne Schalter gilt die Klassenfreigabe aus `mcp.toml` ([`crate::settings`]); im Auslieferungszustand
//!   nur `read`. `read` ist immer enthalten.
//! * `--allow <klassen>` erweitert die Menge um die genannten Klassen.
//! * `--read-only` erzwingt die Menge {`read`} und überstimmt dabei auch die Datei.
//! * `--read-only` zusammen mit `--allow` einer anderen Klasse als `read` ist ein Widerspruch
//!   ([`McpError::Usage`], Exit-Code 2, die Meldung nennt beide Schalter).
//! * Einzelne Werkzeuge schaltet nur `[tools] <name> = false` ab, nie `--allow`; `true` hebt keine
//!   Klassensperre auf.
//!
//! Zusätzlich liegt hier die Domain-Liste für Crawl-Werkzeuge (AGENT-003): [`DomainPolicy`] prüft die Ziel-URL
//! eines Aufrufs der Klasse `crawl`, bevor das Werkzeug läuft. Die Liste kommt ausschließlich aus
//! `mcp.toml`; per MCP lässt sie sich nicht ändern.

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde_json::Value;

use crate::error::McpError;
use crate::settings::McpSettings;
use crate::tool::{Tool, ToolClass};

/// Alle Klassen in fester Reihenfolge (Ausgabe, Meldungen).
pub const ALL_CLASSES: [ToolClass; 4] = [
    ToolClass::Read,
    ToolClass::Crawl,
    ToolClass::Write,
    ToolClass::Cost,
];

/// Name einer Klasse, wie er in `--allow` und `mcp.toml` steht.
pub fn class_name(class: ToolClass) -> &'static str {
    match class {
        ToolClass::Read => "read",
        ToolClass::Crawl => "crawl",
        ToolClass::Write => "write",
        ToolClass::Cost => "cost",
    }
}

/// Liest einen Klassennamen (Groß-/Kleinschreibung egal, Leerraum außen ignoriert).
pub fn parse_class(name: &str) -> Result<ToolClass, McpError> {
    let wanted = name.trim().to_ascii_lowercase();
    ALL_CLASSES
        .into_iter()
        .find(|c| class_name(*c) == wanted)
        .ok_or_else(|| {
            McpError::Usage(format!(
                "--allow: unbekannte Werkzeugklasse \"{}\"; gültig sind {}",
                name.trim(),
                ALL_CLASSES.map(class_name).join(", ")
            ))
        })
}

/// Zugriffsentscheidung für ein Werkzeug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Sichtbar und aufrufbar.
    Allowed,
    /// Die Klasse des Werkzeugs ist nicht freigegeben.
    ClassNotAllowed(ToolClass),
    /// Das Werkzeug ist in `mcp.toml` (`[tools] <name> = false`) abgeschaltet.
    DisabledInSettings,
}

/// Aufgelöste Freigaben eines Serverlaufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permissions {
    classes: Vec<ToolClass>,
    read_only: bool,
    disabled_tools: BTreeSet<String>,
    domains: DomainPolicy,
}

impl Permissions {
    /// Löst `--allow`, `--read-only` und die Einstellungsdatei zur Freigabemenge auf.
    ///
    /// # Errors
    /// [`McpError::Usage`] bei einer unbekannten Klasse in `--allow` und bei `--read-only` zusammen mit
    /// `--allow` einer anderen Klasse als `read`.
    pub fn resolve(
        allow: &[String],
        read_only: bool,
        settings: &McpSettings,
    ) -> Result<Permissions, McpError> {
        let mut allowed = Vec::new();
        for entry in allow.iter().filter(|e| !e.trim().is_empty()) {
            allowed.push(parse_class(entry)?);
        }
        let mut set: BTreeSet<usize> = BTreeSet::new();
        set.insert(0); // read gilt immer
        if read_only {
            if let Some(other) = allowed.iter().find(|c| **c != ToolClass::Read) {
                return Err(McpError::Usage(format!(
                    "--read-only und --allow {} widersprechen sich: --read-only erlaubt nur die Klasse read. \
                     Lassen Sie --read-only weg oder beschränken Sie --allow auf read.",
                    class_name(*other)
                )));
            }
        } else {
            for class in settings.classes.iter().chain(allowed.iter()) {
                set.insert(class_index(*class));
            }
        }
        Ok(Permissions {
            classes: set.into_iter().map(|i| ALL_CLASSES[i]).collect(),
            read_only,
            disabled_tools: settings
                .tools
                .iter()
                .filter(|(_, enabled)| !**enabled)
                .map(|(name, _)| name.clone())
                .collect(),
            domains: DomainPolicy::from_settings(settings),
        })
    }

    /// Freigegebene Klassen in fester Reihenfolge (`read` zuerst).
    pub fn classes(&self) -> &[ToolClass] {
        &self.classes
    }

    /// Ist die Klasse freigegeben?
    pub fn allows_class(&self, class: ToolClass) -> bool {
        self.classes.contains(&class)
    }

    /// Wurde `--read-only` angegeben?
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Namen der in `mcp.toml` abgeschalteten Werkzeuge.
    pub fn disabled_tools(&self) -> impl Iterator<Item = &str> {
        self.disabled_tools.iter().map(String::as_str)
    }

    /// Domain-Liste und Zielregeln für Crawl-Werkzeuge.
    pub fn domains(&self) -> &DomainPolicy {
        &self.domains
    }

    /// Zugriffsentscheidung für ein Werkzeug (Klasse zuerst, dann die Einzelschalter).
    pub fn access(&self, tool: &dyn Tool) -> Access {
        self.access_for(tool.name(), tool.class())
    }

    /// Wie [`Permissions::access`] für Name und Klasse.
    pub fn access_for(&self, name: &str, class: ToolClass) -> Access {
        if !self.allows_class(class) {
            Access::ClassNotAllowed(class)
        } else if self.disabled_tools.contains(name) {
            Access::DisabledInSettings
        } else {
            Access::Allowed
        }
    }

    /// Meldung für den Aufruf eines nicht freigegebenen Werkzeugs, mit Hinweis auf die Freigabe (AGENT-003).
    pub fn denial_message(&self, name: &str, access: Access) -> String {
        match access {
            Access::Allowed => format!("Werkzeug \"{name}\" ist freigegeben."),
            Access::ClassNotAllowed(class) => {
                let cn = class_name(class);
                let how = if self.read_only {
                    format!(
                        "Der Server läuft mit --read-only; starten Sie ihn ohne diesen Schalter und mit \
                         --allow {cn} oder geben Sie die Klasse {cn} in der App bzw. in mcp.toml \
                         (classes = [\"read\", \"{cn}\"]) frei."
                    )
                } else {
                    format!(
                        "Starten Sie den Server mit --allow {cn} oder geben Sie die Klasse {cn} in der App \
                         bzw. in mcp.toml (classes = [\"read\", \"{cn}\"]) frei."
                    )
                };
                format!(
                    "Das Werkzeug \"{name}\" gehört zur Klasse {cn} und ist nicht freigegeben. {how} \
                     Freigaben lassen sich nicht über MCP ändern."
                )
            }
            Access::DisabledInSettings => format!(
                "Das Werkzeug \"{name}\" ist in mcp.toml abgeschaltet ([tools] {name} = false). Entfernen \
                 Sie den Eintrag oder setzen Sie ihn auf true; per MCP lässt sich das nicht ändern."
            ),
        }
    }
}

fn class_index(class: ToolClass) -> usize {
    ALL_CLASSES
        .iter()
        .position(|c| *c == class)
        .unwrap_or_default()
}

/// Erlaubte Crawl-Ziele (AGENT-003): optionale Domain-Liste und Schutz vor privaten Zielen.
///
/// Die Prüfung läuft im Server vor jedem Werkzeug der Klasse `crawl`. Sie wertet die Argumente `start_url`,
/// `url` (Zeichenkette) und `urls` (Liste von Zeichenketten) aus; Crawl-Werkzeuge benennen ihr Ziel
/// entsprechend. Ein Aufruf ohne eines dieser Argumente passiert die Prüfung; das Werkzeug verlangt sein
/// Ziel selbst.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainPolicy {
    allowed: Vec<String>,
    allow_private: bool,
}

/// Argumente, die ein Crawl-Ziel tragen.
const TARGET_ARGS: [&str; 2] = ["start_url", "url"];

impl DomainPolicy {
    /// Baut die Regeln aus `allowed_domains` und `allow_private_targets`.
    pub fn from_settings(settings: &McpSettings) -> Self {
        DomainPolicy::new(&settings.allowed_domains, settings.allow_private_targets)
    }

    /// Baut die Regeln aus einer Domain-Liste (leer = alle Domains) und dem Schalter für private Ziele.
    pub fn new(domains: &[String], allow_private: bool) -> Self {
        DomainPolicy {
            allowed: domains
                .iter()
                .filter_map(|d| normalize_domain_entry(d))
                .collect(),
            allow_private,
        }
    }

    /// Ist eine Domain-Liste gesetzt?
    pub fn is_restricted(&self) -> bool {
        !self.allowed.is_empty()
    }

    /// Prüft alle Ziel-URLs in den Werkzeugargumenten.
    ///
    /// # Errors
    /// Meldung (deutsch) für den MCP-Fehler, wenn eine URL außerhalb der Liste liegt, kein `http`/`https`
    /// ist oder auf ein privates Ziel zeigt.
    pub fn check_args(&self, args: &Value) -> Result<(), String> {
        for key in TARGET_ARGS {
            if let Some(Value::String(url)) = args.get(key) {
                self.check_url(url)?;
            }
        }
        if let Some(Value::Array(urls)) = args.get("urls") {
            for url in urls.iter().filter_map(Value::as_str) {
                self.check_url(url)?;
            }
        }
        Ok(())
    }

    /// Prüft eine Ziel-URL.
    ///
    /// # Errors
    /// Meldung (deutsch), wenn die URL nicht erlaubt ist.
    pub fn check_url(&self, url: &str) -> Result<(), String> {
        let host = parse_http_host(url)?;
        if !self.allowed.is_empty() && !self.allowed.iter().any(|d| host_matches(&host, d)) {
            return Err(format!(
                "Die URL {url} liegt außerhalb der erlaubten Domains ({}). Die Liste steht in mcp.toml \
                 (allowed_domains) und lässt sich nicht über MCP ändern.",
                self.allowed.join(", ")
            ));
        }
        if !self.allow_private && is_private_host(&host) {
            return Err(format!(
                "Die URL {url} zeigt auf ein lokales oder privates Ziel. Das ist nur mit \
                 allow_private_targets = true in mcp.toml erlaubt; per MCP lässt sich das nicht ändern."
            ));
        }
        Ok(())
    }
}

/// Bringt einen Eintrag der Domain-Liste in die Vergleichsform (klein, ohne Schema, `*.` und Endpunkt).
fn normalize_domain_entry(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    let host = if trimmed.contains("://") {
        parse_http_host(trimmed).ok()?
    } else {
        trimmed.to_ascii_lowercase()
    };
    let host = host
        .trim_start_matches("*.")
        .trim_start_matches('.')
        .trim_end_matches('.')
        .to_string();
    (!host.is_empty()).then_some(host)
}

/// `host` ist die Domain selbst oder eine Subdomain davon.
fn host_matches(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|rest| rest.ends_with('.'))
}

/// Zieht den Host aus einer `http(s)`-URL. Verhält sich wie ein URL-Parser des Webs: Zugangsdaten vor `@`
/// zählen nicht zum Host, `\` beendet die Autorität wie `/`, Groß-/Kleinschreibung und ein Endpunkt sind
/// egal. Prozentkodierte Hosts werden abgelehnt.
fn parse_http_host(url: &str) -> Result<String, String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
        .ok_or_else(|| format!("Die URL {url} muss mit http:// oder https:// beginnen."))?;
    let end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(v6) = hostport.strip_prefix('[') {
        let close = v6
            .find(']')
            .ok_or_else(|| format!("Die URL {url} hat einen ungültigen Host."))?;
        v6[..close].to_string()
    } else {
        hostport
            .split(':')
            .next()
            .unwrap_or_default()
            .trim_end_matches('.')
            .to_string()
    };
    if host.is_empty() || (host.contains('%') && !hostport.starts_with('[')) {
        return Err(format!("Die URL {url} hat keinen gültigen Host."));
    }
    Ok(host)
}

/// Lokale und private Ziele: `localhost`, `*.localhost` und IP-Adressen aus privaten, Loopback-,
/// Link-Local-, Unique-Local- oder CGNAT-Bereichen. Unklare Zahlenformen (`2130706433`, `0x7f.1`, `127.1`)
/// gelten als privat, weil Web-URL-Parser sie als IPv4-Adresse lesen.
fn is_private_host(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(ip);
    }
    let last = host.rsplit('.').next().unwrap_or_default();
    last.starts_with("0x") || (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_private_v4(mapped);
            }
            is_private_v6(v6)
        }
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || (a == 100 && (64..128).contains(&b))
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (first & 0xfe00) == 0xfc00 // fc00::/7 unique local
        || (first & 0xffc0) == 0xfe80 // fe80::/10 link local
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| (*i).to_string()).collect()
    }

    fn settings_with(classes: &[ToolClass]) -> McpSettings {
        McpSettings {
            classes: classes.to_vec(),
            ..McpSettings::default()
        }
    }

    // spec: AGENT-003
    #[test]
    fn default_is_read_only() {
        let p = Permissions::resolve(&[], false, &McpSettings::default()).unwrap();
        assert_eq!(p.classes(), [ToolClass::Read]);
        assert!(!p.read_only());
    }

    // spec: AGENT-001
    #[test]
    fn allow_extends_and_read_is_always_included() {
        let p = Permissions::resolve(&s(&["crawl"]), false, &McpSettings::default()).unwrap();
        assert_eq!(p.classes(), [ToolClass::Read, ToolClass::Crawl]);
        let err =
            Permissions::resolve(&s(&["write, Crawl"]), false, &settings_with(&[])).unwrap_err();
        assert!(matches!(err, McpError::Usage(_)), "Komma trennt nur clap");
        let p = Permissions::resolve(&s(&["write", "Crawl"]), false, &settings_with(&[])).unwrap();
        assert_eq!(
            p.classes(),
            [ToolClass::Read, ToolClass::Crawl, ToolClass::Write]
        );
    }

    // spec: AGENT-001
    #[test]
    fn settings_file_classes_apply_without_switches() {
        let p = Permissions::resolve(&[], false, &settings_with(&[ToolClass::Write])).unwrap();
        assert_eq!(p.classes(), [ToolClass::Read, ToolClass::Write]);
        // --allow ergänzt die Datei
        let p = Permissions::resolve(&s(&["crawl"]), false, &settings_with(&[ToolClass::Write]))
            .unwrap();
        assert_eq!(
            p.classes(),
            [ToolClass::Read, ToolClass::Crawl, ToolClass::Write]
        );
    }

    // spec: AGENT-001
    #[test]
    fn read_only_overrides_the_settings_file() {
        let p = Permissions::resolve(
            &[],
            true,
            &settings_with(&[ToolClass::Write, ToolClass::Cost]),
        )
        .unwrap();
        assert_eq!(p.classes(), [ToolClass::Read]);
        assert!(p.read_only());
    }

    // spec: AGENT-001
    #[test]
    fn read_only_with_other_class_is_a_usage_error_naming_both_switches() {
        let err = Permissions::resolve(&s(&["crawl"]), true, &McpSettings::default()).unwrap_err();
        assert!(matches!(err, McpError::Usage(_)));
        assert_eq!(err.exit_code(), 2);
        let msg = err.to_string();
        assert!(
            msg.contains("--read-only") && msg.contains("--allow"),
            "{msg}"
        );
        // read selbst ist kein Widerspruch
        let ok = Permissions::resolve(&s(&["read"]), true, &McpSettings::default()).unwrap();
        assert_eq!(ok.classes(), [ToolClass::Read]);
    }

    // spec: AGENT-001
    #[test]
    fn unknown_class_is_a_usage_error_listing_valid_classes() {
        let err = Permissions::resolve(&s(&["admin"]), false, &McpSettings::default()).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        let msg = err.to_string();
        for c in ["read", "crawl", "write", "cost", "admin"] {
            assert!(msg.contains(c), "{msg}");
        }
    }

    // spec: AGENT-003
    #[test]
    fn access_checks_class_then_single_tool_switch() {
        let mut settings = settings_with(&[ToolClass::Crawl]);
        settings.tools.insert("query_urls".into(), false);
        settings.tools.insert("start_crawl".into(), true);
        let p = Permissions::resolve(&[], false, &settings).unwrap();
        assert_eq!(
            p.access_for("list_crawls", ToolClass::Read),
            Access::Allowed
        );
        assert_eq!(
            p.access_for("query_urls", ToolClass::Read),
            Access::DisabledInSettings
        );
        assert_eq!(
            p.access_for("start_crawl", ToolClass::Crawl),
            Access::Allowed
        );
        assert_eq!(
            p.access_for("export_tab", ToolClass::Write),
            Access::ClassNotAllowed(ToolClass::Write)
        );
        // true hebt eine Klassensperre nicht auf
        settings.tools.insert("export_tab".into(), true);
        let p = Permissions::resolve(&[], false, &settings).unwrap();
        assert_eq!(
            p.access_for("export_tab", ToolClass::Write),
            Access::ClassNotAllowed(ToolClass::Write)
        );
    }

    // spec: AGENT-003
    #[test]
    fn denial_message_says_how_to_enable() {
        let p = Permissions::resolve(&[], false, &McpSettings::default()).unwrap();
        let msg = p.denial_message("start_crawl", Access::ClassNotAllowed(ToolClass::Crawl));
        assert!(msg.contains("--allow crawl"), "{msg}");
        assert!(msg.contains("mcp.toml"), "{msg}");
        let ro = Permissions::resolve(&[], true, &McpSettings::default()).unwrap();
        let msg = ro.denial_message("start_crawl", Access::ClassNotAllowed(ToolClass::Crawl));
        assert!(msg.contains("--read-only"), "{msg}");
        let msg = p.denial_message("query_urls", Access::DisabledInSettings);
        assert!(msg.contains("[tools] query_urls = false"), "{msg}");
    }

    fn policy(domains: &[&str], private: bool) -> DomainPolicy {
        DomainPolicy::new(&s(domains), private)
    }

    // spec: AGENT-003
    #[test]
    fn domain_list_allows_listed_hosts_and_subdomains_only() {
        let p = policy(&["out-basic-site.test"], true);
        assert!(p.check_url("http://out-basic-site.test/").is_ok());
        assert!(p
            .check_url("https://www.out-basic-site.test:8443/a?b#c")
            .is_ok());
        assert!(p.check_url("HTTP://OUT-BASIC-SITE.TEST./").is_ok());
        let err = p.check_url("http://other.test/").unwrap_err();
        assert!(err.contains("out-basic-site.test"), "{err}");
        assert!(p.check_url("http://evilout-basic-site.test/").is_err());
        assert!(p
            .check_url("http://out-basic-site.test.evil.test/")
            .is_err());
    }

    // spec: AGENT-003
    #[test]
    fn domain_list_resists_url_tricks() {
        let p = policy(&["good.test"], true);
        assert!(
            p.check_url("http://good.test@evil.test/").is_err(),
            "Userinfo"
        );
        assert!(
            p.check_url("http://evil.test\\@good.test/").is_err(),
            "Backslash"
        );
        assert!(
            p.check_url("http://evil.test#@good.test/").is_err(),
            "Fragment"
        );
        assert!(
            p.check_url("http://evil.test?.good.test/").is_err(),
            "Query"
        );
        assert!(
            p.check_url("http://go%6fd.test/").is_err(),
            "Prozentkodierung"
        );
        assert!(p.check_url("ftp://good.test/").is_err(), "Schema");
        assert!(p.check_url("good.test").is_err(), "ohne Schema");
        assert!(p.check_url("http://user:pw@good.test:80/").is_ok());
    }

    // spec: AGENT-003
    #[test]
    fn domain_list_entries_are_normalised() {
        let p = policy(
            &["*.Good.Test", "https://other.test/", " third.test. ", ""],
            true,
        );
        assert!(p.check_url("http://good.test/").is_ok());
        assert!(p.check_url("http://a.good.test/").is_ok());
        assert!(p.check_url("http://other.test/x").is_ok());
        assert!(p.check_url("http://third.test/").is_ok());
        assert!(p.is_restricted());
        assert!(!policy(&[""], true).is_restricted());
    }

    // spec: AGENT-003
    #[test]
    fn empty_list_means_no_domain_restriction() {
        let p = policy(&[], false);
        assert!(p.check_url("https://anything.example/").is_ok());
        assert!(!p.is_restricted());
    }

    // spec: AGENT-003
    #[test]
    fn private_targets_are_blocked_unless_allowed() {
        let strict = policy(&[], false);
        for url in [
            "http://localhost/",
            "http://app.localhost:8080/",
            "http://127.0.0.1:3000/",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data",
            "http://100.64.0.1/",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://127.1/",
            "http://0.0.0.0/",
        ] {
            assert!(strict.check_url(url).is_err(), "{url} muss gesperrt sein");
        }
        for url in [
            "http://example.com/",
            "http://8.8.8.8/",
            "https://[2606:4700::1111]/",
            "http://out-basic-site.test/",
            "http://172.32.0.1/",
        ] {
            assert!(strict.check_url(url).is_ok(), "{url} muss erlaubt sein");
        }
        let open = policy(&[], true);
        assert!(open.check_url("http://127.0.0.1:3000/").is_ok());
    }

    // spec: AGENT-003
    #[test]
    fn check_args_looks_at_start_url_url_and_urls() {
        let p = policy(&["good.test"], true);
        assert!(p
            .check_args(&json!({"start_url": "http://good.test/"}))
            .is_ok());
        assert!(p.check_args(&json!({"url": "http://bad.test/"})).is_err());
        assert!(p
            .check_args(&json!({"urls": ["http://good.test/", "http://bad.test/"]}))
            .is_err());
        // Aufrufe ohne Ziel passieren; das Werkzeug verlangt sein Ziel selbst.
        assert!(p.check_args(&json!({"limit": 5})).is_ok());
        assert!(p.check_args(&json!({})).is_ok());
    }
}
