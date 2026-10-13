//! Validierung einer [`CrawlConfig`] (CRAWL-140, OUT-221): Wertebereiche, `min ≤ max`, Regex-Syntax.
//!
//! `validate` lädt oder ändert nichts; es sammelt Fehler (Konfiguration ist so nicht verwendbar) und Warnungen
//! (Konfiguration wird angenommen, der Nutzer bekommt einen Hinweis). Die Bereichstabelle [`INT_RANGES`] ist die
//! einzige Quelle für Ganzzahl-Grenzen: `set` liest sie über das JSON-Schema, das sie als `minimum`/`maximum`
//! einträgt, und `validate` prüft damit die geladenen Werte.

use std::collections::BTreeMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};

use serde_json::Value;

use crate::config::{CrawlConfig, TextLimits};

/// Art einer Meldung zu einer Konfiguration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigIssueKind {
    /// Unbekannter Schlüssel in der Datei (wird ignoriert, OUT-221).
    UnknownField,
    /// `schema_version` ist neuer als die unterstützte Version.
    NewerSchemaVersion,
    /// Ungültiger Wert (Validierungsfehler).
    InvalidValue,
    /// Wert wird angenommen, verdient aber einen Hinweis.
    Warning,
}

/// Eine Meldung mit Feldpfad und – bei geladenen Dateien – Zeile/Spalte des Schlüssels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigIssue {
    /// Art der Meldung.
    pub kind: ConfigIssueKind,
    /// Feldpfad, z. B. `speed.threads` oder `auth.basic[0].prefix`.
    pub path: String,
    /// Zeile des Schlüssels (1-basiert), wenn die Konfiguration aus einer Datei stammt.
    pub line: Option<u32>,
    /// Spalte des Schlüssels (1-basiert).
    pub column: Option<u32>,
    /// Beschreibung (deutsch).
    pub message: String,
}

impl ConfigIssue {
    pub(crate) fn new(
        kind: ConfigIssueKind,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        ConfigIssue {
            kind,
            path: path.into(),
            line: None,
            column: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)?;
        if let (Some(l), Some(c)) = (self.line, self.column) {
            write!(f, " (Zeile {l}, Spalte {c})")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// Ergebnis von [`CrawlConfig::validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Fehler: die Konfiguration darf so nicht verwendet bzw. gespeichert werden.
    pub errors: Vec<ConfigIssue>,
    /// Warnungen: die Konfiguration wird angenommen.
    pub warnings: Vec<ConfigIssue>,
}

impl ValidationReport {
    /// Keine Fehler (Warnungen sind erlaubt).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// `Ok(Warnungen)` ohne Fehler, sonst `Err(Fehler)`.
    pub fn into_result(self) -> Result<Vec<ConfigIssue>, Vec<ConfigIssue>> {
        if self.errors.is_empty() {
            Ok(self.warnings)
        } else {
            Err(self.errors)
        }
    }

    pub(crate) fn locate(&mut self, locations: &BTreeMap<String, (u32, u32)>) {
        for i in self.errors.iter_mut().chain(self.warnings.iter_mut()) {
            // Listenelemente (`scope.exclude[0]`) haben keine eigene Position: dann die der Liste.
            let list_path = i.path.rfind('[').map(|at| &i.path[..at]);
            let found = locations
                .get(&i.path)
                .or_else(|| list_path.and_then(|p| locations.get(p)));
            if let Some(&(l, c)) = found {
                i.line = Some(l);
                i.column = Some(c);
            }
        }
    }
}

/// Grenzen einer Ganzzahl-Option (inklusive).
#[derive(Debug, Clone, Copy)]
pub(crate) struct IntRange {
    pub path: &'static str,
    pub min: i64,
    pub max: Option<i64>,
}

const fn r(path: &'static str, min: i64, max: Option<i64>) -> IntRange {
    IntRange { path, min, max }
}

/// Obergrenze für `u64`-Werte: TOML-Ganzzahlen sind `i64`.
pub(crate) const TOML_INT_MAX: i64 = i64::MAX;

/// Wertebereiche aller Ganzzahl-Optionen mit Einschränkung (CRAWL-112: Threads 1–200; ONPAGE-144: 50–100 %).
pub(crate) const INT_RANGES: &[IntRange] = &[
    r("schema_version", 1, Some(u32::MAX as i64)),
    r("limits.max_urls", 1, Some(TOML_INT_MAX)),
    r("limits.max_page_kb", 1, Some(1 << 30)),
    r("speed.threads", 1, Some(200)),
    r("speed.max_per_host", 1, Some(200)),
    r("advanced.response_timeout_s", 1, Some(3600)),
    r("robots.retries", 0, Some(10)),
    r("robots.retry_delay_s", 0, Some(3600)),
    r("title.max_chars", 1, None),
    r("meta_description.max_chars", 1, None),
    r("url.max_chars", 1, None),
    r("h1.max_chars", 1, None),
    r("h2.max_chars", 1, None),
    r("image.max_kb", 1, None),
    r("image.alt_max_chars", 1, None),
    r("duplicates.near_threshold_pct", 50, Some(100)),
    r("validation.html_doc_max_bytes", 1, Some(TOML_INT_MAX)),
    r("validation.resource_max_bytes", 1, Some(TOML_INT_MAX)),
];

/// Grenzen der Ganzzahl-Option `path`, falls eingeschränkt.
pub(crate) fn int_range(path: &str) -> Option<IntRange> {
    INT_RANGES.iter().copied().find(|x| x.path == path)
}

/// „1–200“ bzw. „≥ 1“.
pub(crate) fn range_text(min: i64, max: Option<i64>) -> String {
    match max {
        Some(max) => format!("{min}–{max}"),
        None => format!("≥ {min}"),
    }
}

fn as_i128(v: &Value) -> Option<i128> {
    v.as_i64()
        .map(i128::from)
        .or_else(|| v.as_u64().map(i128::from))
}

impl CrawlConfig {
    /// Prüft die Konfiguration (CRAWL-140): Wertebereiche, `min ≤ max`, Regex-Syntax, Host-Auflösung,
    /// Basic-Auth-Einträge. Negative oder nicht numerische Werte kann es im Modell nicht geben; sie scheitern
    /// schon beim Laden bzw. in `set` als Typfehler.
    pub fn validate(&self) -> ValidationReport {
        let mut rep = ValidationReport::default();
        let json = serde_json::to_value(self).unwrap_or(Value::Null);

        for range in INT_RANGES {
            let ptr = format!("/{}", range.path.replace('.', "/"));
            let Some(value) = json.pointer(&ptr).and_then(as_i128) else {
                continue; // nicht gesetzte optionale Werte
            };
            let too_small = value < i128::from(range.min);
            let too_big = range.max.is_some_and(|m| value > i128::from(m));
            if too_small || too_big {
                let message = match range.max {
                    Some(max) => {
                        format!("muss zwischen {} und {max} liegen (ist {value})", range.min)
                    }
                    None => format!("muss mindestens {} sein (ist {value})", range.min),
                };
                rep.errors.push(ConfigIssue::new(
                    ConfigIssueKind::InvalidValue,
                    range.path,
                    message,
                ));
            }
        }

        if let Some(x) = self.speed.max_urls_per_sec {
            if !x.is_finite() || x <= 0.0 {
                rep.errors.push(ConfigIssue::new(
                    ConfigIssueKind::InvalidValue,
                    "speed.max_urls_per_sec",
                    format!("muss eine Zahl größer als 0 sein (ist {x})"),
                ));
            }
        }

        check_text_limits("title", &self.title, &mut rep);
        check_text_limits("meta_description", &self.meta_description, &mut rep);

        check_patterns("scope.include", &self.scope.include, &mut rep);
        check_patterns("scope.exclude", &self.scope.exclude, &mut rep);

        for (i, entry) in self.network.resolve.iter().enumerate() {
            if let Err(why) = check_resolve(entry) {
                rep.errors.push(ConfigIssue::new(
                    ConfigIssueKind::InvalidValue,
                    format!("network.resolve[{i}]"),
                    why,
                ));
            }
        }

        for (i, a) in self.auth.basic.iter().enumerate() {
            for (field, value) in [
                ("prefix", &a.prefix),
                ("username", &a.username),
                ("secret_ref", &a.secret_ref),
            ] {
                if value.trim().is_empty() {
                    rep.errors.push(ConfigIssue::new(
                        ConfigIssueKind::InvalidValue,
                        format!("auth.basic[{i}].{field}"),
                        "darf nicht leer sein",
                    ));
                }
            }
        }

        if self.user_agent.preset.trim().is_empty() {
            rep.errors.push(ConfigIssue::new(
                ConfigIssueKind::InvalidValue,
                "user_agent.preset",
                "darf nicht leer sein",
            ));
        }
        if let Some(ua) = &self.user_agent.http {
            if ua.trim().is_empty() || ua.chars().any(char::is_control) {
                rep.errors.push(ConfigIssue::new(
                    ConfigIssueKind::InvalidValue,
                    "user_agent.http",
                    "darf nicht leer sein und keine Steuerzeichen (z. B. Zeilenumbruch) enthalten",
                ));
            }
        }
        if let Some(t) = &self.user_agent.robots_token {
            if t.trim().is_empty() || t.chars().any(|c| c.is_control() || c.is_whitespace()) {
                rep.errors.push(ConfigIssue::new(
                    ConfigIssueKind::InvalidValue,
                    "user_agent.robots_token",
                    "darf nicht leer sein und keine Leerzeichen oder Steuerzeichen enthalten",
                ));
            }
        }

        // CRAWL-140 [P1]: Annehmen, aber warnen – ein Grenzwert über der Seitengröße ist nie erkennbar.
        let page_bytes = i128::from(self.limits.max_page_kb).saturating_mul(1024);
        for (path, value) in [
            (
                "validation.html_doc_max_bytes",
                self.validation.html_doc_max_bytes,
            ),
            (
                "validation.resource_max_bytes",
                self.validation.resource_max_bytes,
            ),
        ] {
            if i128::from(value) > page_bytes {
                rep.warnings.push(ConfigIssue::new(
                    ConfigIssueKind::Warning,
                    path,
                    "Grenzwert kann nicht erkannt werden, wenn Seiten vorher abgeschnitten werden \
                     (größer als limits.max_page_kb)",
                ));
            }
        }

        rep
    }
}

fn check_text_limits(name: &str, t: &TextLimits, rep: &mut ValidationReport) {
    if t.min_chars > t.max_chars {
        rep.errors.push(ConfigIssue::new(
            ConfigIssueKind::InvalidValue,
            format!("{name}.min_chars"),
            format!(
                "darf nicht größer sein als {name}.max_chars ({} > {})",
                t.min_chars, t.max_chars
            ),
        ));
    }
    if t.min_px > t.max_px {
        rep.errors.push(ConfigIssue::new(
            ConfigIssueKind::InvalidValue,
            format!("{name}.min_px"),
            format!(
                "darf nicht größer sein als {name}.max_px ({} > {})",
                t.min_px, t.max_px
            ),
        ));
    }
}

/// Prüft Regex-Syntax mit `fancy-regex`: roh und in der Vollübereinstimmungs-Form `^(?:…)$`.
fn check_patterns(path: &str, patterns: &[String], rep: &mut ValidationReport) {
    for (i, p) in patterns.iter().enumerate() {
        let message = match fancy_regex::Regex::new(p) {
            Err(e) => Some(format!("ungültiger regulärer Ausdruck: {e}")),
            Ok(_) => fancy_regex::Regex::new(&format!("^(?:{p})$"))
                .err()
                .map(|e| format!("als Vollübereinstimmung nicht verwendbar: {e}")),
        };
        if let Some(message) = message {
            rep.errors.push(ConfigIssue::new(
                ConfigIssueKind::InvalidValue,
                format!("{path}[{i}]"),
                message,
            ));
        }
    }
}

/// `host=ip[:port]`, `*.domain=ip[:port]` (CRAWL-123, K-11).
fn check_resolve(entry: &str) -> Result<(), String> {
    let Some((host, target)) = entry.split_once('=') else {
        return Err(format!("{entry:?}: Format host=ip[:port] erwartet"));
    };
    let host = host.trim();
    let name = host.strip_prefix("*.").unwrap_or(host);
    let host_ok = !name.is_empty()
        && !name.starts_with('.')
        && !name.ends_with('.')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_'));
    if !host_ok {
        return Err(format!(
            "{entry:?}: Host oder *.domain erwartet (ungültiger Host {host:?})"
        ));
    }
    let target = target.trim();
    let bare_v6 = target
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or(target);
    if bare_v6.parse::<IpAddr>().is_ok() || target.parse::<SocketAddr>().is_ok() {
        Ok(())
    } else {
        Err(format!(
            "{entry:?}: IP-Adresse (optional mit :Port) erwartet (ungültig: {target:?})"
        ))
    }
}
