//! Additive Konfiguration für die Welle-1-Schnittstellen (M2-F05a).
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// SERP-Metriken je Gerät (ONPAGE-194).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SerpDeviceConfig {
    /// Maximale Titelbreite; None koppelt an title.max_px (ONPAGE-194).
    pub title_max_px: Option<u32>,
    /// Maximale Beschreibungsbreite; None koppelt an meta_description.max_px (ONPAGE-194).
    pub description_max_px: Option<u32>,
    /// Schriftgröße des Titels in Pixeln, 6 bis 64 (ONPAGE-194).
    pub title_font_px: f64,
    /// Schriftgröße der Beschreibung in Pixeln, 6 bis 64 (ONPAGE-194).
    pub description_font_px: f64,
}
impl Default for SerpDeviceConfig {
    fn default() -> Self {
        Self {
            title_max_px: None,
            description_max_px: None,
            title_font_px: 20.0,
            description_font_px: 14.0,
        }
    }
}

/// Gerätespezifische Vorschaugrenzen (ONPAGE-194).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SerpConfig {
    /// Desktop-Vorschau, standardmäßig an allgemeine Textgrenzen gekoppelt (ONPAGE-194).
    pub desktop: SerpDeviceConfig,
    /// Mobil-Vorschau mit eigenen Pixelgrenzen (ONPAGE-194).
    pub mobile: SerpDeviceConfig,
    /// Tablet-Vorschau mit eigenen Pixelgrenzen (ONPAGE-194).
    pub tablet: SerpDeviceConfig,
}
impl Default for SerpConfig {
    fn default() -> Self {
        let mobile = SerpDeviceConfig {
            title_max_px: Some(561),
            description_max_px: Some(680),
            ..Default::default()
        };
        Self {
            desktop: Default::default(),
            tablet: mobile.clone(),
            mobile,
        }
    }
}

/// Profilgebundene Tabelleneinstellungen (ONPAGE-023, OUT-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CrawlUiConfig {
    /// Filter nach den Paritätsregeln darstellen (ONPAGE-023).
    pub parity_filters: bool,
    /// Sichtbare Spalten je Tab in Anzeigereihenfolge; fehlend bedeutet Standard (OUT-009, OUT-220).
    pub columns: BTreeMap<String, Vec<String>>,
}
impl Default for CrawlUiConfig {
    fn default() -> Self {
        Self {
            parity_filters: true,
            columns: BTreeMap::new(),
        }
    }
}

/// Zusätzliche Bildfakten (ONPAGE-172/178/180).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ImagesConfig {
    /// Tatsächliche Bildmaße aus dem Dateikopf lesen (ONPAGE-172).
    pub real_dimensions_from_header: bool,
    /// Inline-CSS-Hintergrundbilder extrahieren (ONPAGE-178).
    pub parse_inline_css_backgrounds: bool,
    /// Mindesteinsparung in KiB für die Größenwarnung (ONPAGE-180).
    pub incorrect_size_min_saving_kb: u32,
}
impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            real_dimensions_from_header: false,
            parse_inline_css_backgrounds: false,
            incorrect_size_min_saving_kb: 4,
        }
    }
}

/// Veraltete MIME-Typen (ONPAGE-043).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SecurityConfig {
    /// MIME-Typen, die als veraltet gelten sollen (ONPAGE-043).
    pub obsolete_mime_types: Vec<String>,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            obsolete_mime_types: vec!["text/javascript".into(), "application/x-javascript".into()],
        }
    }
}

/// Grenzen je API-Quelle (INTEG-005/006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ApiLimitsConfig {
    /// Maximale Anzahl API-Wiederholungen, 0 bis 20 (INTEG-005).
    pub retries: u32,
    /// Maximale parallele API-Anfragen, 1 bis 50 (INTEG-006).
    pub concurrency: u32,
    /// Anfragelimit je Zeitfenster; None deaktiviert das Limit (INTEG-006).
    pub requests_per_window: Option<u32>,
    /// Dauer des API-Zeitfensters in Sekunden, mindestens 1 (INTEG-006).
    pub window_s: u32,
    /// Optionales Kontingent der API-Quelle (INTEG-006).
    pub quota_limit: Option<u64>,
}
impl Default for ApiLimitsConfig {
    fn default() -> Self {
        Self {
            retries: 5,
            concurrency: 5,
            requests_per_window: None,
            window_s: 60,
            quota_limit: None,
        }
    }
}

/// URL-Abgleich von API-Quellen (INTEG-014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct UrlMatchConfig {
    /// Abschließenden Slash beim URL-Abgleich ignorieren (INTEG-014).
    pub ignore_trailing_slash: bool,
    /// Groß- und Kleinschreibung beim URL-Abgleich ignorieren (INTEG-014).
    pub ignore_case: bool,
}
impl Default for UrlMatchConfig {
    fn default() -> Self {
        Self {
            ignore_trailing_slash: true,
            ignore_case: true,
        }
    }
}
