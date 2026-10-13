//! Parsen und Prüfen des Katalogs (UI-063): gültige Werte, Regelcodes, Präfixe, `override_reason`.

use std::collections::{BTreeMap, BTreeSet};

use os_model::{IssuePriority, IssueType};
use serde::Deserialize;

use super::render;
use super::CatalogEntry;

/// Gültige Typ-Regeln (UI-063) mit dem Typ, den sie ergeben.
pub const TYPE_RULES: &[(&str, IssueType)] = &[
    ("T-I1", IssueType::Issue),
    ("T-I2", IssueType::Issue),
    ("T-I3", IssueType::Issue),
    ("T-I4", IssueType::Issue),
    ("T-W1", IssueType::Warning),
    ("T-W2", IssueType::Warning),
    ("T-W3", IssueType::Warning),
    ("T-O1", IssueType::Opportunity),
    ("T-O2", IssueType::Opportunity),
];

/// Gültige Prioritäts-Regeln (UI-063) mit der Priorität, die sie ergeben.
pub const PRIORITY_RULES: &[(&str, IssuePriority)] = &[
    ("P-H1", IssuePriority::High),
    ("P-H2", IssuePriority::High),
    ("P-H3", IssuePriority::High),
    ("P-M1", IssuePriority::Medium),
    ("P-M2", IssuePriority::Medium),
    ("P-L1", IssuePriority::Low),
    ("P-L2", IssuePriority::Low),
];

/// Erlaubte Anfänge einer `override_reason` (UI-063: u. a. `benchmark-parity`).
pub const OVERRIDE_REASON_PREFIXES: &[&str] = &["benchmark-parity"];

/// Erlaubte Platzhalter in `text_params` und in den Texten.
pub const ALLOWED_PLACEHOLDERS: &[&str] = &["max", "min", "unit", "threshold"];

/// Fehler beim Lesen der Katalogdatei.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// Kein gültiges TOML oder unzulässiger Wert (z. B. Typ außerhalb Issue/Warning/Opportunity).
    #[error("Katalog nicht lesbar: {0}")]
    Parse(String),
}

/// Ein Verstoß gegen die Katalogregeln.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProblem {
    /// Eintrag (`id`), bei Dateiproblemen leer.
    pub id: String,
    /// Beschreibung.
    pub message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    #[serde(default)]
    #[allow(dead_code)]
    schema: u32,
    #[serde(default)]
    issue: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    id: String,
    tab: String,
    filter: String,
    #[serde(rename = "type")]
    issue_type: IssueType,
    priority: IssuePriority,
    type_rule: String,
    priority_rule: String,
    #[serde(default)]
    override_reason: Option<String>,
    #[serde(default)]
    needs_crawl_analysis: bool,
    doc_key: String,
    #[serde(default)]
    text_params: BTreeMap<String, String>,
}

/// Liest einen Katalog aus TOML. Typ und Priorität müssen gültige Schlüssel sein.
pub fn parse_catalog(src: &str) -> Result<Vec<CatalogEntry>, CatalogError> {
    let raw: RawCatalog = toml::from_str(src).map_err(|e| CatalogError::Parse(e.to_string()))?;
    Ok(raw
        .issue
        .into_iter()
        .map(|r| CatalogEntry {
            id: r.id,
            tab: r.tab,
            filter: r.filter,
            issue_type: r.issue_type,
            priority: r.priority,
            type_rule: r.type_rule,
            priority_rule: r.priority_rule,
            override_reason: r.override_reason,
            needs_crawl_analysis: r.needs_crawl_analysis,
            doc_key: r.doc_key,
            text_params: r.text_params.into_iter().collect(),
        })
        .collect())
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Prüft die Katalogregeln aus UI-063; leere Liste = in Ordnung.
///
/// - `id`/`doc_key` sind Kleinbuchstaben-Kennungen und eindeutig, `(tab, filter)` ebenfalls,
/// - `type_rule`/`priority_rule` stammen aus den Tabellen von UI-063,
/// - passt der Präfix der Regel nicht zu Typ bzw. Priorität (`T-I*` ↔ Issue, `P-H*` ↔ High …), ist eine
///   `override_reason` Pflicht; umgekehrt braucht ein Eintrag ohne Abweichung keine,
/// - Platzhalter und Konfigurationspfade in `text_params` und im Filternamen sind gültig.
pub fn validate(entries: &[CatalogEntry]) -> Vec<CatalogProblem> {
    let mut problems = Vec::new();
    let mut add = |id: &str, message: String| {
        problems.push(CatalogProblem {
            id: id.to_string(),
            message,
        })
    };
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut docs = BTreeSet::new();
    for e in entries {
        if !is_slug(&e.id) {
            add(&e.id, "id muss aus a-z, 0-9 und - bestehen".into());
        }
        if !ids.insert(e.id.clone()) {
            add(&e.id, "doppelte id".into());
        }
        if !is_slug(&e.doc_key) {
            add(&e.id, "doc_key muss aus a-z, 0-9 und - bestehen".into());
        }
        if !docs.insert(e.doc_key.clone()) {
            add(&e.id, "doppelter doc_key".into());
        }
        if !keys.insert((e.tab.to_lowercase(), e.filter.to_lowercase())) {
            add(&e.id, "Tab und Filter kommen doppelt vor".into());
        }

        let rule_type = TYPE_RULES
            .iter()
            .find(|(c, _)| *c == e.type_rule)
            .map(|(_, t)| *t);
        let rule_prio = PRIORITY_RULES
            .iter()
            .find(|(c, _)| *c == e.priority_rule)
            .map(|(_, p)| *p);
        if rule_type.is_none() {
            add(&e.id, format!("unbekannte type_rule {:?}", e.type_rule));
        }
        if rule_prio.is_none() {
            add(
                &e.id,
                format!("unbekannte priority_rule {:?}", e.priority_rule),
            );
        }
        let deviates = rule_type.is_some_and(|t| t != e.issue_type)
            || rule_prio.is_some_and(|p| p != e.priority);
        match (&e.override_reason, deviates) {
            (None, true) => add(
                &e.id,
                format!(
                    "Typ/Priorität {}/{} weichen von den Regeln {}/{} ab: override_reason fehlt",
                    e.issue_type, e.priority, e.type_rule, e.priority_rule
                ),
            ),
            (Some(reason), true) => {
                if !OVERRIDE_REASON_PREFIXES
                    .iter()
                    .any(|p| reason.starts_with(p))
                {
                    add(
                        &e.id,
                        format!("override_reason {reason:?} nennt keine erlaubte Begründung"),
                    );
                }
            }
            (Some(_), false) => add(
                &e.id,
                "override_reason ohne Abweichung von den Regeln".into(),
            ),
            (None, false) => {}
        }

        for (placeholder, path) in &e.text_params {
            if !ALLOWED_PLACEHOLDERS.contains(&placeholder.as_str()) {
                add(&e.id, format!("unzulässiger Platzhalter {{{placeholder}}}"));
            }
            if render::default_value(path).is_none() {
                add(
                    &e.id,
                    format!("Konfigurationspfad {path:?} existiert nicht oder ist kein Skalar"),
                );
            }
        }
        for path in render::filter_paths(&e.filter) {
            if render::default_value(&path).is_none() {
                add(
                    &e.id,
                    format!("Filtername verweist auf unbekannten Pfad {path:?}"),
                );
            }
        }
    }
    problems
}

/// Anteil der Einträge mit `override_reason` (UI-063: Ziel < 10 %).
pub fn override_share(entries: &[CatalogEntry]) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    let n = entries
        .iter()
        .filter(|e| e.override_reason.is_some())
        .count();
    n as f64 / entries.len() as f64
}
