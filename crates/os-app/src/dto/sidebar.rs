//! Seitenleiste: Overview, Issues, Site Structure, Response Times, Segmente, API-Status (`sidebar.*`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::{FilterInfo, IssuePriorityDto, IssueTypeDto};
use super::view::{CompareClass, SegmentFilter};

/// Eine Kennzahl der Zusammenfassung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SummaryRow {
    /// Kanon-Name der Kennzahl.
    pub key: String,
    /// Wert.
    pub count: u64,
    /// Anteil in Prozent.
    pub pct: Option<f64>,
}

/// Filter eines Tabs in der Overview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OverviewTab {
    /// Tab.
    pub tab: String,
    /// Filter mit Zählern.
    pub filters: Vec<FilterInfo>,
}

/// Antwort von `sidebar.overview` (UI-061, OUT-142).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OverviewDto {
    /// Zusammenfassung.
    pub summary: Vec<SummaryRow>,
    /// Tabs mit Filtern.
    pub tabs: Vec<OverviewTab>,
}

/// Argumente von `sidebar.issues` (UI-062).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct IssuesArgs {
    /// Nur diese Art.
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub issue_type: Option<IssueTypeDto>,
    /// Nur diese Priorität.
    pub priority: Option<IssuePriorityDto>,
    /// Suchtext.
    pub search: Option<String>,
    /// Segment.
    pub segment: Option<SegmentFilter>,
    /// Vergleichsklasse.
    pub compare: Option<CompareClass>,
}

/// Anteil eines Segments an einem Issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentShare {
    /// Segmentname.
    pub segment: String,
    /// URLs des Segments mit diesem Befund.
    pub count: u64,
}

/// Eine Zeile der Issue-Liste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct IssueRow {
    /// Kennung (`<Tab>/<Filter>`).
    pub id: String,
    /// Tab.
    pub tab: String,
    /// Filter.
    pub filter: String,
    /// Art.
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub issue_type: IssueTypeDto,
    /// Priorität.
    pub priority: IssuePriorityDto,
    /// Betroffene URLs.
    pub count: u64,
    /// Anteil in Prozent.
    pub pct: Option<f64>,
    /// Anteile der Segmente.
    pub segment_shares: Vec<SegmentShare>,
    /// Im Vergleichsmodus: Differenz zum Vergleichscrawl.
    pub delta: Option<i64>,
}

/// Argumente von `sidebar.issueExplain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueExplainArgs {
    /// Issue.
    pub issue_id: String,
    /// Sprache (`de`, `en`).
    pub lang: String,
}

/// Erklärungstext eines Issues (UI-065, eine Quelle für UI, Export und MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct IssueText {
    /// Was ist das?
    pub what: String,
    /// Warum ist es wichtig?
    pub why: String,
    /// Wie wird es behoben?
    pub fix: String,
    /// Regel/Quelle.
    pub rule: Option<String>,
    /// Weiterführender Link.
    pub learn_more_url: Option<String>,
}

/// Ansicht der Site-Structure-Aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SiteStructureView {
    /// Nach Indexierbarkeit.
    Indexability,
    /// Nach Statusklasse.
    ResponseCodes,
    /// Nach Crawl-Tiefe.
    CrawlDepth,
}

/// Argumente von `sidebar.siteStructure`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SiteStructureArgs {
    /// Ansicht.
    pub view: SiteStructureView,
    /// Segment.
    pub segment: Option<SegmentFilter>,
    /// Vertragserweiterung M2-F05b: path.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub path: Option<String>,

    /// Vertragserweiterung M2-F05b: offset.
    pub offset: u64,

    /// Vertragserweiterung M2-F05b: limit.
    pub limit: u32,
}

/// Knoten des Verzeichnisbaums (UI-066).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PathNode {
    /// Name des Segments.
    pub name: String,
    /// Vollständiger Pfad.
    pub path: String,
    /// URLs unterhalb.
    pub count: u64,
    /// Aufteilung nach der gewählten Ansicht.
    pub by_class: BTreeMap<String, u64>,
    /// Kindknoten (Tiefe begrenzt, der Rest über weitere Aufrufe).
    pub children: Vec<PathNode>,
    /// Vertragserweiterung M2-F05b: has_more_children.
    pub has_more_children: bool,
}

/// Zeile der Tiefenverteilung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DepthRow {
    /// Crawl-Tiefe.
    pub depth: u32,
    /// URLs.
    pub count: u64,
    /// Aufteilung nach der gewählten Ansicht.
    pub by_class: BTreeMap<String, u64>,
}

/// Antwort von `sidebar.siteStructure`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SiteStructure {
    /// Verzeichnisbaum.
    pub paths: Vec<PathNode>,
    /// Tiefenverteilung.
    pub depths: Vec<DepthRow>,
}

/// Grundmenge der Antwortzeiten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResponseTimeScope {
    /// Alle URLs.
    All,
    /// Interne HTML-Seiten.
    InternalHtml,
}

/// Argumente von `sidebar.responseTimes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTimesArgs {
    /// Grundmenge.
    pub scope: ResponseTimeScope,
}

/// Ein Eimer des Histogramms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    /// Untergrenze in ms (inklusive).
    pub from_ms: u32,
    /// Obergrenze in ms (exklusive).
    pub to_ms: Option<u32>,
    /// URLs.
    pub count: u64,
}

/// Antwort von `sidebar.responseTimes` (UI-069).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTimes {
    /// Eimer.
    pub buckets: Vec<Bucket>,
    /// URLs ohne Antwort.
    pub no_response: u64,
}

/// Argumente von `sidebar.segments`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentsViewArgs {
    /// Aufteilung.
    pub view: SiteStructureView,
}

/// Kennzahlen eines Segments (UI-067).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentRow {
    /// Name; `null` = „Kein Segment“.
    #[ts(optional = nullable)]
    pub name: Option<String>,
    /// Farbe `#rrggbb`.
    #[ts(optional = nullable)]
    pub color: Option<String>,
    /// URLs.
    pub urls: u64,
    /// Aufteilung nach der gewählten Ansicht.
    pub by_class: BTreeMap<String, u64>,
}
