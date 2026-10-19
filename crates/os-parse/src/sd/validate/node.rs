//! Lesen der gemeinsamen Elementdarstellung (JSON-LD-artige Objekte mit `_os_*`-Metadaten, siehe `sd::model`).

use serde_json::{Map, Value};

use super::vocab::Vocab;

/// Herkunft einer Typ- oder Eigenschafts-IRI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Iri<'a> {
    /// schema.org; Name ohne Präfix (kurze Namen ohne Präfix gelten als schema.org).
    Schema(&'a str),
    /// Das abgekündigte Vokabular `data-vocabulary.org`.
    DataVocabulary,
    /// Anderes Vokabular (eigenes Präfix, fremde IRI): wird nicht geprüft.
    Other,
}

/// Ordnet eine Typ- oder Eigenschaftsangabe einem Vokabular zu.
pub fn classify(raw: &str) -> Iri<'_> {
    let raw = raw.trim();
    for prefix in [
        "https://schema.org/",
        "http://schema.org/",
        "https://www.schema.org/",
        "http://www.schema.org/",
        "schema:",
    ] {
        if let Some(name) = raw.strip_prefix(prefix) {
            return Iri::Schema(name.trim_end_matches('/'));
        }
    }
    for prefix in [
        "https://data-vocabulary.org/",
        "http://data-vocabulary.org/",
        "https://www.data-vocabulary.org/",
        "http://www.data-vocabulary.org/",
    ] {
        if raw.starts_with(prefix) {
            return Iri::DataVocabulary;
        }
    }
    if raw.contains(':') || raw.contains('/') {
        Iri::Other
    } else {
        Iri::Schema(raw)
    }
}

/// Die `@type`-Angaben eines Knotens in Quelltextreihenfolge.
pub fn raw_types(node: &Map<String, Value>) -> Vec<&str> {
    match node.get("@type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// Die bekannten schema.org-Typen eines Knotens (kanonische Namen).
pub fn schema_types<'v>(
    node: &Map<String, Value>,
    vocab: &'v Vocab,
    case_sensitive: bool,
) -> Vec<&'v str> {
    raw_types(node)
        .into_iter()
        .filter_map(|t| match classify(t) {
            Iri::Schema(name) => vocab.type_name(name, case_sensitive),
            _ => None,
        })
        .collect()
}

/// Eigenschaften eines Knotens in Quelltextreihenfolge (`_os_order`, danach übrige Schlüssel sortiert); ohne
/// JSON-LD-Schlüsselwörter (`@…`) und reservierte Metadaten (`_os_…`).
pub fn properties(node: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut keys: Vec<&str> = node
        .get("_os_order")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut rest: Vec<&str> = node
        .keys()
        .map(String::as_str)
        .filter(|k| !keys.contains(k))
        .collect();
    rest.sort_unstable();
    keys.extend(rest);
    keys.into_iter()
        .filter(|k| !k.starts_with('@') && !k.starts_with("_os_"))
        .filter_map(|k| node.get(k).map(|v| (k, v)))
        .collect()
}

/// Die Einzelwerte einer Eigenschaft und ob sie als Liste angegeben ist (`@list` wird aufgelöst).
pub fn values(v: &Value) -> (Vec<&Value>, bool) {
    match v {
        Value::Array(a) => (a.iter().flat_map(|v| values(v).0).collect(), true),
        Value::Object(o) => match o.get("@list") {
            Some(Value::Array(a)) => (a.iter().collect(), true),
            _ => (vec![v], false),
        },
        _ => (vec![v], false),
    }
}

/// Ein Objektknoten (kein `@value`-Literal).
pub fn as_node(v: &Value) -> Option<&Map<String, Value>> {
    v.as_object().filter(|o| !o.contains_key("@value"))
}

/// Der Literalwert (Text, Zahl, Wahrheitswert oder `{"@value": …}`).
pub fn literal(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(o) => o.get("@value").filter(|v| !v.is_object() && !v.is_array()),
        Value::Array(_) | Value::Null => None,
        v => Some(v),
    }
}

/// Pfad einer Eigenschaft ab dem Top-Level-Item (`offers[0].priceCurrency`).
pub fn path(prefix: &str, key: &str, index: Option<usize>) -> String {
    match index {
        Some(i) => format!("{prefix}{key}[{i}]"),
        None => format!("{prefix}{key}"),
    }
}
