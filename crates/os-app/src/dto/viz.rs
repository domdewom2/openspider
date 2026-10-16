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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VizExportArgs {
    /// Schnappschuss.
    pub snapshot_id: String,
    /// Zielordner (`nodes.csv`, `edges.csv`).
    pub dir: String,
}
