//! Exporte, Berichte und Sitemaps (`export.*`, OUT-020 ff., OUT-030 ff., OUT-120 ff.).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::Selection;
use super::view::ViewSpec;

/// Ausgabeformat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormatDto {
    /// CSV.
    Csv,
    /// Excel.
    Xlsx,
    /// Vertragserweiterung M2-F05b.
    Json,
}

/// Ein Eintrag des Export-Katalogs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    /// Kanonischer Name (Dateiname und Aufruf).
    pub name: String,
    /// i18n-Schlüssel des Anzeigenamens.
    pub label_key: String,
    /// Vertragserweiterung M2-F05b: enabled.
    pub enabled: bool,

    /// Vertragserweiterung M2-F05b: disabled_reason.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub disabled_reason: Option<String>,
}

/// Gruppe von Exporten im Menü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportGroup {
    /// Gruppenname.
    pub key: String,
    /// i18n-Schlüssel der Gruppe.
    pub label_key: String,
    /// Einträge.
    pub items: Vec<ExportItem>,
}

/// Antwort von `export.catalog` (UI-002, OUT-030 ff.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportCatalog {
    /// Bulk-Exporte.
    pub bulk: Vec<ExportGroup>,
    /// Berichte.
    pub reports: Vec<ExportGroup>,
    /// Sitemap-Arten.
    pub sitemaps: Vec<ExportItem>,
}

/// Ziel eines Exports (für `export.suggestName`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportTarget {
    /// Ansicht eines Tabs.
    #[serde(rename_all = "camelCase")]
    View {
        /// Tab.
        tab: String,
        /// Filter.
        filter: String,
        /// Format.
        format: ExportFormatDto,
    },
    /// Detailtabelle.
    #[serde(rename_all = "camelCase")]
    Detail {
        /// Beschreibung.
        spec: ViewSpec,
        /// Format.
        format: ExportFormatDto,
    },
    /// Bulk-Export.
    #[serde(rename_all = "camelCase")]
    Bulk {
        /// Name.
        name: String,
        /// Format.
        format: ExportFormatDto,
    },
    /// Bericht.
    #[serde(rename_all = "camelCase")]
    Report {
        /// Name.
        name: String,
        /// Format.
        format: ExportFormatDto,
    },
    /// Sitemap.
    #[serde(rename_all = "camelCase")]
    Sitemap {
        /// `pages` oder `images`.
        sitemap_kind: String,
    },
}

/// Argument von `export.suggestName`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SuggestNameArgs {
    /// Ziel.
    pub target: ExportTarget,
}

/// Argument von `export.view` (OUT-020).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportViewArgs {
    /// View.
    pub view_id: String,
    /// Version, die exportiert wird (Schnappschuss).
    pub version: u64,
    /// Zieldatei.
    pub path: String,
    /// Format.
    pub format: ExportFormatDto,
    /// Auch ausgeblendete Spalten exportieren.
    pub all_columns: bool,
    /// Vertragserweiterung M2-F05b: columns.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub columns: Option<Vec<String>>,

    /// Vertragserweiterung M2-F05b: preserve_upload_order.
    #[serde(default = "default_upload_order")]
    pub preserve_upload_order: bool,
}

/// Argument von `export.detail` (OUT-021).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportDetailArgs {
    /// Beschreibung der Detailtabelle.
    pub spec: ViewSpec,
    /// Auswahl der Quell-URLs.
    pub selection: Selection,
    /// Zieldatei.
    pub path: String,
    /// Format.
    pub format: ExportFormatDto,
}

/// Argument von `export.bulk` und `export.report` (OUT-030 ff., OUT-080 ff.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportBulkArgs {
    /// Name aus dem Katalog.
    pub name: String,
    /// Zielordner.
    pub dir: String,
    /// Format.
    pub format: ExportFormatDto,
    /// In einer Arbeitsmappe konsolidieren (OUT-007).
    pub single_workbook: bool,
}

/// Argument von `export.sitemap`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportSitemapArgs {
    /// `pages` oder `images`.
    pub sitemap_kind: String,
    /// Optionen: Inhalt von `os_model::SitemapGenConfig` als JSON (OUT-121 ff.); `null` = die der Konfiguration.
    #[ts(optional = nullable)]
    pub options: Option<serde_json::Value>,
    /// Zielordner.
    pub dir: String,
}

/// Seitenleisten-Inhalt für `export.sidebar`/`export.issues` (UI-060, UI-062).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PanelKind {
    /// Overview.
    Overview,
    /// Issues.
    Issues,
    /// Site Structure.
    SiteStructure,
    /// Response Times.
    ResponseTimes,
    /// Segmente.
    Segments,
    /// API-Status.
    Api,
    /// Vertragserweiterung M2-F05b.
    SpellingGrammar,
}

/// Argument von `export.issues` und `export.sidebar`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportPanelArgs {
    /// Welches Panel.
    pub which: PanelKind,
    /// Zieldatei.
    pub path: String,
    /// Format.
    pub format: ExportFormatDto,
}

fn default_upload_order() -> bool {
    true
}

/// Command-Vertrag ExportSel (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportSel {
    /// Vertragserweiterung M2-F05b: kind.
    pub kind: String,

    /// Vertragserweiterung M2-F05b: name.
    pub name: String,
}

/// Command-Vertrag ExportMultiArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportMultiArgs {
    /// Vertragserweiterung M2-F05b: items.
    pub items: Vec<ExportSel>,

    /// Vertragserweiterung M2-F05b: format.
    pub format: ExportFormatDto,

    /// Vertragserweiterung M2-F05b: dir.
    pub dir: String,

    /// Vertragserweiterung M2-F05b: single_workbook.
    pub single_workbook: bool,

    /// Vertragserweiterung M2-F05b: skip_empty.
    pub skip_empty: bool,

    /// Vertragserweiterung M2-F05b: preset.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub preset: Option<String>,
}

/// Command-Vertrag ExportBatchResult (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportBatchResult {
    /// Vertragserweiterung M2-F05b: files.
    pub files: Vec<super::common::FileResult>,

    /// Vertragserweiterung M2-F05b: warnings.
    pub warnings: Vec<String>,

    /// Vertragserweiterung M2-F05b: skipped.
    pub skipped: Vec<String>,
}

/// Command-Vertrag ExportPreset (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreset {
    /// Vertragserweiterung M2-F05b: name.
    pub name: String,

    /// Vertragserweiterung M2-F05b: items.
    pub items: Vec<ExportSel>,

    /// Vertragserweiterung M2-F05b: single_workbook.
    pub single_workbook: bool,

    /// Vertragserweiterung M2-F05b: skip_empty.
    pub skip_empty: bool,
}

/// Command-Vertrag ExportPresetArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportPresetArgs {
    /// Vertragserweiterung M2-F05b: name.
    pub name: String,

    /// Vertragserweiterung M2-F05b: preset.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub preset: Option<ExportPreset>,
}

/// Command-Vertrag CustomSummaryArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomSummaryArgs {
    /// Vertragserweiterung M2-F05b: account.
    pub account: String,

    /// Vertragserweiterung M2-F05b: target.
    pub target: String,

    /// Vertragserweiterung M2-F05b: metrics.
    pub metrics: Vec<String>,
}

/// Command-Vertrag CustomSummaryMetric (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomSummaryMetric {
    /// Vertragserweiterung M2-F05b: id.
    pub id: String,

    /// Vertragserweiterung M2-F05b: label_key.
    pub label_key: String,
}
