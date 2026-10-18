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
    /// Kennung des zu erneuernden Visualisierungs-Snapshots.
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
    /// Auch URLs ohne HTML-Inhalt einbeziehen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_non_html: Option<bool>,

    /// Auch nicht indexierbare URLs einbeziehen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_non_indexable: Option<bool>,

    /// Anzahl der beim Öffnen aufzuklappenden Ebenen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub expand_depth: Option<u32>,

    /// Kennung des zu erkundenden Knotens.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub explore_node: Option<i64>,

    /// Kennzahl für die Färbung der Knoten.
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
    /// Independent numeric color metric (UI-095).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub color_value: Option<f64>,
    /// Current layout coordinates, supplied by the visualization for CSV export.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub x: Option<f64>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub y: Option<f64>,
    /// Knoten hat weitere Kinder außerhalb des Budgets.
    pub has_more: bool,
    /// HTTP-Statuscode der URL.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub status_code: Option<u16>,

    /// Indexierbarkeitsstatus der URL.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub indexability: Option<String>,

    /// Anzahl eindeutiger eingehender Links.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub unique_inlinks: Option<u64>,

    /// Berechneter Link-Score der URL.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub link_score: Option<f64>,

    /// Kennung des zugeordneten Segments.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub segment_id: Option<String>,

    /// Anzeigefarbe des zugeordneten Segments.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub segment_color: Option<String>,

    /// Anzahl der ausgeblendeten direkten Kinder.
    pub hidden_children: u64,

    /// Anzahl aller ausgeblendeten Nachfahren.
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
    /// Legacy crawls use a link-based BFS when discovery provenance is absent.
    #[serde(default)]
    pub provenance_missing: bool,
    /// Numeric metrics available in this immutable snapshot.
    #[serde(default)]
    pub available_metrics: Vec<String>,
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
    /// Ausgewählter Auswertungsmodus.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub mode: Option<String>,

    /// Stoppwörter aus der Wortauswertung entfernen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub remove_stopwords: Option<bool>,

    /// Sprache für die Wortauswertung.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub language: Option<String>,

    /// Maximale Anzahl ausgegebener Begriffe.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub max_terms: Option<u32>,

    /// Auch Links mit nofollow einbeziehen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_nofollow: Option<bool>,

    /// Links auf dieselbe URL ausschließen.
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
    /// Knoten des Graphen.
    pub nodes: Vec<GraphNode>,

    /// Kanten des Graphen.
    pub edges: Vec<GraphEdge>,

    /// Anordnung der Knoten im Graphen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub layout: Option<String>,
}
