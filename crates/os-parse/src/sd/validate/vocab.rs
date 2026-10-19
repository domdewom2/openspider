//! Versioniertes schema.org-Vokabular (TECH-187): Typen mit Oberklassen, Datentypen, Domain und Range.
//!
//! Quelle ist das Datenasset `resources/schemaorg/vocabulary.json` (CC BY-SA 3.0, siehe `LICENSE.md` dort); es wird
//! einmal je Prozess gelesen.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

const SOURCE: &str = include_str!("../../../resources/schemaorg/vocabulary.json");

#[derive(Deserialize)]
struct Raw {
    version: String,
    datatypes: Vec<String>,
    types: HashMap<String, Vec<String>>,
    properties: HashMap<String, (Vec<String>, Vec<String>)>,
}

/// Eine Eigenschaft: Typen, an denen sie stehen darf, und erlaubte Werttypen.
pub struct Property {
    pub domain: Vec<String>,
    pub range: Vec<String>,
}

/// Das geladene Vokabular.
pub struct Vocab {
    /// schema.org-Version, z. B. `30.1`.
    pub version: String,
    datatypes: HashSet<String>,
    /// Typ → der Typ selbst und alle Oberklassen.
    ancestors: HashMap<String, HashSet<String>>,
    properties: HashMap<String, Property>,
    types_lower: HashMap<String, String>,
    properties_lower: HashMap<String, String>,
}

/// Das Vokabular dieses Builds.
pub fn get() -> &'static Vocab {
    static VOCAB: OnceLock<Vocab> = OnceLock::new();
    VOCAB.get_or_init(|| Vocab::parse(SOURCE))
}

impl Vocab {
    fn parse(source: &str) -> Vocab {
        let raw: Raw =
            serde_json::from_str(source).expect("schema.org-Vokabular ist gültiges JSON");
        let mut ancestors = HashMap::new();
        for name in raw.types.keys() {
            let mut seen = HashSet::new();
            let mut stack = vec![name.as_str()];
            while let Some(t) = stack.pop() {
                if seen.insert(t.to_owned()) {
                    stack.extend(raw.types.get(t).into_iter().flatten().map(String::as_str));
                }
            }
            ancestors.insert(name.clone(), seen);
        }
        Vocab {
            version: raw.version,
            datatypes: raw.datatypes.into_iter().collect(),
            types_lower: raw
                .types
                .keys()
                .map(|k| (k.to_lowercase(), k.clone()))
                .collect(),
            properties_lower: raw
                .properties
                .keys()
                .map(|k| (k.to_lowercase(), k.clone()))
                .collect(),
            properties: raw
                .properties
                .into_iter()
                .map(|(k, (domain, range))| (k, Property { domain, range }))
                .collect(),
            ancestors,
        }
    }

    /// Kanonischer Typname; ohne Groß-/Kleinschreibung wird `product` zu `Product`.
    pub fn type_name(&self, name: &str, case_sensitive: bool) -> Option<&str> {
        if let Some((k, _)) = self.ancestors.get_key_value(name) {
            return Some(k);
        }
        if case_sensitive {
            return None;
        }
        self.types_lower
            .get(&name.to_lowercase())
            .map(String::as_str)
    }

    /// Kanonischer Eigenschaftsname und Definition.
    pub fn property(&self, name: &str, case_sensitive: bool) -> Option<(&str, &Property)> {
        if let Some((k, p)) = self.properties.get_key_value(name) {
            return Some((k, p));
        }
        if case_sensitive {
            return None;
        }
        let k = self.properties_lower.get(&name.to_lowercase())?;
        self.properties
            .get_key_value(k.as_str())
            .map(|(k, p)| (k.as_str(), p))
    }

    /// `ty` ist `ancestor` oder ein Untertyp davon (beide kanonisch).
    pub fn is_a(&self, ty: &str, ancestor: &str) -> bool {
        self.ancestors
            .get(ty)
            .is_some_and(|set| set.contains(ancestor))
    }

    /// Anzahl der Obertypen (inklusive des Typs selbst); größer = spezieller.
    pub fn depth(&self, ty: &str) -> usize {
        self.ancestors.get(ty).map_or(0, HashSet::len)
    }

    /// `ty` ist ein Datentyp (Text, Number, Date …).
    pub fn is_datatype(&self, ty: &str) -> bool {
        self.datatypes.contains(ty)
    }
}
