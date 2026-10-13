//! Spaltenmapping SF → OS (`tests/parity/mappings/<export-slug>.toml`, Abschnitt 5.2).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::ParityError;
use crate::normalize::{BoolMode, CellRules};

/// Name der laufenden Nummer im Schlüssel (Link-Exporte mit gleichem Quelle/Ziel/Typ).
pub const ORDINAL_KEY: &str = "#";

/// Schalter für Wahrheitswerte: `"case-insensitive"`, `"loose"`, `"off"` oder ein Boolean.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawBool {
    Flag(bool),
    Text(String),
}

impl RawBool {
    fn mode(&self, flag_true: BoolMode) -> Result<BoolMode, ParityError> {
        match self {
            RawBool::Flag(true) => Ok(flag_true),
            RawBool::Flag(false) => Ok(BoolMode::Off),
            RawBool::Text(t) => match t.trim().to_lowercase().as_str() {
                "case-insensitive" | "case_insensitive" | "ci" => Ok(BoolMode::CaseInsensitive),
                "loose" => Ok(BoolMode::Loose),
                "off" | "strict" | "none" => Ok(BoolMode::Off),
                other => Err(ParityError::Invalid(format!(
                    "unbekannter Wert für bool: {other:?}"
                ))),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawColumnOpts {
    os: Option<String>,
    #[serde(default)]
    ignore: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    map: BTreeMap<String, String>,
    tolerance_pct: Option<f64>,
    number_decimals: Option<u32>,
    #[serde(default)]
    bool: Option<RawBool>,
    empty_equals_null: Option<bool>,
    trim: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawColumn {
    Name(String),
    Opts(RawColumnOpts),
}

#[derive(Debug, Clone, Deserialize)]
struct RawNormalize {
    #[serde(default = "yes")]
    base_url: bool,
    #[serde(default = "yes")]
    trim: bool,
    #[serde(default)]
    bool: Option<RawBool>,
    #[serde(default)]
    number_decimals: Option<u32>,
    #[serde(default)]
    empty_equals_null: bool,
}

impl Default for RawNormalize {
    fn default() -> Self {
        Self {
            base_url: true,
            trim: true,
            bool: None,
            number_decimals: None,
            empty_equals_null: false,
        }
    }
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct RawMapping {
    #[serde(default)]
    key: Vec<String>,
    #[serde(default)]
    drop_sf_columns: Vec<String>,
    #[serde(default)]
    columns: BTreeMap<String, RawColumn>,
    #[serde(default)]
    normalize: RawNormalize,
}

/// Eine Spalte des Mappings.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnMap {
    /// Spaltenname in der Referenz.
    pub sf: String,
    /// Spaltenname in OpenSpider (bei `ignore` ohne Bedeutung).
    pub os: String,
    /// Spalte wird nicht verglichen.
    pub ignore: bool,
    /// Begründung der Ignorierung.
    pub reason: Option<String>,
    /// Aufgelöste Vergleichsregeln.
    pub rules: CellRules,
}

/// Geladenes Mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    /// Schlüsselspalten (Referenznamen); `"#"` = laufende Nummer.
    pub key: Vec<String>,
    /// Zu verwerfende Referenzspalten.
    pub drop_sf_columns: Vec<String>,
    /// Spalten in alphabetischer Reihenfolge der Referenznamen.
    pub columns: Vec<ColumnMap>,
    /// Regeln für Schlüsselspalten und nicht explizit aufgeführte Spalten.
    pub defaults: CellRules,
}

impl Mapping {
    /// Lädt ein Mapping aus einer TOML-Datei.
    pub fn load(path: &Path) -> Result<Self, ParityError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            ParityError::Invalid(format!("Mapping {} nicht lesbar: {e}", path.display()))
        })?;
        Self::parse(&text)
            .map_err(|e| ParityError::Invalid(format!("Mapping {}: {e}", path.display())))
    }

    /// Parst ein Mapping aus TOML-Text.
    pub fn parse(text: &str) -> Result<Self, ParityError> {
        let raw: RawMapping =
            toml::from_str(text).map_err(|e| ParityError::Invalid(e.to_string()))?;
        if raw.key.is_empty() {
            return Err(ParityError::Invalid(
                "`key` darf nicht leer sein (z. B. key = [\"Address\"])".into(),
            ));
        }
        let n = &raw.normalize;
        let defaults = CellRules {
            trim: n.trim,
            base_url: n.base_url,
            bool_mode: match &n.bool {
                Some(b) => b.mode(BoolMode::CaseInsensitive)?,
                None => BoolMode::Off,
            },
            number_decimals: n.number_decimals,
            tolerance_pct: None,
            empty_equals_null: n.empty_equals_null,
            map: BTreeMap::new(),
        };
        let mut columns = Vec::new();
        for (sf, col) in raw.columns {
            let cm = match col {
                RawColumn::Name(os) => ColumnMap {
                    sf,
                    os,
                    ignore: false,
                    reason: None,
                    rules: defaults.clone(),
                },
                RawColumn::Opts(o) => {
                    let mut rules = defaults.clone();
                    if let Some(t) = o.trim {
                        rules.trim = t;
                    }
                    if let Some(b) = &o.bool {
                        rules.bool_mode = b.mode(BoolMode::Loose)?;
                    }
                    if o.number_decimals.is_some() {
                        rules.number_decimals = o.number_decimals;
                    }
                    rules.tolerance_pct = o.tolerance_pct;
                    if let Some(e) = o.empty_equals_null {
                        rules.empty_equals_null = e;
                    }
                    rules.map = o.map;
                    ColumnMap {
                        os: o.os.unwrap_or_else(|| sf.clone()),
                        sf,
                        ignore: o.ignore,
                        reason: o.reason,
                        rules,
                    }
                }
            };
            columns.push(cm);
        }
        Ok(Self {
            key: raw.key,
            drop_sf_columns: raw.drop_sf_columns,
            columns,
            defaults,
        })
    }

    /// Spalte für einen Referenznamen.
    pub fn column(&self, sf: &str) -> Option<&ColumnMap> {
        self.columns.iter().find(|c| c.sf == sf)
    }

    /// OpenSpider-Name einer Schlüsselspalte (ohne Eintrag unverändert).
    pub fn os_name_of_key(&self, key: &str) -> String {
        self.column(key)
            .map(|c| c.os.clone())
            .unwrap_or_else(|| key.to_string())
    }

    /// Regeln für eine Schlüsselspalte.
    pub fn key_rules(&self, key: &str) -> CellRules {
        self.column(key)
            .map(|c| c.rules.clone())
            .unwrap_or_else(|| self.defaults.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
key = ["Address"]
drop_sf_columns = ["Description", "How To Fix"]
[columns]
"Address" = "Address"
"Status Code" = "Status Code"
"Title 1 Pixel Width" = { os = "Title 1 Pixel Width", tolerance_pct = 3.0 }
"Response Time" = { ignore = true, reason = "nicht deterministisch" }
"Indexability Status" = { os = "Indexability Status", map = { "Canonicalised" = "Canonicalised" } }
"Flag" = { bool = true }
[normalize]
base_url = true
trim = true
bool = "case-insensitive"
number_decimals = 3
empty_equals_null = true
"#;

    #[test]
    fn parses_columns_options_and_defaults() {
        let m = Mapping::parse(SAMPLE).unwrap();
        assert_eq!(m.key, ["Address"]);
        assert_eq!(m.drop_sf_columns, ["Description", "How To Fix"]);
        assert_eq!(m.columns.len(), 6);
        let w = m.column("Title 1 Pixel Width").unwrap();
        assert_eq!(w.rules.tolerance_pct, Some(3.0));
        assert_eq!(w.rules.number_decimals, Some(3), "erbt [normalize]");
        let rt = m.column("Response Time").unwrap();
        assert!(rt.ignore);
        assert_eq!(rt.reason.as_deref(), Some("nicht deterministisch"));
        let idx = m.column("Indexability Status").unwrap();
        assert_eq!(idx.rules.map.get("Canonicalised").unwrap(), "Canonicalised");
        assert_eq!(m.column("Flag").unwrap().rules.bool_mode, BoolMode::Loose);
        assert_eq!(
            m.column("Status Code").unwrap().rules.bool_mode,
            BoolMode::CaseInsensitive
        );
        assert!(m.defaults.empty_equals_null);
    }

    #[test]
    fn normalize_defaults_when_absent() {
        let m = Mapping::parse("key = [\"A\"]\n[columns]\n\"A\" = \"A\"\n").unwrap();
        assert!(m.defaults.trim && m.defaults.base_url);
        assert_eq!(m.defaults.bool_mode, BoolMode::Off);
        assert!(!m.defaults.empty_equals_null);
        assert_eq!(m.os_name_of_key("A"), "A");
        assert_eq!(m.os_name_of_key("B"), "B");
    }

    #[test]
    fn empty_key_and_bad_bool_are_errors() {
        assert!(Mapping::parse("[columns]\n\"A\" = \"A\"\n").is_err());
        assert!(Mapping::parse("key=[\"A\"]\n[normalize]\nbool = \"quatsch\"\n").is_err());
        assert!(Mapping::parse("key = [").is_err());
    }

    #[test]
    fn composite_key_with_ordinal_is_kept() {
        let m = Mapping::parse("key = [\"Source\",\"Destination\",\"Type\",\"#\"]\n").unwrap();
        assert_eq!(m.key.last().unwrap(), ORDINAL_KEY);
    }
}
