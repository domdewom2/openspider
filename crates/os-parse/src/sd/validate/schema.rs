//! Prüfung gegen das schema.org-Vokabular (TECH-187).
//!
//! Jeder Knoten eines Elements wird geprüft, auch verschachtelte (nach der `@id`-Auflösung der Extraktion). Befunde:
//! unbekannter Typ, unbekannte Eigenschaft, Eigenschaft nicht für den Typ (Domain inkl. Vererbung), Objektwert
//! außerhalb der Range, Literal mit falscher Syntax bei reiner Nicht-Text-Datentyp-Range; Warnung für Knoten im
//! Vokabular `data-vocabulary.org`. Eigenschaften fremder Vokabulare bleiben ohne Befund.

use serde_json::{Map, Value};

use super::node::{self, Iri};
use super::values;
use super::vocab::Vocab;
use super::{Hit, MAX_DEPTH};

pub const UNKNOWN_TYPE: &str = "sd.schema.unknown_type";
pub const UNKNOWN_PROPERTY: &str = "sd.schema.unknown_property";
pub const PROPERTY_NOT_ALLOWED: &str = "sd.schema.property_not_allowed";
pub const VALUE_TYPE: &str = "sd.schema.value_type";
pub const VALUE_FORMAT: &str = "sd.schema.value_format";
pub const DEPRECATED_VOCABULARY: &str = "sd.schema.deprecated_vocabulary";

/// Datentypen, deren Literalsyntax geprüft wird; jeder andere Range-Eintrag (Text, URL, Klassen …) lässt jeden
/// Text zu.
const CHECKED_DATATYPES: [&str; 7] = [
    "Boolean", "Date", "DateTime", "Float", "Integer", "Number", "Time",
];

struct Checker<'a> {
    vocab: &'a Vocab,
    case_sensitive: bool,
    hits: Vec<Hit>,
}

/// Prüft ein Element (Wurzelknoten).
pub fn check(root: &Value, vocab: &Vocab, case_sensitive: bool) -> Vec<Hit> {
    let mut c = Checker {
        vocab,
        case_sensitive,
        hits: Vec::new(),
    };
    if let Some(node) = node::as_node(root) {
        c.node(node, "", None, 0);
    }
    c.hits
}

impl Checker<'_> {
    fn hit(&mut self, severity: &'static str, code: &'static str, path: String) {
        self.hits.push(Hit {
            severity,
            code,
            path,
            feature: None,
        });
    }

    /// `role_of`: Name der Eigenschaft, über die ein `Role`-Knoten erreicht wurde (sie ist an ihm erlaubt).
    fn node(
        &mut self,
        node: &Map<String, Value>,
        prefix: &str,
        role_of: Option<&str>,
        depth: usize,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let raw = node::raw_types(node);
        let mut known = Vec::new();
        let mut legacy = false;
        let mut schema_node = false;
        for (i, t) in raw.iter().enumerate() {
            let iri = node::classify(t);
            schema_node |= matches!(iri, Iri::Schema(_));
            match iri {
                Iri::Schema(name) => match self.vocab.type_name(name, self.case_sensitive) {
                    Some(name) => known.push(name),
                    None => {
                        let index = (raw.len() > 1).then_some(i);
                        self.hit("error", UNKNOWN_TYPE, node::path(prefix, "@type", index));
                    }
                },
                Iri::DataVocabulary => legacy = true,
                Iri::Other => {}
            }
        }
        if legacy {
            self.hit(
                "warning",
                DEPRECATED_VOCABULARY,
                node::path(prefix, "@type", None),
            );
        }
        for (key, value) in node::properties(node) {
            let prop = match node::classify(key) {
                // Aktions-Annotationen `<Eigenschaft>-input`/`-output` (schema.org Actions) gelten als gültig.
                Iri::Schema(name) if schema_node && annotation(name).is_some() => {
                    let base = annotation(name).unwrap_or(name);
                    if self.vocab.property(base, self.case_sensitive).is_none() {
                        self.hit("error", UNKNOWN_PROPERTY, node::path(prefix, key, None));
                    }
                    None
                }
                Iri::Schema(name) if schema_node => {
                    match self.vocab.property(name, self.case_sensitive) {
                        Some(p) => Some(p),
                        None => {
                            self.hit("error", UNKNOWN_PROPERTY, node::path(prefix, key, None));
                            None
                        }
                    }
                }
                _ => None,
            };
            if let Some((name, p)) = prop.filter(|_| !known.is_empty()) {
                let allowed = role_of == Some(name)
                    || known
                        .iter()
                        .any(|t| p.domain.iter().any(|d| self.vocab.is_a(t, d)));
                if !allowed {
                    self.hit("error", PROPERTY_NOT_ALLOWED, node::path(prefix, key, None));
                }
            }
            let (items, list) = node::values(value);
            for (i, v) in items.into_iter().enumerate() {
                let index = list.then_some(i);
                if let Some(child) = node::as_node(v) {
                    let child_types = node::schema_types(child, self.vocab, self.case_sensitive);
                    let role = child_types.iter().any(|t| self.vocab.is_a(t, "Role"));
                    if let Some((_, p)) = prop {
                        let fits = child_types
                            .iter()
                            .any(|t| p.range.iter().any(|r| self.vocab.is_a(t, r)));
                        if !child_types.is_empty() && !fits && !role {
                            self.hit("error", VALUE_TYPE, node::path(prefix, key, index));
                        }
                    }
                    let child_prefix = format!("{}.", node::path(prefix, key, Some(i)));
                    let via = prop.filter(|_| role).map(|(name, _)| name);
                    self.node(child, &child_prefix, via, depth + 1);
                } else if let (Some((_, p)), Some(lit)) = (prop, node::literal(v)) {
                    if !self.literal_fits(lit, &p.range) {
                        self.hit("error", VALUE_FORMAT, node::path(prefix, key, index));
                    }
                }
            }
        }
    }

    /// Ein Literal passt, wenn die Range einen ungeprüften Eintrag enthält oder einer der geprüften Datentypen passt.
    fn literal_fits(&self, v: &Value, range: &[String]) -> bool {
        if range.is_empty()
            || range
                .iter()
                .any(|r| !CHECKED_DATATYPES.contains(&r.as_str()) || !self.vocab.is_datatype(r))
        {
            return true;
        }
        let text = v.as_str().map(str::trim);
        range.iter().any(|r| match r.as_str() {
            "Boolean" => values::is_boolean(v),
            "Integer" => values::is_integer(v),
            "Number" | "Float" => values::is_number(v),
            "Date" | "DateTime" => text.is_some_and(values::is_iso_date),
            "Time" => text.is_some_and(values::is_time),
            _ => true,
        })
    }
}

/// Basisname einer Aktions-Annotation (`query-input` → `query`).
fn annotation(name: &str) -> Option<&str> {
    name.strip_suffix("-input")
        .or_else(|| name.strip_suffix("-output"))
}
