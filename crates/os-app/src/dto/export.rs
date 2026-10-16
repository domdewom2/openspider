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
}

/// Ein Eintrag des Export-Katalogs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportItem {
    /// Kanonischer Name (Dateiname und Aufruf).
    pub name: String,
    /// i18n-Schlüssel des Anzeigenamens.
    pub label_key: String,
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
