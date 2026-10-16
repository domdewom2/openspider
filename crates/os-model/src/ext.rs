//! Erweiterungsschlüssel `ext` (`url_ext`, Schema v3): skalare Zusatzfakten je URL (M2-F01, `docs/plan/M2.md` 3.3).
//!
//! Ein Schlüssel hat die Form `<namensraum>.<name>` (beides `[a-z][a-z0-9_]*`), z. B. `security.csp_present`,
//! `head.title_outside`, `readability.flesch`. Jedes Paket der Wellen 1 – 3 besitzt genau die Namensräume, die
//! [`NAMESPACES`] ihm zuweist, und schreibt nur Schlüssel in diesen Namensräumen. Ein neuer Namensraum entsteht
//! ausschließlich über einen Schnittstellenbedarf (Änderung dieser Tabelle durch ein N-Paket).
//!
//! Werte sind [`Value`] (Null, Bool, Ganzzahl, Gleitkommazahl, Text); der Store legt sie in `v_int`, `v_real` und
//! `v_text` ab.

use std::borrow::Cow;

use crate::error::ModelError;
use crate::ids::UrlId;
use crate::value::Value;

/// Schlüssel eines Erweiterungswertes: `"<namensraum>.<name>"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtKey(pub Cow<'static, str>);

impl ExtKey {
    /// Schlüssel aus einem statischen Text (ohne Allokation).
    pub const fn from_static(key: &'static str) -> ExtKey {
        ExtKey(Cow::Borrowed(key))
    }

    /// Schlüssel aus Namensraum und Name; prüft die Form.
    pub fn new(namespace: &str, name: &str) -> Result<ExtKey, ModelError> {
        let key = format!("{namespace}.{name}");
        ExtKey::parse(&key)
    }

    /// Prüft die Form `<namensraum>.<name>` und gibt den Schlüssel zurück.
    pub fn parse(key: &str) -> Result<ExtKey, ModelError> {
        match key.split_once('.') {
            Some((ns, name)) if ident_ok(ns) && ident_ok(name) => {
                Ok(ExtKey(Cow::Owned(key.to_string())))
            }
            _ => Err(ModelError::UnknownValue {
                kind: "ExtKey",
                value: key.to_string(),
            }),
        }
    }

    /// Der Schlüssel als Text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Namensraum (Teil vor dem Punkt).
    pub fn namespace(&self) -> &str {
        self.0.split_once('.').map_or(&self.0, |(ns, _)| ns)
    }

    /// Paket, dem der Namensraum gehört (`None` = unbekannter Namensraum).
    pub fn owner(&self) -> Option<&'static str> {
        owner_of(self.namespace())
    }
}

impl std::fmt::Display for ExtKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn ident_ok(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Ein skalarer Zusatzfakt einer URL (Eintrag in `url_ext`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtWrite {
    /// URL.
    pub url: UrlId,
    /// Schlüssel.
    pub key: ExtKey,
    /// Wert.
    pub value: Value,
}

/// Namensräume der Erweiterungsschlüssel und das Paket, das sie schreibt. Vollständig für alle Pakete der Wellen 1 – 3
/// aus `docs/plan/M2-packages.json`, die skalare Zusatzfakten je URL erzeugen; Test `ext_namespaces`.
pub const NAMESPACES: &[(&str, &str)] = &[
    // Engine, HTTP, URL, Rendering
    ("crawl", "M2-E01"),
    ("lifecycle", "M2-E02"),
    ("http", "M2-E03"),
    ("urlfilter", "M2-E04"),
    ("render", "M2-E05"),
    ("engine", "M2-E06"),
    ("js", "M2-R02"),
    // Fachpakete: Tabs und Filter
    ("head", "M2-D01"),
    ("serp", "M2-D01"),
    ("content", "M2-D02"),
    ("readability", "M2-D02"),
    ("dup", "M2-D02"),
    ("image", "M2-D03"),
    ("canonical", "M2-D04"),
    ("pagination", "M2-D05"),
    ("directive", "M2-D06"),
    ("hreflang", "M2-D07"),
    ("link", "M2-D08"),
    ("linkscore", "M2-D08"),
    ("sd", "M2-D09"),
    ("validation", "M2-D10"),
    ("security", "M2-D11"),
    ("response", "M2-D12"),
    ("sitemap", "M2-D13"),
    ("custom_search", "M2-D14"),
    ("custom_extract", "M2-D15"),
    ("linkpos", "M2-D16"),
    ("webdata", "M2-D18"),
    ("spelling", "M2-D19"),
    ("sdval", "M2-D20"),
    ("a11y", "M2-D21"),
    ("custom_js", "M2-D22"),
    ("mobile", "M2-D23"),
    // Integrationen
    ("api", "M2-I01"),
    ("ga4", "M2-I02"),
    ("gsc", "M2-I03"),
    ("psi", "M2-I04"),
    ("linkmetrics", "M2-I05"),
    ("apimode", "M2-I06"),
    // Kern der App-Schicht
    ("analysis", "M2-K03"),
    ("segment", "M2-K14"),
    ("compare", "M2-C01"),
    ("change", "M2-C02"),
];

/// Paket, dem der Namensraum `namespace` gehört.
pub fn owner_of(namespace: &str) -> Option<&'static str> {
    NAMESPACES
        .iter()
        .find(|(ns, _)| *ns == namespace)
        .map(|(_, pkg)| *pkg)
}

/// Gehört der Schlüssel in einen Namensraum des Pakets `package`?
pub fn key_allowed_for(package: &str, key: &ExtKey) -> bool {
    key.owner() == Some(package)
}
