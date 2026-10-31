//! Dokumentierte Paritätsabweichungen (`tests/parity/deviations.toml`, Abschnitt 5.3 Punkt 4).
//!
//! Jede Abweichung nennt eine Spec-ID und ein Kennzeichen (`Paritätsabweichung`, `Annahme`,
//! `OS-Festlegung`) und schränkt ein, was sie entschuldigt:
//!
//! ```toml
//! [[deviation]]
//! id = "DEV-001"                 # frei wählbar
//! spec = "ONPAGE-004"            # Pflicht: Spec-ID der Begründung
//! kind = "OS-Festlegung"         # Pflicht: Paritätsabweichung | Annahme | OS-Festlegung
//! note = "Begründung"
//! site = "parity-onpage"         # optional: nur diese Site
//! export = "page_titles_all"     # optional: nur dieser Export (Mapping-Name; mehrere durch `|` getrennt, `*` = alle)
//! column = "Title 1"             # optional: nur diese Referenzspalte (Scope cell)
//! key = "{BASE}/a.html"          # optional: genau diese Zeile (normalisierter Schlüssel)
//! key_prefix = "{BASE}/robots"   # optional: alle Zeilen mit diesem Präfix
//! scope = "cell"                 # cell (Standard) | missing_in_os | extra_in_os
//! ref = "100"                    # optional: nur wenn der Referenzwert so lautet
//! os = "99"                      # optional: nur wenn der OpenSpider-Wert so lautet
//! delta = 1                       # optional: nur wenn OpenSpider-Wert − Referenzwert genau so groß ist
//! key_prefixes = ["Internal | All", "Security | "]   # optional: Zeilen, deren Schlüssel mit einem davon beginnt
//! ```
//!
//! `column` darf mehrere Spalten nennen, getrennt durch `|` (`column = "Title 1 Length|Title 2 Length"`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ParityError;

/// Was eine Abweichung entschuldigt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Zellabweichungen (aus dem Vergleich genommen).
    #[default]
    Cell,
    /// Zeile der Referenz fehlt in OpenSpider.
    MissingInOs,
    /// Zeile in OpenSpider, die die Referenz nicht hat.
    ExtraInOs,
}

/// Eine dokumentierte Abweichung.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Deviation {
    /// Kennung.
    #[serde(default)]
    pub id: String,
    /// Spec-ID der Begründung.
    #[serde(default)]
    pub spec: String,
    /// Kennzeichen.
    #[serde(default)]
    pub kind: String,
    /// Begründung (in `tests/parity/deviations.toml` als `reason` dokumentiert, `note` bleibt erlaubt).
    #[serde(default, alias = "reason")]
    pub note: String,
    /// Einschränkung auf eine Site.
    #[serde(default)]
    pub site: Option<String>,
    /// Einschränkung auf einen Export (Mapping-Name).
    #[serde(default)]
    pub export: Option<String>,
    /// Optionaler Referenzdateiname; wird vom Runner vor dem Vergleich gefiltert.
    #[serde(default)]
    pub file: Option<String>,
    /// Einschränkung auf eine Referenzspalte.
    #[serde(default)]
    pub column: Option<String>,
    /// Einschränkung auf eine Zeile (in `tests/parity/deviations.toml` als `address` dokumentiert).
    #[serde(default, alias = "address")]
    pub key: Option<String>,
    /// Einschränkung auf Zeilen mit diesem Schlüsselpräfix.
    #[serde(default)]
    pub key_prefix: Option<String>,
    /// Einschränkung auf Zeilen, deren Schlüssel mit einem dieser Präfixe beginnt.
    #[serde(default)]
    pub key_prefixes: Vec<String>,
    /// Nur bei genau dieser Differenz „OpenSpider-Wert − Referenzwert“ (beide Werte numerisch).
    #[serde(default)]
    pub delta: Option<f64>,
    /// Geltungsbereich.
    #[serde(default)]
    pub scope: Scope,
    /// Nur bei diesem Referenzwert.
    #[serde(default, rename = "ref")]
    pub reference: Option<String>,
    /// Nur bei diesem OpenSpider-Wert.
    #[serde(default)]
    pub os: Option<String>,
}

/// Erlaubte Kennzeichen.
pub const KINDS: [&str; 3] = ["Paritätsabweichung", "Annahme", "OS-Festlegung"];

/// Ist das Kennzeichen eines der erlaubten? Die ASCII-Schreibweise `Paritaetsabweichung` (so steht sie im
/// Schema von `tests/parity/deviations.toml`) gilt als dasselbe Kennzeichen.
pub fn is_known_kind(kind: &str) -> bool {
    let k = kind.trim().replace("ae", "ä");
    KINDS
        .iter()
        .any(|known| *known == kind.trim() || *known == k)
}

impl Deviation {
    /// Gilt die Abweichung für diese Site und diesen Export?
    pub fn applies_to(&self, site: Option<&str>, export: &str) -> bool {
        self.site.as_deref().is_none_or(|s| {
            s == "*" || site.is_some_and(|x| s.split('|').any(|part| part.trim() == x))
        }) && self
            .export
            .as_deref()
            .is_none_or(|e| e == "*" || e.split('|').any(|part| part.trim() == export))
    }

    fn matches_key(&self, key: &str) -> bool {
        if let Some(k) = &self.key {
            if k != key {
                return false;
            }
        }
        if let Some(p) = &self.key_prefix {
            if !key.starts_with(p.as_str()) {
                return false;
            }
        }
        if !self.key_prefixes.is_empty() && !self.key_prefixes.iter().any(|p| key.starts_with(p)) {
            return false;
        }
        true
    }

    /// Entschuldigt die Abweichung eine abweichende Zelle?
    pub fn excuses_cell(&self, key: &str, column: &str, reference: &str, os: &str) -> bool {
        self.scope == Scope::Cell
            && self.matches_key(key)
            && self
                .column
                .as_deref()
                .is_none_or(|c| c == "*" || c.split('|').any(|x| x.trim() == column))
            && self.reference.as_deref().is_none_or(|v| v == reference)
            && self.os.as_deref().is_none_or(|v| v == os)
            && self.delta.is_none_or(|d| {
                match (
                    crate::normalize::parse_number(reference),
                    crate::normalize::parse_number(os),
                ) {
                    (Some(a), Some(b)) => ((b - a) - d).abs() < 1e-9,
                    _ => false,
                }
            })
    }

    /// Entschuldigt die Abweichung eine fehlende bzw. zusätzliche Zeile?
    pub fn excuses_row(&self, scope: Scope, key: &str) -> bool {
        self.scope == scope && scope != Scope::Cell && self.matches_key(key)
    }
}

/// Geladene Abweichungsliste mit Hinweisen zu unvollständigen Einträgen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deviations {
    /// Einträge in Dateireihenfolge.
    pub items: Vec<Deviation>,
    /// Warnungen (fehlende Spec-ID, unbekanntes Kennzeichen).
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default, alias = "deviations")]
    deviation: Vec<Deviation>,
}

impl Deviations {
    /// Lädt die Datei; eine fehlende Datei ergibt eine leere Liste.
    pub fn load(path: &Path) -> Result<Self, ParityError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)
                .map_err(|e| ParityError::Invalid(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Parst den TOML-Text.
    pub fn parse(text: &str) -> Result<Self, ParityError> {
        let raw: RawFile = toml::from_str(text).map_err(|e| ParityError::Invalid(e.to_string()))?;
        let mut warnings = Vec::new();
        for (i, d) in raw.deviation.iter().enumerate() {
            let label = if d.id.is_empty() {
                format!("Abweichung Nr. {}", i + 1)
            } else {
                d.id.clone()
            };
            if d.spec.trim().is_empty() {
                warnings.push(format!("{label}: Spec-ID (`spec`) fehlt"));
            }
            if !is_known_kind(&d.kind) {
                warnings.push(format!(
                    "{label}: Kennzeichen {:?} ist keines von {}",
                    d.kind,
                    KINDS.join(", ")
                ));
            }
        }
        Ok(Self {
            items: raw.deviation,
            warnings,
        })
    }

    /// Einträge, die für Site und Export gelten.
    pub fn for_export(&self, site: Option<&str>, export: &str) -> Vec<Deviation> {
        self.for_file(site, export, None)
    }

    /// Einträge für eine konkrete Referenzdatei. Ohne Datei gelten nur dateiunabhängige Ausnahmen.
    pub fn for_file(&self, site: Option<&str>, export: &str, file: Option<&str>) -> Vec<Deviation> {
        self.items
            .iter()
            .filter(|d| {
                d.applies_to(site, export)
                    && d.file.as_ref().is_none_or(|f| {
                        file.is_some_and(|file| f.split('|').any(|name| name == file))
                    })
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[deviation]]
id = "DEV-1"
spec = "ONPAGE-004"
kind = "OS-Festlegung"
note = "Pixelbreite weicht ab"
export = "page_titles_all"
column = "Title 1 Pixel Width"

[[deviation]]
id = "DEV-2"
spec = "CRAWL-050"
kind = "Annahme"
scope = "missing_in_os"
key_prefix = "{BASE}/robots"
site = "parity-response"

[[deviation]]
id = "DEV-3"
kind = "komisch"
"#;

    #[test]
    fn parses_and_warns_about_incomplete_entries() {
        let d = Deviations::parse(SAMPLE).unwrap();
        assert_eq!(d.items.len(), 3);
        assert_eq!(d.items[1].scope, Scope::MissingInOs);
        assert_eq!(d.warnings.len(), 2);
        assert!(d
            .warnings
            .iter()
            .any(|w| w.contains("DEV-3") && w.contains("Spec-ID")));
        assert!(d.warnings.iter().any(|w| w.contains("Kennzeichen")));
    }

    #[test]
    fn filters_by_site_and_export() {
        let d = Deviations::parse(SAMPLE).unwrap();
        assert_eq!(
            d.for_export(Some("parity-onpage"), "page_titles_all").len(),
            2
        );
        assert_eq!(d.for_export(Some("parity-onpage"), "internal_all").len(), 1);
        let r = d.for_export(Some("parity-response"), "internal_all");
        assert_eq!(r.len(), 2);
        assert!(d.for_export(None, "x").iter().all(|d| d.site.is_none()));
    }

    #[test]
    fn cell_and_row_matching() {
        let d = Deviations::parse(SAMPLE).unwrap();
        assert!(d.items[0].excuses_cell("{BASE}/a", "Title 1 Pixel Width", "1", "2"));
        assert!(!d.items[0].excuses_cell("{BASE}/a", "Title 1", "1", "2"));
        assert!(!d.items[0].excuses_row(Scope::MissingInOs, "{BASE}/a"));
        assert!(d.items[1].excuses_row(Scope::MissingInOs, "{BASE}/robots.txt"));
        assert!(!d.items[1].excuses_row(Scope::MissingInOs, "{BASE}/index"));
        assert!(!d.items[1].excuses_row(Scope::ExtraInOs, "{BASE}/robots.txt"));
        assert!(!d.items[1].excuses_cell("{BASE}/robots.txt", "x", "", ""));
    }

    #[test]
    fn value_restriction_and_missing_file() {
        let d = Deviation {
            reference: Some("100".into()),
            os: Some("99".into()),
            ..Default::default()
        };
        assert!(d.excuses_cell("k", "c", "100", "99"));
        assert!(!d.excuses_cell("k", "c", "100", "98"));
        let missing = Deviations::load(Path::new("/nicht/vorhanden/deviations.toml")).unwrap();
        assert!(missing.items.is_empty());
    }

    #[test]
    fn documented_schema_names_and_ascii_kind_are_accepted() {
        let d = Deviations::parse(
            "[[deviation]]\nid=\"DEV-001\"\nspec=\"ONPAGE-081\"\nkind=\"Paritaetsabweichung\"\n\
             export=\"*\"\nsite=\"*\"\naddress=\"{BASE}/x.html\"\nreason=\"Begründung\"\n",
        )
        .unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        assert_eq!(d.items[0].note, "Begründung");
        assert_eq!(d.items[0].key.as_deref(), Some("{BASE}/x.html"));
        assert!(d.items[0].excuses_cell("{BASE}/x.html", "c", "", ""));
        assert!(!d.items[0].excuses_cell("{BASE}/y.html", "c", "", ""));
        // `site = "*"` gilt auch, wenn der Lauf keine Site nennt (`diff` ohne `run`).
        assert_eq!(d.for_export(None, "internal_all").len(), 1);
        assert!(is_known_kind("Paritätsabweichung") && !is_known_kind("komisch"));
    }

    #[test]
    fn aliased_table_name_is_accepted() {
        let d = Deviations::parse("[[deviations]]\nspec=\"A-1\"\nkind=\"Annahme\"\n").unwrap();
        assert_eq!(d.items.len(), 1);
        assert!(d.warnings.is_empty());
    }
}
