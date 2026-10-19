//! Metrikkatalog und Metrik-Baum (INTEG-092): Standardmetriken, Spaltennamen und Kategorien.
//!
//! Der statische Katalog (`metrics.json`, mindestens 65 Metriken) ist die Rückfallquelle und die Namensquelle der
//! Spalten `GA4 <Metrik>`; der Baum der Auswahl entsteht aus dem Metadaten-Endpunkt der Property (`getMetadata`), in dem
//! benutzerdefinierte Metriken unter der Kategorie „Custom“ erscheinen.

use std::sync::OnceLock;

use serde_json::Value;

/// Gemeinsame Katalogdatei (auch von `os-analyze` und dem GA4-Mock gelesen).
pub const CATALOG_JSON: &str = include_str!("metrics.json");

/// Kategorie benutzerdefinierter Metriken.
pub const CUSTOM_CATEGORY: &str = "Custom";

/// Werteart einer Metrik (Darstellung und Export, INTEG-092).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// Ganzzahl.
    Integer,
    /// Dezimalzahl.
    Float,
    /// Quotient (UI: Prozent mit 2 Nachkommastellen, Export: Dezimalzahl).
    Percent,
    /// Betrag in der Property-Währung (2 Nachkommastellen).
    Currency,
    /// Dauer in Sekunden.
    Seconds,
}

impl MetricKind {
    fn from_catalog(kind: &str) -> MetricKind {
        match kind {
            "integer" => MetricKind::Integer,
            "percent" => MetricKind::Percent,
            "currency" => MetricKind::Currency,
            "seconds" => MetricKind::Seconds,
            _ => MetricKind::Float,
        }
    }

    /// Werteart aus dem Typ der Data API (`TYPE_INTEGER`, `TYPE_CURRENCY`, …).
    pub fn from_api_type(kind: &str) -> MetricKind {
        match kind {
            "TYPE_INTEGER" => MetricKind::Integer,
            "TYPE_CURRENCY" => MetricKind::Currency,
            "TYPE_SECONDS" | "TYPE_MILLISECONDS" | "TYPE_MINUTES" | "TYPE_HOURS" => {
                MetricKind::Seconds
            }
            _ => MetricKind::Float,
        }
    }
}

/// Eine Metrik.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricInfo {
    /// API-Name (`sessions`).
    pub api: String,
    /// Englischer Spaltenname ohne Präfix (`Sessions`, `Conversions` für `keyEvents`).
    pub name: String,
    /// Kategorie (`User`, `Session`, `Engagement`, `E-commerce`, `Advertising`, `Custom`).
    pub category: String,
    /// Werteart.
    pub kind: MetricKind,
    /// Benutzerdefinierte Metrik der Property.
    pub custom: bool,
}

/// Eine Dimension (Filterliste INTEG-094).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionInfo {
    /// API-Name.
    pub api: String,
    /// Anzeigename.
    pub name: String,
    /// Kategorie.
    pub category: String,
}

struct Catalog {
    standard: Vec<String>,
    metrics: Vec<MetricInfo>,
    dimensions: Vec<DimensionInfo>,
}

fn load() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let v: Value = serde_json::from_str(CATALOG_JSON).expect("GA4-Katalog ist gültiges JSON");
        let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
        Catalog {
            standard: v["standard"]
                .as_array()
                .into_iter()
                .flatten()
                .map(text)
                .collect(),
            metrics: v["metrics"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|m| MetricInfo {
                    api: text(&m["api"]),
                    name: text(&m["name"]),
                    category: text(&m["category"]),
                    kind: MetricKind::from_catalog(m["kind"].as_str().unwrap_or_default()),
                    custom: false,
                })
                .collect(),
            dimensions: v["dimensions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|d| DimensionInfo {
                    api: text(&d["api"]),
                    name: text(&d["name"]),
                    category: text(&d["category"]),
                })
                .collect(),
        }
    })
}

/// Alle Metriken des statischen Katalogs.
pub fn metrics() -> &'static [MetricInfo] {
    &load().metrics
}

/// Dimensionen des statischen Katalogs.
pub fn dimensions() -> &'static [DimensionInfo] {
    &load().dimensions
}

/// Die sieben Standardmetriken in Spaltenreihenfolge (INTEG-092).
pub fn standard_metrics() -> Vec<&'static str> {
    load().standard.iter().map(String::as_str).collect()
}

/// Metrik des statischen Katalogs.
pub fn metric(api: &str) -> Option<&'static MetricInfo> {
    metrics().iter().find(|m| m.api == api)
}

/// Wörter mit großem Anfangsbuchstaben (GA4-Anzeigenamen sind im Satzstil: `Engaged sessions` → `Engaged Sessions`).
pub fn title_case(text: &str) -> String {
    text.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Spaltenname ohne Dimensionszusatz: `GA4 <Name>`; unbekannte Metriken `GA4 <API-Name ohne Präfix>`.
pub fn base_column_name(api: &str) -> String {
    match metric(api) {
        Some(m) => format!("GA4 {}", m.name),
        None => {
            let short = api.rsplit(':').next().unwrap_or(api);
            format!("GA4 {}", title_case(&short.replace('_', " ")))
        }
    }
}

/// Der Metrik-Baum einer Property: Kategorien mit Metriken, aus `getMetadata`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetricTree {
    /// Kategorien in Katalogreihenfolge, `Custom` zuletzt.
    pub categories: Vec<(String, Vec<MetricInfo>)>,
}

impl MetricTree {
    /// Baum aus dem statischen Katalog (wenn der Metadaten-Endpunkt nicht erreichbar ist).
    pub fn from_catalog() -> MetricTree {
        let mut tree = MetricTree::default();
        for m in metrics() {
            tree.push(m.clone());
        }
        tree
    }

    /// Baum aus der Antwort von `getMetadata`; bekannte Metriken behalten ihren Katalognamen.
    pub fn from_metadata(meta: &Value) -> MetricTree {
        let mut tree = MetricTree::default();
        let mut custom = Vec::new();
        for m in meta["metrics"].as_array().into_iter().flatten() {
            let api = m["apiName"].as_str().unwrap_or_default();
            if api.is_empty() {
                continue;
            }
            let is_custom = m["customDefinition"].as_bool().unwrap_or(false);
            let info = match metric(api) {
                Some(known) if !is_custom => known.clone(),
                _ => MetricInfo {
                    api: api.to_string(),
                    name: title_case(m["uiName"].as_str().unwrap_or(api)),
                    category: if is_custom {
                        CUSTOM_CATEGORY.to_string()
                    } else {
                        m["category"].as_str().unwrap_or("Other").to_string()
                    },
                    kind: MetricKind::from_api_type(m["type"].as_str().unwrap_or_default()),
                    custom: is_custom,
                },
            };
            if info.custom {
                custom.push(info);
            } else {
                tree.push(info);
            }
        }
        for info in custom {
            tree.push(info);
        }
        tree
    }

    fn push(&mut self, info: MetricInfo) {
        match self
            .categories
            .iter_mut()
            .find(|(c, _)| *c == info.category)
        {
            Some((_, list)) => list.push(info),
            None => self.categories.push((info.category.clone(), vec![info])),
        }
    }

    /// Alle Metriken des Baums.
    pub fn all(&self) -> impl Iterator<Item = &MetricInfo> {
        self.categories.iter().flat_map(|(_, list)| list.iter())
    }

    /// Suchfeld: Metriken, deren Name oder API-Name den Text enthält (ohne Groß-/Kleinschreibung).
    pub fn search(&self, query: &str) -> Vec<&MetricInfo> {
        let q = query.to_lowercase();
        self.all()
            .filter(|m| m.name.to_lowercase().contains(&q) || m.api.to_lowercase().contains(&q))
            .collect()
    }

    /// Metriken einer Kategorie.
    pub fn category(&self, name: &str) -> &[MetricInfo] {
        self.categories
            .iter()
            .find(|(c, _)| c == name)
            .map_or(&[], |(_, list)| list.as_slice())
    }
}
