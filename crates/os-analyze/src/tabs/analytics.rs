//! Tab „Analytics“: Google-Analytics-4-Daten (INTEG-097, INTEG-098, INTEG-100; M2-I02).
//!
//! Grundmenge: gecrawlte interne HTML-URLs der Tabellen (INTEG-097: eine Zeile je HTML-URL, URLs ohne Daten mit leeren
//! Metrikzellen). Die Werte schreibt die GA4-Quelle (`os_integrations::ga4`) in `api_values` (Quelle `ga4`, Metrik =
//! API-Name, bei Landingpage mit Zusatz `@landing`); `0` ist ein gelieferter Wert, eine fehlende Zeile bleibt leer.
//!
//! Die GA4-Spalten (`GA4 <Metrik>`, bei Landingpage `GA4 <Metrik> (Landing Page)`) liefert [`column_source`] aus der
//! Metrikauswahl `api.ga4.metrics` (leer = sieben Standardmetriken); die Namen stammen aus dem gemeinsamen Katalog
//! `os-integrations/src/ga4/metrics.json`. Als gewöhnliche Spalten sind sie in Regelsuche, Sortierung und Segmenten
//! nutzbar (INTEG-100).
//!
//! Filter (INTEG-098), alle live:
//! - **Sessions Above 0:** `sessions` ≥ 1 (eine der Seitendimensionen).
//! - **Bounce Rate Above 70%:** `bounceRate` > 0,70 (streng) bei mindestens einer Sitzung derselben Seitendimension;
//!   die Quelle fragt `bounceRate` und `sessions` immer mit ab, auch wenn sie nicht gewählt sind.
//! - **No GA Data:** Status 200 ohne jede GA4-Zeile.
//! - **Non-Indexable with GA Data:** nicht indexierbar und mindestens ein GA4-Wert > 0.
//! - **Orphan URLs:** URL mit GA4-Daten ohne internen Link (keine Start-URL), erst nach der Crawl-Analyse
//!   (`crawl_meta.analysis_state` `completed`/`stale`); beachtet `issues.ignore_non_indexable` (CRAWL-089).

use std::sync::OnceLock;

use os_model::CrawlConfig;
use serde_json::Value;

use crate::defs::{
    ColumnDef, ColumnKind, ColumnSource, FilterDef, FilterKind, Needs, Requirement, SortKey, TabDef,
};

/// Grundmenge des Tabs.
const POPULATION: &str = "u.is_internal = 1 AND u.in_tables = 1 AND (u.content_class = 'HTML' OR EXISTS(SELECT 1 FROM crawl_meta WHERE key='source_mode' AND value='apis'))";

/// Die URL hat mindestens eine GA4-Zeile.
macro_rules! has_ga {
    () => {
        "EXISTS (SELECT 1 FROM api_values a WHERE a.url_id = u.id AND a.source = 'ga4')"
    };
}

const SESSIONS_ABOVE_0: &str =
    "EXISTS (SELECT 1 FROM api_values a WHERE a.url_id = u.id AND a.source = 'ga4' \
     AND a.metric IN ('sessions', 'sessions@landing') AND a.v_real >= 1)";

const BOUNCE_ABOVE_70: &str = "EXISTS (SELECT 1 FROM api_values b JOIN api_values s ON s.url_id = b.url_id \
     AND s.source = 'ga4' AND s.metric = replace(b.metric, 'bounceRate', 'sessions') \
     WHERE b.url_id = u.id AND b.source = 'ga4' AND b.metric IN ('bounceRate', 'bounceRate@landing') \
     AND b.v_real > 0.7 AND s.v_real >= 1)";

const NO_GA_DATA: &str = concat!("u.status_code = 200 AND NOT ", has_ga!());

const NON_INDEXABLE_WITH_GA: &str =
    "u.indexability = 'Non-Indexable' AND EXISTS (SELECT 1 FROM api_values a \
     WHERE a.url_id = u.id AND a.source = 'ga4' AND a.v_real > 0)";

const ORPHAN: &str = concat!(
    has_ga!(),
    " AND u.discovered_via <> 'start' \
     AND COALESCE((SELECT c.inlinks - c.self_inlinks FROM url_counters c WHERE c.url_id = u.id), 0) = 0 \
     AND EXISTS (SELECT 1 FROM crawl_meta m WHERE m.key = 'analysis_state' AND m.value IN ('completed', 'stale'))"
);

fn filter(id: &'static str, predicate: &'static str, ignore_non_indexable: bool) -> FilterDef {
    FilterDef {
        id,
        predicate,
        kind: FilterKind::Live,
        requires: vec![Requirement::Config("api.ga4.enabled")],
        ignore_non_indexable,
        default_columns: &[],
    }
}

/// Definition des Tabs „Analytics“ (Basisspalten; die GA4-Spalten liefert [`column_source`]).
pub fn tab() -> TabDef {
    TabDef {
        id: "Analytics",
        population: POPULATION,
        hidden_population: None,
        columns: vec![
            ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
            ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
            ColumnDef::new("Indexability", "u.indexability", ColumnKind::Text, true),
            ColumnDef::new(
                "Indexability Status",
                "u.indexability_status",
                ColumnKind::Text,
                true,
            ),
        ],
        filters: vec![
            FilterDef::placeholder("All", FilterKind::Live, &[]),
            filter("Sessions Above 0", SESSIONS_ABOVE_0, false),
            filter("Bounce Rate Above 70%", BOUNCE_ABOVE_70, false),
            filter("No GA Data", NO_GA_DATA, false),
            filter("Non-Indexable with GA Data", NON_INDEXABLE_WITH_GA, false),
            // CRAWL-089: der Orphan-Filter der Integrationen beachtet `issues.ignore_non_indexable`.
            filter("Orphan URLs", ORPHAN, true),
        ],
        default_sort: vec![SortKey {
            column: "Address".into(),
            descending: false,
        }],
        params: |_| Vec::new(),
    }
}

/// Eine Metrik des gemeinsamen Katalogs: Spaltenname ohne Präfix und Werteart.
struct CatalogMetric {
    api: String,
    name: String,
    kind: String,
}

struct Catalog {
    standard: Vec<String>,
    metrics: Vec<CatalogMetric>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!(
            "../../../os-integrations/src/ga4/metrics.json"
        ))
        .expect("GA4-Katalog ist gültiges JSON");
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
                .map(|m| CatalogMetric {
                    api: text(&m["api"]),
                    name: text(&m["name"]),
                    kind: text(&m["kind"]),
                })
                .collect(),
        }
    })
}

/// Name ohne Präfix für Metriken außerhalb des Katalogs (benutzerdefiniert): `customEvent:fixture_score` → `Fixture Score`.
fn fallback_name(api: &str) -> String {
    api.rsplit(':')
        .next()
        .unwrap_or(api)
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Eine GA4-Spalte der Konfiguration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ga4Column {
    /// Spaltenname (`GA4 Sessions`, `GA4 Sessions (Landing Page)`).
    pub name: String,
    /// Schlüssel in `api_values.metric`.
    pub metric: String,
}

/// Die GA4-Spalten der Metrikauswahl in Spaltenreihenfolge (INTEG-092, INTEG-093, INTEG-097).
pub fn ga4_columns(cfg: &CrawlConfig) -> Vec<(Ga4Column, ColumnKind)> {
    let ga4 = &cfg.api.ga4;
    let landing_default = matches!(
        ga4.page_dimension.as_deref(),
        Some("landing" | "landingPage" | "landingPagePlusQueryString")
    );
    let entries: Vec<String> = if ga4.metrics.is_empty() {
        catalog().standard.clone()
    } else {
        ga4.metrics.clone()
    };
    let mut out: Vec<(Ga4Column, ColumnKind)> = Vec::new();
    for entry in entries {
        let (api, landing) = match entry.split_once('@') {
            Some((api, dim)) => (
                api.trim().to_string(),
                matches!(
                    dim.trim(),
                    "landing" | "landingPage" | "landingPagePlusQueryString"
                ),
            ),
            None => (entry.trim().to_string(), landing_default),
        };
        if api.is_empty() {
            continue;
        }
        let known = catalog().metrics.iter().find(|m| m.api == api);
        let base = known.map_or_else(|| fallback_name(&api), |m| m.name.clone());
        let kind = match known.map(|m| m.kind.as_str()) {
            Some("integer") => ColumnKind::Int,
            Some("percent") => ColumnKind::Float { decimals: 4 },
            _ => ColumnKind::Float { decimals: 2 },
        };
        let column = if landing {
            Ga4Column {
                name: format!("GA4 {base} (Landing Page)"),
                metric: format!("{api}@landing"),
            }
        } else {
            Ga4Column {
                name: format!("GA4 {base}"),
                metric: api,
            }
        };
        if !out.iter().any(|(c, _)| c.name == column.name) {
            out.push((column, kind));
        }
    }
    out
}

struct AnalyticsColumns;

impl AnalyticsColumns {
    fn defs(cfg: &CrawlConfig) -> Vec<ColumnDef> {
        ga4_columns(cfg)
            .into_iter()
            .map(|(c, kind)| {
                let sql = format!(
                    "(SELECT a.v_real FROM api_values a WHERE a.url_id = u.id AND a.source = 'ga4' \
                     AND a.metric = '{}' AND a.level = 'url')",
                    c.metric.replace('\'', "''")
                );
                ColumnDef::dynamic(&c.name, &sql, kind, true).with_needs(Needs::Api("ga4"))
            })
            .collect()
    }
}

impl ColumnSource for AnalyticsColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        if cfg.api.ga4.enabled {
            Self::defs(cfg)
        } else {
            Vec::new()
        }
    }

    fn columns_for_file(&self, cfg: &CrawlConfig, file: &os_store::CrawlFile) -> Vec<ColumnDef> {
        // Gespeicherte GA4-Daten bleiben sichtbar, auch wenn die Quelle inzwischen getrennt ist (INTEG-001).
        let stored = file
            .conn()
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM api_values WHERE source = 'ga4')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false);
        if cfg.api.ga4.enabled || stored {
            Self::defs(cfg)
        } else {
            Vec::new()
        }
    }
}

/// Quelle der GA4-Spalten des Tabs „Analytics“.
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(AnalyticsColumns))
}
