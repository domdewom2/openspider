//! Kataloge: Tabs, Filter, Zähler (`catalog.*`, `M2-ui-concept.md` 4.3).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::view::{CompareClass, SegmentFilter};

/// Zustand eines Filters (UI-006, ONPAGE-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilterStateDto {
    /// Zahl liegt vor.
    Ready,
    /// Crawl-Analyse erforderlich.
    NeedsCrawlAnalysis,
    /// Konfigurationsschlüssel muss an sein.
    #[serde(rename_all = "camelCase")]
    NeedsConfig {
        /// Schlüssel.
        key: String,
    },
    /// Nur mit JavaScript-Rendering befüllbar.
    NeedsRendering,
    /// Analyse veraltet.
    Stale,
    /// Vertragserweiterung M2-F05b.
    NeedsRecrawl,
    /// Vertragserweiterung M2-F05b.
    Incomplete { unchecked: u64 },
}

/// Art eines Befunds (UI-063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IssueTypeDto {
    /// Fehler.
    Issue,
    /// Warnung.
    Warning,
    /// Chance.
    Opportunity,
}

/// Priorität eines Befunds (UI-063).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IssuePriorityDto {
    /// Hoch.
    High,
    /// Mittel.
    Medium,
    /// Niedrig.
    Low,
}

/// Typ und Priorität eines Filters, der zugleich ein Issue ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueBadge {
    /// Typ.
    pub issue_type: IssueTypeDto,
    /// Priorität.
    pub priority: IssuePriorityDto,
}

/// Ein Filter mit Zähler (`catalog.filterCounts`, `sidebar.overview`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct FilterInfo {
    /// Kanonische Kennung (mit aufgelösten Schwellen, z. B. `Over 60 Characters`).
    pub id: String,
    /// Vorlage der Kennung mit Platzhaltern (für Übersetzung).
    pub label_template: String,
    /// Werte der Platzhalter.
    pub params: BTreeMap<String, serde_json::Value>,
    /// Zähler; `null` = noch nicht berechnet.
    pub count: Option<u64>,
    /// Anteil an der Grundmenge in Prozent.
    pub pct: Option<f64>,
    /// Zustand.
    pub state: FilterStateDto,
    /// Issue-Typ und -Priorität, falls der Filter ein Issue ist.
    pub issue: Option<IssueBadge>,
    /// Analyse-Teilschritt, von dem der Filter abhängt.
    pub analysis_step: Option<String>,
}

/// Ein Tab der oberen Tabelle (`catalog.tabs`, ohne Zähler).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    /// Kennung (Kanon-Name, z. B. `Internal`).
    pub id: String,
    /// Standardposition.
    pub default_order: u16,
    /// Filter des Tabs (Zähler leer).
    pub filters: Vec<FilterInfo>,
    /// Hat tabellarische Detailansichten.
    pub detail_capable: bool,
}

/// Argumente von `catalog.columns`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ColumnsArgs {
    /// Tab-Kennung.
    pub tab: String,
}

/// Argumente von `catalog.filterCounts` und den Seitenleisten-Commands mit Bereichseinschränkung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ScopeArgs {
    /// Segment.
    pub segment: Option<SegmentFilter>,
    /// Vergleichsklasse (Vergleichsmodus).
    pub compare: Option<CompareClass>,
}

/// Antwort von `catalog.filterCounts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilterCounts {
    /// Version der Zähler (`Tick.counts_version`).
    pub version: u64,
    /// Filter aller Tabs mit Zählern.
    pub filters: Vec<FilterCountRow>,
}

/// Zähler eines Filters innerhalb eines Tabs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FilterCountRow {
    /// Tab.
    pub tab: String,
    /// Filter mit Zähler.
    pub filter: FilterInfo,
}
