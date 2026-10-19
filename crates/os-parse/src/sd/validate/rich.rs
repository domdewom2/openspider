//! Prüfung gegen die Rich-Result-Regeldatei (TECH-188, `resources/rich-results/rules.json`).
//!
//! Ein Feature wird von Top-Level-Knoten seiner Typen (inkl. Untertypen) ausgelöst und von verschachtelten Knoten,
//! deren direkter Elternknoten einen der Typen aus `nested_in` hat. Abgekündigte Features (`deprecated_since`)
//! werden weder erkannt noch geprüft. Dieselbe Eigenschaft wird je Feature höchstens einmal gemeldet.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::node;
use super::values;
use super::vocab::Vocab;
use super::{Hit, MAX_DEPTH};

pub const MISSING_REQUIRED: &str = "sd.google.missing_required";
pub const MISSING_RECOMMENDED: &str = "sd.google.missing_recommended";
pub const INVALID_VALUE: &str = "sd.google.invalid_value";

const SOURCE: &str = include_str!("../../../resources/rich-results/rules.json");

/// Die Regeldatei.
#[derive(Deserialize)]
pub struct Rules {
    /// Version der Regeldatei, z. B. `2026.10.1`.
    pub version: String,
    enums: HashMap<String, Vec<String>>,
    /// Features in Dateireihenfolge.
    pub features: Vec<Feature>,
}

/// Ein Rich-Result-Feature.
#[derive(Deserialize)]
pub struct Feature {
    /// Anzeigename, z. B. `Product snippet`.
    pub name: String,
    /// Dokumentation des Features.
    #[serde(default)]
    pub doc: Option<String>,
    /// Abgekündigt seit (Jahr-Monat); gesetzt = nicht erkannt und nicht geprüft.
    pub deprecated_since: Option<String>,
    types: Vec<String>,
    #[serde(default)]
    with: Option<String>,
    #[serde(default)]
    nested_in: Vec<String>,
    checks: HashMap<String, Check>,
    values: HashMap<String, String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Check {
    required: Vec<String>,
    recommended: Vec<String>,
    one_of: Vec<Vec<String>>,
    amp_required: Vec<String>,
    required_top_level: Vec<String>,
    required_unless_last: Vec<String>,
    follow: Vec<String>,
}

/// Die Regeln dieses Builds.
pub fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| {
        serde_json::from_str(SOURCE).expect("Rich-Result-Regeldatei ist gültiges JSON")
    })
}

/// Seiteneigenschaften, die die Prüfung beeinflussen.
#[derive(Clone, Copy)]
pub struct Page {
    /// AMP-Dokument (`<html amp>`/`<html ⚡>`): AMP-Pflichteigenschaften von Article gelten.
    pub amp: bool,
    pub case_sensitive: bool,
}

struct Checker<'a> {
    vocab: &'a Vocab,
    rules: &'a Rules,
    page: Page,
    hits: Vec<Hit>,
    seen: HashSet<(usize, &'static str, String)>,
    features: Vec<usize>,
}

/// Prüft ein Element; liefert die Befunde und die erkannten Features (Indizes in [`Rules::features`]).
pub fn check(root: &Value, vocab: &Vocab, rules: &Rules, page: Page) -> (Vec<Hit>, Vec<usize>) {
    let mut c = Checker {
        vocab,
        rules,
        page,
        hits: Vec::new(),
        seen: HashSet::new(),
        features: Vec::new(),
    };
    if let Some(node) = node::as_node(root) {
        c.walk(node, "", &[], true, 0);
    }
    (c.hits, c.features)
}

/// Ein Eigenschaftswert gilt als vorhanden, wenn er weder `null` noch leerer Text noch leere Liste ist.
fn present(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => a.iter().any(present),
        _ => true,
    }
}

impl<'a> Checker<'a> {
    fn types(&self, node: &Map<String, Value>) -> Vec<&'a str> {
        node::schema_types(node, self.vocab, self.page.case_sensitive)
    }

    fn is_any(&self, types: &[&str], of: &[String]) -> bool {
        types
            .iter()
            .any(|t| of.iter().any(|o| self.vocab.is_a(t, o)))
    }

    /// Eigenschaft eines Knotens nach kanonischem Namen (ohne Groß-/Kleinschreibung, wenn so eingestellt).
    fn get<'n>(&self, node: &'n Map<String, Value>, name: &str) -> Option<(&'n str, &'n Value)> {
        node::properties(node)
            .into_iter()
            .find(|(key, _)| match node::classify(key) {
                node::Iri::Schema(k) => {
                    k == name || (!self.page.case_sensitive && k.eq_ignore_ascii_case(name))
                }
                _ => false,
            })
    }

    fn walk(
        &mut self,
        node: &Map<String, Value>,
        prefix: &str,
        parent: &[&str],
        top: bool,
        depth: usize,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let types = self.types(node);
        for (fi, feature) in self.rules.features.iter().enumerate() {
            if feature.deprecated_since.is_some() || !self.is_any(&types, &feature.types) {
                continue;
            }
            if !top && !self.is_any(parent, &feature.nested_in) {
                continue;
            }
            if let Some(with) = &feature.with {
                if !self.get(node, with).is_some_and(|(_, v)| present(v)) {
                    continue;
                }
            }
            if !self.features.contains(&fi) {
                self.features.push(fi);
            }
            self.apply(fi, node, prefix, top, None, depth);
        }
        for (key, value) in node::properties(node) {
            for (i, v) in node::values(value).0.into_iter().enumerate() {
                if let Some(child) = node::as_node(v) {
                    let child_prefix = format!("{}.", node::path(prefix, key, Some(i)));
                    self.walk(child, &child_prefix, &types, false, depth + 1);
                }
            }
        }
    }

    /// Die Prüfregel eines Features für die Typen eines Knotens (der speziellste passende Eintrag).
    fn rule_for(&self, fi: usize, types: &[&str]) -> Option<&'a Check> {
        let feature = &self.rules.features[fi];
        feature
            .checks
            .iter()
            .filter(|(name, _)| types.iter().any(|t| self.vocab.is_a(t, name)))
            .max_by_key(|(name, _)| (self.vocab.depth(name), std::cmp::Reverse(name.as_str())))
            .map(|(_, check)| check)
    }

    fn hit(&mut self, fi: usize, severity: &'static str, code: &'static str, path: String) {
        if self.seen.insert((fi, code, path.clone())) {
            self.hits.push(Hit {
                severity,
                code,
                path,
                feature: Some(fi),
            });
        }
    }

    /// `position`: Index und „letztes Element“ in einer über `follow` erreichten Liste.
    fn apply(
        &mut self,
        fi: usize,
        node: &Map<String, Value>,
        prefix: &str,
        top: bool,
        position: Option<(usize, bool)>,
        depth: usize,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let types = self.types(node);
        let Some(check) = self.rule_for(fi, &types) else {
            return;
        };
        let last = position.is_some_and(|(_, last)| last);
        let mut required: Vec<&String> = check.required.iter().collect();
        let mut recommended: Vec<&String> = check.recommended.iter().collect();
        if top {
            required.extend(&check.required_top_level);
        }
        if !last {
            required.extend(&check.required_unless_last);
        }
        if self.page.amp {
            required.extend(&check.amp_required);
            recommended.retain(|p| !check.amp_required.contains(p));
        } else {
            recommended.extend(
                check
                    .amp_required
                    .iter()
                    .filter(|p| !check.recommended.contains(p)),
            );
        }
        for name in required {
            if !self.validate(fi, node, prefix, name) {
                self.hit(
                    fi,
                    "error",
                    MISSING_REQUIRED,
                    node::path(prefix, name, None),
                );
            }
        }
        for group in &check.one_of {
            let mut any = false;
            for name in group {
                any |= self.validate(fi, node, prefix, name);
            }
            if !any {
                self.hit(
                    fi,
                    "error",
                    MISSING_REQUIRED,
                    format!("{prefix}{}", group.join("|")),
                );
            }
        }
        for name in recommended {
            if !self.validate(fi, node, prefix, name) {
                self.hit(
                    fi,
                    "warning",
                    MISSING_RECOMMENDED,
                    node::path(prefix, name, None),
                );
            }
        }
        for name in &check.follow {
            let Some((key, value)) = self.get(node, name) else {
                continue;
            };
            let children: Vec<_> = node::values(value)
                .0
                .into_iter()
                .filter_map(node::as_node)
                .collect();
            let n = children.len();
            for (i, child) in children.into_iter().enumerate() {
                let child_prefix = format!("{}.", node::path(prefix, key, Some(i)));
                self.apply(
                    fi,
                    child,
                    &child_prefix,
                    false,
                    Some((i, i + 1 == n)),
                    depth + 1,
                );
            }
        }
    }

    /// Prüft das Format einer vorhandenen Eigenschaft (Befund `invalid_value`); `false` = fehlt.
    fn validate(&mut self, fi: usize, node: &Map<String, Value>, prefix: &str, name: &str) -> bool {
        let Some((key, value)) = self.get(node, name) else {
            return false;
        };
        if !present(value) {
            return false;
        }
        let feature = &self.rules.features[fi];
        if let Some(kind) = feature.values.get(name) {
            let (items, list) = node::values(value);
            for (i, v) in items.into_iter().enumerate() {
                if !self.fits(kind, v) {
                    self.hit(
                        fi,
                        "error",
                        INVALID_VALUE,
                        node::path(prefix, key, list.then_some(i)),
                    );
                }
            }
        }
        true
    }

    fn fits(&self, kind: &str, v: &Value) -> bool {
        let text = node::literal(v).and_then(|l| match l {
            Value::String(s) => Some(s.trim().to_owned()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        });
        let node_url = || {
            node::as_node(v).and_then(|o| {
                ["url", "contentUrl", "@id"]
                    .iter()
                    .find_map(|k| o.get(*k).and_then(Value::as_str))
            })
        };
        match kind {
            "text" => text.is_some_and(|t| !t.is_empty()),
            "number" => node::literal(v).is_some_and(values::is_number),
            "integer" => node::literal(v).is_some_and(values::is_integer),
            "currency" => text.is_some_and(|t| values::is_currency(&t)),
            "duration" => text.is_some_and(|t| values::is_duration(&t)),
            "date" => text.is_some_and(|t| values::is_full_date(&t) || values::is_datetime(&t)),
            "url" => match text {
                Some(t) => values::is_url(&t),
                None => node::as_node(v)
                    .and_then(|o| o.get("@id").or_else(|| o.get("url")))
                    .and_then(Value::as_str)
                    .is_some_and(values::is_url),
            },
            "image" => match text {
                Some(t) => values::is_url(&t),
                None => node_url().is_some_and(values::is_url),
            },
            enumeration => {
                let Some(list) = enumeration
                    .strip_prefix("enum:")
                    .and_then(|e| self.rules.enums.get(e))
                else {
                    return true;
                };
                let raw = text.or_else(|| node_url().map(str::to_owned));
                raw.is_some_and(|t| match node::classify(&t) {
                    node::Iri::Schema(name) => list.iter().any(|e| e == name),
                    _ => false,
                })
            }
        }
    }
}
