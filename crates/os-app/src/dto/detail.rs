//! Nicht-tabellarische Detaildaten (`detail.*`, `crawl.path`; tabellarische Detail-Tabs laufen über `view.*`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{Header, NameValue};

/// Gerät der SERP-Vorschau (ONPAGE-194).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SerpDevice {
    /// Desktop.
    Desktop,
    /// Mobil.
    Mobile,
    /// Tablet-Geräteprofil für die SERP-Vorschau.
    Tablet,
}

/// Gespeicherte Bearbeitung des Snippets (K-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SerpEdit {
    /// Bearbeiteter Titel.
    pub title: Option<String>,
    /// Bearbeitete Beschreibung.
    pub description: Option<String>,
    /// Zeitpunkt (ISO 8601 UTC).
    pub edited_at: String,
}

/// Argumente von `detail.serp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerpArgs {
    /// URL.
    pub url_id: i64,
    /// Gerät.
    pub device: SerpDevice,
}

/// Antwort von `detail.serp` (UI-037).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SerpData {
    /// Titel.
    pub title: String,
    /// Beschreibung.
    pub description: String,
    /// Host.
    pub host: String,
    /// Brotkrumen-Pfad.
    pub breadcrumb: String,
    /// Favicon (URL-Schema `ospider://img/…`).
    pub favicon: Option<String>,
    /// Bearbeitung, falls vorhanden.
    pub edited: Option<SerpEdit>,
}

/// Argumente von `detail.serpMeasure` (UI-038, < 16 ms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerpMeasureArgs {
    /// Titel.
    pub title: String,
    /// Beschreibung.
    pub description: String,
    /// Gerät.
    pub device: SerpDevice,
    /// Fett darzustellende Suchbegriffe.
    pub bold_terms: Vec<String>,
    /// Text vor dem Titel/der Beschreibung (z. B. Datum).
    pub prefix: String,
}

/// Messung einer Zeile des Snippets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SerpMeasureRow {
    /// Zeichen.
    pub chars: u32,
    /// Breite in Pixeln.
    pub px: f64,
    /// Höchstbreite in Pixeln.
    pub max_px: f64,
    /// Verbleibende Pixel (negativ = zu lang).
    pub remaining: f64,
    /// Wird abgeschnitten.
    pub truncated: bool,
    /// Zeichenposition der Kürzung.
    pub cut_at: Option<u32>,
}

/// Antwort von `detail.serpMeasure`: Titel, dann Beschreibung (bei Mobil mehrere Zeilen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SerpMeasure {
    /// Zeilen.
    pub rows: Vec<SerpMeasureRow>,
}

/// Argumente von `detail.serpSaveEdit` und `detail.serpReset` (Reset: beide Felder ignoriert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SerpEditArgs {
    /// URL.
    pub url_id: i64,
    /// Titel.
    pub title: Option<String>,
    /// Beschreibung.
    pub description: Option<String>,
}

/// Antwort von `detail.headers` (UI-042).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HeadersDto {
    /// Statuszeile.
    pub status_line: String,
    /// Request-Header.
    pub request: Vec<Header>,
    /// Response-Header.
    pub response: Vec<Header>,
}

/// Fassung des Quelltextes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SourceWhich {
    /// Original-HTML.
    Original,
    /// Gerendertes HTML.
    Rendered,
}

/// Darstellung des Quelltextes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SourceMode {
    /// HTML.
    Html,
    /// Reiner Text.
    Text,
}

/// Argumente von `detail.sourceOpen`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceOpenArgs {
    /// URL.
    pub url_id: i64,
    /// Fassung.
    pub which: SourceWhich,
    /// Darstellung.
    pub mode: SourceMode,
    /// Formatiert ausgeben.
    pub pretty: bool,
}

/// Antwort von `detail.sourceOpen` (UI-041; bis 20 MB virtualisiert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceDoc {
    /// Kennung des geöffneten Dokuments.
    pub doc_id: String,
    /// Zeilenzahl.
    pub lines: u64,
    /// Größe in Byte.
    pub bytes: u64,
}

/// Argumente von `detail.sourceLines`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceLinesArgs {
    /// Dokument.
    pub doc_id: String,
    /// Erste Zeile (0-basiert).
    pub from: u64,
    /// Anzahl Zeilen.
    pub count: u32,
}

/// Granularität des Vergleichs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DiffMode {
    /// Zeilenweise.
    Lines,
    /// Wortweise.
    Words,
}

/// Argumente von `detail.sourceDiff` (Original gegen gerendert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceDiffArgs {
    /// URL.
    pub url_id: i64,
    /// Granularität.
    pub mode: DiffMode,
}

/// Art einer Diff-Zeile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DiffKind {
    /// Unverändert.
    Context,
    /// Hinzugefügt.
    Added,
    /// Entfernt.
    Removed,
}

/// Eine Zeile eines Diff-Blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    /// Art.
    pub kind: DiffKind,
    /// Text.
    pub text: String,
}

/// Ein Block des Diffs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    /// Erste Zeile links (1-basiert).
    pub old_start: u64,
    /// Zeilen links.
    pub old_lines: u64,
    /// Erste Zeile rechts.
    pub new_start: u64,
    /// Zeilen rechts.
    pub new_lines: u64,
    /// Inhalt.
    pub lines: Vec<DiffLine>,
}

/// Ergebnis von `detail.sourceDiff` (Ergebnis der Operation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunks {
    /// Hinzugefügte Einheiten.
    pub added: u64,
    /// Entfernte Einheiten.
    pub removed: u64,
    /// Blöcke.
    pub hunks: Vec<DiffHunk>,
}

/// Antwort von `detail.screenshot` (UI-039).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotInfo {
    /// Basis-URL `ospider://shot/<crawl>/<urlId>`; Kacheln als `/<tile>`.
    pub url: String,
    /// Breite in Pixeln.
    pub width: u32,
    /// Höhe in Pixeln.
    pub height: u32,
    /// Anzahl Kacheln.
    pub tiles: u32,
}

/// Antwort von `detail.imagePreview` (UI-035).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImagePreview {
    /// URL `ospider://img/…`.
    pub url: String,
    /// Breite in Pixeln.
    pub width: u32,
    /// Höhe in Pixeln.
    pub height: u32,
    /// Bild ist in der Crawl-Datei gespeichert (sonst Live-Abruf).
    pub stored: bool,
}

/// Befund an einem Knoten der strukturierten Daten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SdIssueDto {
    /// `error` oder `warning`.
    pub severity: String,
    /// `parse`, `schema_org` oder `rich_results`.
    pub validation: String,
    /// Rich-Result-Feature.
    pub feature: Option<String>,
    /// Eigenschaft.
    pub property: Option<String>,
    /// Maschinenlesbarer Code.
    pub code: String,
    /// Beschreibung des Befunds.
    pub message: String,

    /// Pfad zur betroffenen Eigenschaft in den strukturierten Daten.
    pub path: String,

    /// Zeilennummer im Quelldokument.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub line: Option<u32>,

    /// Spaltennummer im Quelldokument.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub col: Option<u32>,

    /// Quelltextausschnitt zum Befund.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub snippet: Option<String>,

    /// URL der weiterführenden Dokumentation.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub doc_url: Option<String>,
}

/// Knoten des Baums der strukturierten Daten (UI-045).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SdNode {
    /// Laufende Nummer.
    pub id: u32,
    /// Format (`json-ld`, `microdata`, `rdfa`).
    pub format: String,
    /// Typpfad.
    pub type_path: String,
    /// Eigenschaften mit einfachem Wert.
    pub props: Vec<NameValue>,
    /// Verschachtelte Knoten.
    pub children: Vec<SdNode>,
    /// Befunde.
    pub issues: Vec<SdIssueDto>,
    /// Pfad zur betroffenen Eigenschaft in den strukturierten Daten.
    pub path: String,

    /// Zeilennummer der Fundstelle im Quelltext.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub source_line: Option<u32>,

    /// Spaltennummer der Fundstelle im Quelltext.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub source_col: Option<u32>,

    /// Reihenfolge der Eigenschaften im Quelldokument.
    pub property_order: Vec<String>,
}

/// Antwort von `detail.structuredData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SdTree {
    /// Wurzelknoten.
    pub nodes: Vec<SdNode>,
}

/// Art der Duplikate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DupKind {
    /// Exakte Duplikate.
    Exact,
    /// Nahezu-Duplikate.
    Near,
}

/// Argumente von `detail.duplicates`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesArgs {
    /// URL.
    pub url_id: i64,
    /// Art.
    pub kind: DupKind,
}

/// Ein Duplikat-Partner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct DupPartner {
    /// URL.
    pub url_id: i64,
    /// Adresse.
    pub address: String,
    /// Ähnlichkeit in Prozent (nur Nahezu-Duplikate).
    pub similarity: Option<f64>,
}

/// Antwort von `detail.duplicates` (UI-044).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct DuplicatesResult {
    /// Partner.
    pub partners: Vec<DupPartner>,
    /// Text des Inhaltsbereichs der Fokus-URL.
    pub text: Option<String>,
}

/// Argumente von `detail.lighthouse` (P2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LighthouseArgs {
    /// URL.
    pub url_id: i64,
    /// Gerät.
    pub device: SerpDevice,
}

/// Antwort von `detail.lighthouse` (UI-046, P2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct LighthouseDto {
    /// Leistungsindex 0 – 100.
    pub performance_score: Option<f64>,
    /// Metriken.
    pub metrics: Vec<NameValue>,
    /// Möglichkeiten (Einsparungen).
    pub opportunities: Vec<NameValue>,
    /// Diagnosen.
    pub diagnostics: Vec<NameValue>,
}

/// Argumente von `detail.spellingContext`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpellingContextArgs {
    /// URL.
    pub url_id: i64,
    /// Befund.
    pub finding_id: i64,
}

/// Antwort von `detail.spellingContext` (ONPAGE-153, UI-047).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpellingContext {
    /// Text um den Befund.
    pub text: String,
    /// Beginn des Befunds im Text (Zeichen).
    pub start: u32,
    /// Ende des Befunds im Text (Zeichen).
    pub end: u32,
    /// Vorschläge.
    pub suggestions: Vec<String>,
}

/// Argumente von `detail.a11yNode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct A11yNodeArgs {
    /// URL.
    pub url_id: i64,
    /// Knoten.
    pub node_id: i64,
}

/// Antwort von `detail.a11yNode` (TECH-259, UI-048).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct A11yNodeDetail {
    /// Regel.
    pub rule_id: String,
    /// Schweregrad.
    pub impact: Option<String>,
    /// CSS-Selektor.
    pub target: String,
    /// HTML-Auszug.
    pub snippet: String,
    /// i18n-Schlüssel der Kurzbeschreibung.
    pub summary_key: String,
}
