//! Visualisierungen (`viz.*`, UI-090 … UI-100).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Art der Visualisierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum VizKind {
    /// Crawl-Baum (UI-091).
    CrawlTree,
    /// Kraftgerichteter Crawl-Graph (UI-092).
    CrawlGraph,
    /// Verzeichnisbaum (UI-093).
    DirectoryTree,
    /// Kraftgerichteter Verzeichnisgraph (UI-094).
    DirectoryGraph,
    /// 3D-Variante des Crawl-Graphen (UI-098).
    CrawlGraph3d,
    /// 3D-Variante des Verzeichnisgraphen (UI-098).
    DirectoryGraph3d,
    /// Inlink-Ankertext-Wortwolke (UI-099).
    AnchorWords,
    /// Body-Text-Wortwolke (UI-100).
    BodyWords,
}

/// Argument von `viz.open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct VizOpenArgs {
    /// Art.
    pub kind: VizKind,
    /// Wurzel; `null` = Start-URL bzw. Wurzelverzeichnis.
    pub root_url_id: Option<i64>,
    /// Vertragserweiterung M2-F05b: refresh_snapshot_id.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub refresh_snapshot_id: Option<String>,
}

/// Antwort von `viz.open`: das Fenster arbeitet auf einem Schnappschuss (K-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VizOpenResult {
    /// Schnappschuss.
    pub snapshot_id: String,
    /// Name des geöffneten Fensters (`viz-<n>`).
    pub window: String,
}

/// Argument von `viz.graph`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct VizGraphArgs {
    /// Schnappschuss.
    pub snapshot_id: String,
    /// Art.
    pub kind: VizKind,
    /// Wurzel des Teilbaums (UI-096).
    pub root: Option<i64>,
    /// Knotenbudget.
    pub budget: u32,
    /// Spalte, nach der Farbe und Größe bestimmt werden (UI-095).
    pub metric: String,
    /// Vertragserweiterung M2-F05b: include_non_html.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_non_html: Option<bool>,

    /// Vertragserweiterung M2-F05b: include_non_indexable.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_non_indexable: Option<bool>,

    /// Vertragserweiterung M2-F05b: expand_depth.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub expand_depth: Option<u32>,

    /// Vertragserweiterung M2-F05b: explore_node.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub explore_node: Option<i64>,

    /// Vertragserweiterung M2-F05b: color_metric.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub color_metric: Option<String>,
}

/// Ein Knoten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct GraphNode {
    /// Knoten-Kennung (URL-Kennung bzw. Verzeichnisnummer).
    pub id: i64,
    /// Beschriftung.
    pub label: String,
    /// Adresse.
    pub address: Option<String>,
    /// Tiefe.
    pub depth: u32,
    /// Wert der Metrik.
    pub metric: Option<f64>,
    /// Knoten hat weitere Kinder außerhalb des Budgets.
    pub has_more: bool,
    /// Vertragserweiterung M2-F05b: status_code.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub status_code: Option<u16>,

    /// Vertragserweiterung M2-F05b: indexability.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub indexability: Option<String>,

    /// Vertragserweiterung M2-F05b: unique_inlinks.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub unique_inlinks: Option<u64>,

    /// Vertragserweiterung M2-F05b: link_score.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub link_score: Option<f64>,

    /// Vertragserweiterung M2-F05b: segment_id.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub segment_id: Option<String>,

    /// Vertragserweiterung M2-F05b: segment_color.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub segment_color: Option<String>,

    /// Vertragserweiterung M2-F05b: hidden_children.
    pub hidden_children: u64,

    /// Vertragserweiterung M2-F05b: hidden_descendants.
    pub hidden_descendants: u64,
}

/// Eine Kante.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    /// Quelle.
    pub source: i64,
    /// Ziel.
    pub target: i64,
}

/// Antwort von `viz.graph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    /// Knoten.
    pub nodes: Vec<GraphNode>,
    /// Kanten.
    pub edges: Vec<GraphEdge>,
    /// Das Knotenbudget hat gekürzt.
    pub truncated: bool,
}

/// Quelle der Wortwolke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WordSource {
    /// Ankertexte der Inlinks.
    Anchors,
    /// Text des Inhaltsbereichs.
    Body,
}

/// Argument von `viz.words`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VizWordsArgs {
    /// Schnappschuss.
    pub snapshot_id: String,
    /// Quelle.
    pub kind: WordSource,
    /// URL.
    pub url_id: i64,
    /// Vertragserweiterung M2-F05b: mode.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub mode: Option<String>,

    /// Vertragserweiterung M2-F05b: remove_stopwords.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub remove_stopwords: Option<bool>,

    /// Vertragserweiterung M2-F05b: language.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub language: Option<String>,

    /// Vertragserweiterung M2-F05b: max_terms.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub max_terms: Option<u32>,

    /// Vertragserweiterung M2-F05b: include_nofollow.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_nofollow: Option<bool>,

    /// Vertragserweiterung M2-F05b: exclude_self_links.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub exclude_self_links: Option<bool>,
}

/// Wort und Häufigkeit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WordFreq {
    /// Wort.
    pub word: String,
    /// Häufigkeit.
    pub count: u64,
}

/// Argument von `viz.exportCsv`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VizExportArgs {
    /// Schnappschuss.
    pub snapshot_id: String,
    /// Zielordner (`nodes.csv`, `edges.csv`).
    pub dir: String,
    /// Vertragserweiterung M2-F05b: nodes.
    pub nodes: Vec<GraphNode>,

    /// Vertragserweiterung M2-F05b: edges.
    pub edges: Vec<GraphEdge>,

    /// Vertragserweiterung M2-F05b: layout.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub layout: Option<String>,
}
