//! GA4-Einstellungen aus `api.ga4` (INTEG-090 … 096): Property und Datenstream, Zeitraum, Metriken mit Seitendimension,
//! Dimensionsfilter, Ergebnislimit und Seitengröße.
//!
//! **Ablage in der eingefrorenen `Ga4Config`** (bis ein Schnittstellenbedarf eigene Felder liefert):
//! - `property`: `properties/<id>` (alle Datenstreams) oder `properties/<id>/dataStreams/<stream>` (ein Stream);
//!   eine bloße Zahl gilt als Property-Kennung.
//! - `metrics`: API-Namen in Spaltenreihenfolge, optional mit Seitendimension `@page` bzw. `@landing`
//!   (`sessions@landing`); leer = die sieben Standardmetriken.
//! - `page_dimension`: globale Seitendimension, `pageLocation` (Standard) oder `landingPagePlusQueryString`.
//! - `filter`: JSON – entweder eine Liste von [`DimensionFilter`] oder ein Objekt
//!   `{"filters": [...], "max_results": 100000, "page_size": 10000}`.

use os_model::config::{DateRangeConfig, Ga4Config, UrlMatchConfig};
use os_model::DateRangePreset;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::Date;

use super::catalog;

/// Standard des Ergebnislimits (INTEG-095).
pub const DEFAULT_MAX_RESULTS: u64 = 100_000;
/// Größtes Ergebnislimit (INTEG-095).
pub const MAX_MAX_RESULTS: u64 = 10_000_000;
/// Standard-Seitengröße (INTEG-096).
pub const DEFAULT_PAGE_SIZE: u64 = 10_000;
/// Größte Seitengröße der Data API (INTEG-096).
pub const MAX_PAGE_SIZE: u64 = 250_000;
/// Höchstens so viele Metriken je `runReport` (INTEG-092).
pub const MAX_METRICS_PER_REQUEST: usize = 10;

/// Fehler einer Einstellung, vor dem Senden erkannt.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    /// Keine Property gewählt.
    #[error("Keine GA4-Property gewählt")]
    NoProperty,
    /// Property-Angabe unlesbar.
    #[error("Ungültige GA4-Property: {0}")]
    BadProperty(String),
    /// Ungültiges Datum.
    #[error("Ungültiges Datum: {0}")]
    BadDate(String),
    /// Enddatum vor dem Startdatum (INTEG-091).
    #[error("Das Enddatum liegt vor dem Startdatum")]
    EndBeforeStart,
    /// Ungültiger regulärer Ausdruck (INTEG-094).
    #[error("Ungültiger regulärer Ausdruck in Filter {index}: {message}")]
    BadRegex {
        /// Position des Filters.
        index: usize,
        /// Grund.
        message: String,
    },
    /// Filterangabe unlesbar.
    #[error("Ungültiger Dimensionsfilter: {0}")]
    BadFilter(String),
    /// Unbekannte Seitendimension.
    #[error("Unbekannte Seitendimension: {0}")]
    BadDimension(String),
}

/// Seitendimension (INTEG-093).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum PageDimension {
    /// Vollständige Seiten-URL (`pageLocation`).
    #[default]
    PageLocation,
    /// Landingpage mit Query (`landingPagePlusQueryString`), ergänzt um den Host.
    LandingPage,
}

impl PageDimension {
    /// API-Name der Dimension.
    pub fn api(self) -> &'static str {
        match self {
            PageDimension::PageLocation => "pageLocation",
            PageDimension::LandingPage => "landingPagePlusQueryString",
        }
    }

    fn parse(text: &str) -> Result<PageDimension, SettingsError> {
        match text {
            "" | "page" | "pageLocation" => Ok(PageDimension::PageLocation),
            "landing" | "landingPage" | "landingPagePlusQueryString" => {
                Ok(PageDimension::LandingPage)
            }
            other => Err(SettingsError::BadDimension(other.to_string())),
        }
    }
}

/// Eine gewählte Metrik mit ihrer Seitendimension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetricSel {
    /// API-Name.
    pub api: String,
    /// Seitendimension.
    pub dimension: PageDimension,
}

impl MetricSel {
    /// Metrik mit Seitendimension.
    pub fn new(api: &str, dimension: PageDimension) -> MetricSel {
        MetricSel {
            api: api.to_string(),
            dimension,
        }
    }

    /// Schlüssel in `api_values.metric`: der API-Name, bei Landingpage mit Zusatz `@landing`.
    pub fn key(&self) -> String {
        value_key(&self.api, self.dimension)
    }

    /// Spaltenname `GA4 <Metrik>`, bei Landingpage mit Zusatz ` (Landing Page)` (INTEG-092/093).
    pub fn column_name(&self) -> String {
        column_name(&self.api, self.dimension)
    }
}

/// Schlüssel eines Wertes in `api_values.metric`.
pub fn value_key(api: &str, dimension: PageDimension) -> String {
    match dimension {
        PageDimension::PageLocation => api.to_string(),
        PageDimension::LandingPage => format!("{api}@landing"),
    }
}

/// Spaltenname einer Metrik.
pub fn column_name(api: &str, dimension: PageDimension) -> String {
    let base = catalog::base_column_name(api);
    match dimension {
        PageDimension::PageLocation => base,
        PageDimension::LandingPage => format!("{base} (Landing Page)"),
    }
}

/// Operator eines Dimensionsfilters (INTEG-094).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    /// Ist gleich.
    #[default]
    Exact,
    /// Enthält.
    Contains,
    /// Beginnt mit.
    BeginsWith,
    /// Endet mit.
    EndsWith,
    /// Regulärer Ausdruck (RE2, Teiltreffer).
    Regex,
}

/// Ein Dimensionsfilter; mehrere Filter sind UND-verknüpft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DimensionFilter {
    /// API-Name der Dimension (`sessionDefaultChannelGroup`).
    pub dimension: String,
    /// Operator.
    pub op: FilterOp,
    /// Wert.
    pub value: String,
    /// Negation („ist nicht“).
    pub negate: bool,
}

impl DimensionFilter {
    /// Filterausdruck der Data API.
    pub fn expression(&self) -> Value {
        let match_type = match self.op {
            FilterOp::Exact => "EXACT",
            FilterOp::Contains => "CONTAINS",
            FilterOp::BeginsWith => "BEGINS_WITH",
            FilterOp::EndsWith => "ENDS_WITH",
            FilterOp::Regex => "PARTIAL_REGEXP",
        };
        let filter = json!({"filter": {"fieldName": self.dimension,
            "stringFilter": {"matchType": match_type, "value": self.value, "caseSensitive": false}}});
        if self.negate {
            json!({ "notExpression": filter })
        } else {
            filter
        }
    }

    /// Lesbare Beschreibung für den Hinweis „Gefiltert: …“.
    pub fn describe(&self) -> String {
        let op = match (self.op, self.negate) {
            (FilterOp::Exact, false) => "=",
            (FilterOp::Exact, true) => "≠",
            (FilterOp::Contains, false) => "enthält",
            (FilterOp::Contains, true) => "enthält nicht",
            (FilterOp::BeginsWith, false) => "beginnt mit",
            (FilterOp::BeginsWith, true) => "beginnt nicht mit",
            (FilterOp::EndsWith, false) => "endet mit",
            (FilterOp::EndsWith, true) => "endet nicht mit",
            (FilterOp::Regex, false) => "passt auf",
            (FilterOp::Regex, true) => "passt nicht auf",
        };
        format!("{} {op} {}", self.dimension, self.value)
    }
}

/// Prüft einen RE2-Ausdruck: Syntax wie die Regex-Prüfung der Konfiguration; Rückverweise und Lookarounds kennt RE2 nicht.
pub fn check_regex(pattern: &str) -> Result<(), String> {
    for unsupported in ["(?=", "(?!", "(?<=", "(?<!"] {
        if pattern.contains(unsupported) {
            return Err(format!(
                "{unsupported} wird von GA4 (RE2) nicht unterstützt"
            ));
        }
    }
    let bytes = pattern.as_bytes();
    if bytes
        .windows(2)
        .any(|w| w[0] == b'\\' && w[1].is_ascii_digit() && w[1] != b'0')
    {
        return Err("Rückverweise werden von GA4 (RE2) nicht unterstützt".into());
    }
    // Syntaxprüfung über die Konfigurationsvalidierung (gleiche Regex-Bibliothek wie Custom Search).
    let cfg = os_model::CrawlConfig {
        custom_search: os_model::CustomSearchConfig {
            filters: vec![os_model::SearchFilter {
                name: "ga4".into(),
                kind: os_model::SearchKind::Regex,
                expression: pattern.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        },
        ..Default::default()
    };
    match cfg
        .validate()
        .errors
        .into_iter()
        .find(|e| e.path.starts_with("custom_search.filters[0].expression"))
    {
        Some(e) => Err(e.message),
        None => Ok(()),
    }
}

/// Inhalt von `api.ga4.filter`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct FilterSpec {
    filters: Vec<DimensionFilter>,
    max_results: Option<u64>,
    page_size: Option<u64>,
}

/// Wirksame GA4-Einstellungen.
#[derive(Debug, Clone, PartialEq)]
pub struct Ga4Settings {
    /// Numerische Property-Kennung.
    pub property: String,
    /// Numerische Kennung des Datenstreams; `None` = „Alle Datenstreams“.
    pub stream: Option<String>,
    /// Gewählte Metriken in Spaltenreihenfolge.
    pub metrics: Vec<MetricSel>,
    /// Zeitraum.
    pub date_range: DateRangeConfig,
    /// Dimensionsfilter (UND).
    pub filters: Vec<DimensionFilter>,
    /// Ergebnislimit (Top-N nach Sitzungen).
    pub max_results: u64,
    /// Seitengröße der Paginierung (auf 250.000 begrenzt).
    pub page_size: u64,
    /// URL-Abgleich.
    pub url_match: UrlMatchConfig,
}

impl Default for Ga4Settings {
    fn default() -> Self {
        Ga4Settings {
            property: String::new(),
            stream: None,
            metrics: catalog::standard_metrics()
                .into_iter()
                .map(|m| MetricSel::new(m, PageDimension::PageLocation))
                .collect(),
            date_range: DateRangeConfig::default(),
            filters: Vec::new(),
            max_results: DEFAULT_MAX_RESULTS,
            page_size: DEFAULT_PAGE_SIZE,
            url_match: UrlMatchConfig::default(),
        }
    }
}

/// Zerlegt `properties/<id>[/dataStreams/<stream>]`.
pub fn parse_property(text: &str) -> Result<(String, Option<String>), SettingsError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(SettingsError::NoProperty);
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let parts: Vec<&str> = text.split('/').collect();
    match parts.as_slice() {
        [id] if digits(id) => Ok((id.to_string(), None)),
        ["properties", id] if digits(id) => Ok((id.to_string(), None)),
        ["properties", id, "dataStreams", s] if digits(id) && digits(s) => {
            Ok((id.to_string(), Some(s.to_string())))
        }
        _ => Err(SettingsError::BadProperty(text.to_string())),
    }
}

impl Ga4Settings {
    /// Liest die Einstellungen aus der Konfiguration.
    pub fn from_config(cfg: &Ga4Config) -> Result<Ga4Settings, SettingsError> {
        let (property, stream) = parse_property(cfg.property.as_deref().unwrap_or_default())?;
        let global = PageDimension::parse(cfg.page_dimension.as_deref().unwrap_or_default())?;
        let mut metrics = Vec::new();
        let entries: Vec<String> = if cfg.metrics.is_empty() {
            catalog::standard_metrics()
                .into_iter()
                .map(str::to_string)
                .collect()
        } else {
            cfg.metrics.clone()
        };
        for entry in entries {
            let (api, dim) = match entry.split_once('@') {
                Some((api, dim)) => (api.trim(), PageDimension::parse(dim.trim())?),
                None => (entry.trim(), global),
            };
            let sel = MetricSel::new(api, dim);
            if !api.is_empty() && !metrics.contains(&sel) {
                metrics.push(sel);
            }
        }
        let spec = match cfg.filter.as_deref().map(str::trim) {
            None | Some("") => FilterSpec::default(),
            Some(text) if text.starts_with('[') => FilterSpec {
                filters: serde_json::from_str(text)
                    .map_err(|e| SettingsError::BadFilter(e.to_string()))?,
                ..Default::default()
            },
            Some(text) => {
                serde_json::from_str(text).map_err(|e| SettingsError::BadFilter(e.to_string()))?
            }
        };
        let settings = Ga4Settings {
            property,
            stream,
            metrics,
            date_range: cfg.date_range.clone(),
            filters: spec.filters,
            max_results: spec
                .max_results
                .unwrap_or(DEFAULT_MAX_RESULTS)
                .clamp(1, MAX_MAX_RESULTS),
            page_size: spec
                .page_size
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .clamp(1, MAX_PAGE_SIZE),
            url_match: cfg.url_match.clone(),
        };
        settings.check_filters()?;
        Ok(settings)
    }

    /// Schreibt die Einstellungen in die Konfiguration (Profil, INTEG-090: Property- und Stream-Kennung).
    pub fn to_config(&self, cfg: &mut Ga4Config) {
        cfg.property = Some(match &self.stream {
            Some(s) => format!("properties/{}/dataStreams/{s}", self.property),
            None => format!("properties/{}", self.property),
        });
        cfg.page_dimension = None;
        cfg.metrics = self
            .metrics
            .iter()
            .map(|m| match m.dimension {
                PageDimension::PageLocation => m.api.clone(),
                PageDimension::LandingPage => format!("{}@landing", m.api),
            })
            .collect();
        cfg.date_range = self.date_range.clone();
        let spec = FilterSpec {
            filters: self.filters.clone(),
            max_results: (self.max_results != DEFAULT_MAX_RESULTS).then_some(self.max_results),
            page_size: (self.page_size != DEFAULT_PAGE_SIZE).then_some(self.page_size),
        };
        cfg.filter = (spec != FilterSpec::default())
            .then(|| serde_json::to_string(&spec).expect("Filter serialisierbar"));
        cfg.url_match = self.url_match.clone();
    }

    /// Prüft die Filter vor dem Senden (INTEG-094: ungültiger Regex wird abgelehnt).
    pub fn check_filters(&self) -> Result<(), SettingsError> {
        for (index, f) in self.filters.iter().enumerate() {
            if f.dimension.trim().is_empty() {
                return Err(SettingsError::BadFilter(format!(
                    "Filter {index}: keine Dimension"
                )));
            }
            if f.op == FilterOp::Regex {
                check_regex(&f.value)
                    .map_err(|message| SettingsError::BadRegex { index, message })?;
            }
        }
        Ok(())
    }

    /// Seitendimensionen der Auswahl (in fester Reihenfolge, ohne Doppelte).
    pub fn page_dimensions(&self) -> Vec<PageDimension> {
        let mut dims: Vec<PageDimension> = self.metrics.iter().map(|m| m.dimension).collect();
        dims.sort();
        dims.dedup();
        if dims.is_empty() {
            dims.push(PageDimension::PageLocation);
        }
        dims
    }

    /// Abzufragende Metriken je Seitendimension: die Auswahl, ergänzt um `sessions` (Rangfolge des Abgleichs, Top-N,
    /// Filter „Sessions Above 0“) und `bounceRate` (Filter „Bounce Rate Above 70%“, INTEG-098), falls nicht gewählt.
    pub fn request_metrics(&self, dimension: PageDimension) -> Vec<String> {
        let mut out: Vec<String> = self
            .metrics
            .iter()
            .filter(|m| m.dimension == dimension)
            .map(|m| m.api.clone())
            .collect();
        for hidden in ["sessions", "bounceRate"] {
            if !out.iter().any(|m| m == hidden) {
                out.push(hidden.to_string());
            }
        }
        out
    }

    /// Filterausdruck: Dimensionsfilter (UND) plus optional der Hostname des Datenstreams (INTEG-090).
    pub fn dimension_filter(&self, stream_host: Option<&str>) -> Option<Value> {
        let mut parts: Vec<Value> = self
            .filters
            .iter()
            .map(DimensionFilter::expression)
            .collect();
        if let Some(host) = stream_host {
            parts.push(json!({"filter": {"fieldName": "hostName",
                "stringFilter": {"matchType": "EXACT", "value": host, "caseSensitive": false}}}));
        }
        match parts.len() {
            0 => None,
            1 => parts.pop(),
            _ => Some(json!({"andGroup": {"expressions": parts}})),
        }
    }

    /// Hinweis im Analytics-Tab: `Gefiltert: <Filter>` (INTEG-094); `None` ohne Filter.
    pub fn filter_hint(&self) -> Option<String> {
        (!self.filters.is_empty()).then(|| {
            format!(
                "Gefiltert: {}",
                self.filters
                    .iter()
                    .map(DimensionFilter::describe)
                    .collect::<Vec<_>>()
                    .join(" UND ")
            )
        })
    }

    /// Fingerabdruck der datenrelevanten Einstellungen (Cache des Reports).
    pub fn fingerprint(&self) -> String {
        format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}",
            self.property,
            self.stream,
            self.metrics,
            self.date_range,
            self.filters,
            self.max_results,
            self.page_size,
            (
                self.url_match.ignore_case,
                self.url_match.ignore_trailing_slash
            )
        )
    }
}

/// Zeitraum einer Anfrage (INTEG-091).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDates {
    /// `startDate` der Anfrage (`30daysAgo` oder `YYYY-MM-DD`).
    pub start: String,
    /// `endDate` der Anfrage (`yesterday`, `today` oder `YYYY-MM-DD`).
    pub end: String,
    /// Erster Tag (Anzeige im Tab-Hinweis).
    pub from: Date,
    /// Letzter Tag (Anzeige im Tab-Hinweis).
    pub to: Date,
}

impl RequestDates {
    /// Anzeige `YYYY-MM-DD – YYYY-MM-DD`.
    pub fn label(&self) -> String {
        format!("{} – {}", self.from, self.to)
    }
}

fn parse_date(text: &str) -> Result<Date, SettingsError> {
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .expect("Datumsformat");
    Date::parse(text.trim(), &format).map_err(|_| SettingsError::BadDate(text.to_string()))
}

/// Löst den Zeitraum auf. Vorgaben werden als relative Daten der Data API gesendet (`30daysAgo` bis `yesterday`), die
/// GA4 in der Zeitzone der Property auswertet; freie Zeiträume werden geprüft (Ende vor Start → Fehler, Ende in der
/// Zukunft → `today`). Eine Untergrenze für das Startdatum gibt es nicht (INTEG-091).
pub fn resolve_dates(range: &DateRangeConfig, today: Date) -> Result<RequestDates, SettingsError> {
    let yesterday = today.previous_day().unwrap_or(today);
    let days = match range.preset {
        DateRangePreset::Last7Days => Some(7),
        DateRangePreset::Last28Days => Some(28),
        DateRangePreset::Last30Days => Some(30),
        DateRangePreset::Last90Days => Some(90),
        DateRangePreset::Custom => None,
    };
    if let Some(days) = days {
        return Ok(RequestDates {
            start: format!("{days}daysAgo"),
            end: "yesterday".into(),
            from: today - time::Duration::days(days),
            to: yesterday,
        });
    }
    let from = parse_date(range.from.as_deref().unwrap_or_default())?;
    let to = parse_date(range.to.as_deref().unwrap_or_default())?;
    let (end, to) = if to > today {
        ("today".to_string(), today)
    } else {
        (to.to_string(), to)
    };
    if to < from {
        return Err(SettingsError::EndBeforeStart);
    }
    Ok(RequestDates {
        start: from.to_string(),
        end,
        from,
        to,
    })
}

/// Vorgaben ohne eigenen Konfigurationswert (14 Tage, 12 Monate) als freier Zeitraum bis gestern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraPreset {
    /// Letzte 14 Tage.
    Last14Days,
    /// Letzte 12 Monate.
    Last12Months,
}

/// Freier Zeitraum für eine [`ExtraPreset`]-Vorgabe.
pub fn extra_preset(preset: ExtraPreset, today: Date) -> DateRangeConfig {
    let from = match preset {
        ExtraPreset::Last14Days => today - time::Duration::days(14),
        ExtraPreset::Last12Months => today
            .replace_year(today.year() - 1)
            .unwrap_or(today - time::Duration::days(365)),
    };
    DateRangeConfig {
        preset: DateRangePreset::Custom,
        from: Some(from.to_string()),
        to: Some(today.previous_day().unwrap_or(today).to_string()),
    }
}
