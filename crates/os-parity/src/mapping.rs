//! Spaltenmapping Referenz → OS (`tests/parity/mappings/<export-slug>.toml`, Abschnitt 5.2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    optional_ref: bool,
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
    /// Zusatzregeln für den Zellvergleich: `ignore_case`, `zero_as_empty`, `xpath_tags`, `hex_trim_zeros`,
    /// `decimal_comma`.
    #[serde(default)]
    transform: Vec<String>,
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
    #[serde(default)]
    transform: Vec<String>,
}

impl Default for RawNormalize {
    fn default() -> Self {
        Self {
            base_url: true,
            trim: true,
            bool: None,
            number_decimals: None,
            empty_equals_null: false,
            transform: Vec::new(),
        }
    }
}

/// Block-Layout der Referenz (Crawl Overview): mehrere Abschnitte mit eigener Kopfzeile statt einer Tabelle.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlocks {
    /// Weitere Filterkataloge (M2 ergänzt M1).
    #[serde(default)]
    filters: Vec<String>,
    /// Abschnittsnamen der Referenz → Abschnittsnamen von OpenSpider.
    #[serde(default)]
    section_map: BTreeMap<String, String>,
    /// Pfad (relativ zur Repository-Wurzel) der Filterliste M1; nur Zeilen zu M1-Filtern müssen in
    /// OpenSpider vorkommen, alle übrigen Zeilen der Referenz werden nur gezählt.
    #[serde(default)]
    m1_filters: Option<String>,
    /// Zeilen, die nur OpenSpider hat (Zusatzabschnitte, Programmangaben), sind kein Fehler.
    #[serde(default)]
    os_only_rows_ok: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct RawMapping {
    #[serde(default)]
    chain_end_target: bool,
    /// Reihenfolge der normalisierten Schlüssel muss übereinstimmen (Listenmodus).
    #[serde(default)]
    ordered: bool,
    #[serde(default)]
    key: Vec<String>,
    #[serde(default)]
    drop_ref_columns: Vec<String>,
    #[serde(default)]
    columns: BTreeMap<String, RawColumn>,
    #[serde(default)]
    normalize: RawNormalize,
    /// `"table"` (Standard) oder `"blocks"` (Crawl-Overview-Blockformat der Referenz).
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    blocks: Option<RawBlocks>,
    /// Mehrere Referenzzeilen mit gleichem Schlüssel (Redirect-Reports: eine Zeile je verlinkende Quelle)
    /// zählen als eine Zeile; verglichen wird die erste.
    #[serde(default)]
    dedupe_reference: bool,
}

/// Layout der Referenzdatei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefFormat {
    /// Eine Kopfzeile, dann Datenzeilen.
    #[default]
    Table,
    /// Abschnitte mit eigener Kopfzeile (Crawl Overview).
    Blocks,
    /// XML- oder Bilder-Sitemap.
    Xml,
    /// Referenz-Verbindungen und OpenSpider-Knoten (OUT-023).
    CrawlPath,
}

/// Einstellungen für [`RefFormat::Blocks`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlocksOpts {
    /// Abschnittsnamen der Referenz → OpenSpider.
    pub section_map: BTreeMap<String, String>,
    /// Gelesene M1-Filter als `(tab, filter)` in Kleinschreibung; `None` = jede Zeile der Referenz zählt.
    pub m1_filters: Option<BTreeSet<(String, String)>>,
    /// Zeilen nur in OpenSpider sind kein Fehler.
    pub os_only_rows_ok: bool,
}

/// Eine Spalte des Mappings.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnMap {
    /// Spaltenname in der Referenz.
    pub reference: String,
    /// Die Spalte fehlt in manchen Referenzgenerationen; vorhandene Werte bleiben Pflicht.
    pub optional_ref: bool,
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
    /// OUT-081: Referenz hängt das Endziel als Sprung n+1 an.
    pub chain_end_target: bool,
    /// Reihenfolge zusätzlich zum Zellvergleich prüfen.
    pub ordered: bool,
    /// Schlüsselspalten (Referenznamen); `"#"` = laufende Nummer.
    pub key: Vec<String>,
    /// Zu verwerfende Referenzspalten.
    pub drop_ref_columns: Vec<String>,
    /// Spalten in alphabetischer Reihenfolge der Referenznamen.
    pub columns: Vec<ColumnMap>,
    /// Regeln für Schlüsselspalten und nicht explizit aufgeführte Spalten.
    pub defaults: CellRules,
    /// Layout der Referenzdatei.
    pub format: RefFormat,
    /// Einstellungen des Block-Layouts (nur bei [`RefFormat::Blocks`]).
    pub blocks: BlocksOpts,
    /// Referenzzeilen mit gleichem Schlüssel zu einer Zeile zusammenfassen (die erste gilt).
    pub dedupe_reference: bool,
    /// Pfad der Filterliste, solange [`Mapping::load`] sie noch nicht gelesen hat.
    m1_filters_path: Option<String>,
    filters_paths: Vec<String>,
}

/// Wendet Zusatzregeln (`transform`) auf Vergleichsregeln an.
fn apply_transforms(rules: &mut CellRules, names: &[String]) -> Result<(), ParityError> {
    for n in names {
        match n.trim() {
            "ignore_case" => rules.ignore_case = true,
            "zero_as_empty" => rules.zero_as_empty = true,
            "xpath_tags" => rules.xpath_tags = true,
            "hex_trim_zeros" => rules.hex_trim_zeros = true,
            "decimal_comma" => rules.decimal_comma = true,
            other => {
                return Err(ParityError::Invalid(format!(
                    "unbekannte transform-Regel {other:?} (erlaubt: ignore_case, zero_as_empty, xpath_tags, hex_trim_zeros, decimal_comma)"
                )))
            }
        }
    }
    Ok(())
}

/// Liest die M1-Filterliste (`docs/plan/M1-filters.toml`): `(tab, filter)` in Kleinschreibung.
fn load_m1_filters(path: &Path) -> Result<BTreeSet<(String, String)>, ParityError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ParityError::Invalid(format!("Filterliste {} nicht lesbar: {e}", path.display()))
    })?;
    let v: toml::Value = toml::from_str(&text)
        .map_err(|e| ParityError::Invalid(format!("Filterliste {}: {e}", path.display())))?;
    let mut out = BTreeSet::new();
    for tab in v
        .get("tab")
        .and_then(|t| t.as_array())
        .into_iter()
        .flatten()
    {
        let Some(id) = tab.get("id").and_then(|i| i.as_str()) else {
            continue;
        };
        for f in tab
            .get("filters")
            .and_then(|f| f.as_array())
            .into_iter()
            .flatten()
        {
            if f.get("extension").and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }
            if let Some(fid) = f
                .get("default_label")
                .or_else(|| f.get("id"))
                .and_then(|i| i.as_str())
            {
                let mut label = fid.to_string();
                let defaults = serde_json::to_value(os_model::CrawlConfig::default())
                    .map_err(|e| ParityError::Invalid(e.to_string()))?;
                while let Some(start) = label.find('{') {
                    let Some(end) = label[start..].find('}').map(|n| start + n) else {
                        break;
                    };
                    let key = &label[start + 1..end];
                    let value = key.split('.').try_fold(&defaults, |v, k| v.get(k));
                    let Some(value) = value else {
                        break;
                    };
                    let value = if key.ends_with("_bytes") {
                        value
                            .as_u64()
                            .map(|n| (n / (1024 * 1024)).to_string())
                            .unwrap_or_else(|| value.to_string())
                    } else {
                        value
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| value.to_string())
                    };
                    label.replace_range(start..=end, &value);
                }
                out.insert((id.to_lowercase(), label.to_lowercase()));
            }
        }
    }
    Ok(out)
}

/// Sucht `rel` in `start` und seinen Elternordnern.
fn find_upwards(start: &Path, rel: &str) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|d| d.join(rel))
        .find(|candidate| candidate.is_file())
}

impl Mapping {
    /// Lädt ein Mapping aus einer TOML-Datei.
    pub fn load(path: &Path) -> Result<Self, ParityError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            ParityError::Invalid(format!("Mapping {} nicht lesbar: {e}", path.display()))
        })?;
        let mut mapping = Self::parse(&text)
            .map_err(|e| ParityError::Invalid(format!("Mapping {}: {e}", path.display())))?;
        if let Some(rel) = mapping.m1_filters_path.take() {
            let dir = path.parent().unwrap_or_else(|| Path::new("."));
            let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
            let file = find_upwards(&dir, &rel).ok_or_else(|| {
                ParityError::Invalid(format!(
                    "Mapping {}: Filterliste {rel} nicht gefunden",
                    path.display()
                ))
            })?;
            mapping.blocks.m1_filters = Some(load_m1_filters(&file)?);
        }
        for rel in std::mem::take(&mut mapping.filters_paths) {
            let dir = std::fs::canonicalize(path.parent().unwrap_or_else(|| Path::new(".")))?;
            let file = find_upwards(&dir, &rel)
                .ok_or_else(|| ParityError::Invalid(format!("Filterliste {rel} nicht gefunden")))?;
            mapping
                .blocks
                .m1_filters
                .get_or_insert_with(BTreeSet::new)
                .extend(load_m1_filters(&file)?);
        }
        Ok(mapping)
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
            ..CellRules::default()
        };
        let mut defaults = defaults;
        apply_transforms(&mut defaults, &n.transform)?;
        let mut columns = Vec::new();
        for (reference, col) in raw.columns {
            let cm = match col {
                RawColumn::Name(os) => ColumnMap {
                    optional_ref: false,
                    reference,
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
                    apply_transforms(&mut rules, &o.transform)?;
                    ColumnMap {
                        optional_ref: o.optional_ref,
                        os: o.os.unwrap_or_else(|| reference.clone()),
                        reference,
                        ignore: o.ignore,
                        reason: o.reason,
                        rules,
                    }
                }
            };
            columns.push(cm);
        }
        let format = match raw.format.as_deref().map(str::trim) {
            None | Some("") | Some("table") => RefFormat::Table,
            Some("blocks") => RefFormat::Blocks,
            Some("xml") => RefFormat::Xml,
            Some("crawl_path") => RefFormat::CrawlPath,
            Some(other) => {
                return Err(ParityError::Invalid(format!(
                    "unbekanntes format {other:?} (erlaubt: table, blocks, xml, crawl_path)"
                )))
            }
        };
        let (blocks, m1_filters_path, filters_paths) = match raw.blocks {
            Some(b) => (
                BlocksOpts {
                    section_map: b.section_map,
                    m1_filters: None,
                    os_only_rows_ok: b.os_only_rows_ok,
                },
                b.m1_filters,
                b.filters,
            ),
            None => (BlocksOpts::default(), None, Vec::new()),
        };
        Ok(Self {
            chain_end_target: raw.chain_end_target,
            ordered: raw.ordered,
            filters_paths,
            key: raw.key,
            drop_ref_columns: raw.drop_ref_columns,
            columns,
            defaults,
            format,
            blocks,
            dedupe_reference: raw.dedupe_reference,
            m1_filters_path,
        })
    }

    /// Spalte für einen Referenznamen.
    pub fn column(&self, reference: &str) -> Option<&ColumnMap> {
        self.columns.iter().find(|c| c.reference == reference)
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
drop_ref_columns = ["Description", "How To Fix"]
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
        assert_eq!(m.drop_ref_columns, ["Description", "How To Fix"]);
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
