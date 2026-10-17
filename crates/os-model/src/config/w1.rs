//! Additive Konfiguration für die Welle-1-Schnittstellen (M2-F05a).
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// SERP-Metriken je Gerät (ONPAGE-194).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SerpDeviceConfig {
    pub title_max_px: Option<u32>,
    pub description_max_px: Option<u32>,
    pub title_font_px: f64,
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
    pub desktop: SerpDeviceConfig,
    pub mobile: SerpDeviceConfig,
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
    pub parity_filters: bool,
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
    pub real_dimensions_from_header: bool,
    pub parse_inline_css_backgrounds: bool,
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
    pub retries: u32,
    pub concurrency: u32,
    pub requests_per_window: Option<u32>,
    pub window_s: u32,
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
    pub ignore_trailing_slash: bool,
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
