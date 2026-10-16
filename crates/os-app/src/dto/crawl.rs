//! Crawl-Steuerung und Analyse (`crawl.*`, `analysis.*`, `M2-ui-concept.md` 4.4).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::events::{CrawlModeDto, CrawlOwner, CrawlPhase};

/// Argument von `crawl.validateStartUrl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ValidateStartUrlArgs {
    /// Eingabe des Nutzers.
    pub input: String,
}

/// Antwort von `crawl.validateStartUrl` (Fehler als `CmdError` mit `field`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedUrl {
    /// Normalisierte Start-URL.
    pub normalized: String,
}

/// Quelle der URL-Liste (`crawl.parseList`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ListSource {
    /// Eingefügter Text.
    #[serde(rename_all = "camelCase")]
    Text {
        /// Inhalt, eine URL je Zeile.
        text: String,
    },
    /// Datei (CSV, TXT, XLSX, Sitemap).
    #[serde(rename_all = "camelCase")]
    Path {
        /// Pfad.
        path: String,
    },
}

/// Vorschau einer URL-Liste (CRAWL-027).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListPreview {
    /// Gültige URLs.
    pub valid: u64,
    /// Ungültige Einträge.
    pub invalid: u64,
    /// Doppelte Einträge.
    pub duplicates: u64,
    /// Beispiele gültiger URLs.
    pub sample: Vec<String>,
}

/// Quelle eines Crawls (`crawl.start`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StartSource {
    /// Spider ab Start-URL.
    #[serde(rename_all = "camelCase")]
    Spider {
        /// Start-URL.
        url: String,
    },
    /// Liste von URLs.
    #[serde(rename_all = "camelCase")]
    List {
        /// URLs.
        items: Vec<String>,
        /// Bezeichnung (Dateiname).
        label: String,
    },
    /// APIs-Modus: Datenabruf ohne Crawl (INTEG-012).
    #[serde(rename_all = "camelCase")]
    Apis {
        /// URLs bzw. Quellen.
        items: Vec<String>,
    },
}

/// Bereich des Crawls (Subdomains, Ordner).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StartScope {
    /// Nur die Start-Domain.
    Domain,
    /// Alle Subdomains.
    Subdomains,
    /// Nur der Startordner.
    Folder,
}

/// Argument von `crawl.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct StartRequest {
    /// Quelle.
    pub source: StartSource,
    /// Bereich.
    pub scope: StartScope,
    /// Konfigurationsprofil; `null` = Standard.
    pub profile: Option<String>,
}

/// Antwort von `crawl.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrawlRef {
    /// Kennung des Crawls.
    pub crawl_id: String,
    /// Crawl-Datei.
    pub file: String,
}

/// Argument von `crawl.open`: Kennung oder Pfad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenCrawlArgs {
    /// Kennung aus der Verwaltung.
    #[serde(rename_all = "camelCase")]
    Id {
        /// Crawl.
        crawl_id: String,
    },
    /// Datei (Doppelklick, Datei → Öffnen).
    #[serde(rename_all = "camelCase")]
    Path {
        /// Pfad.
        path: String,
    },
}

/// Zusammenfassung eines Crawls (Antwort von `crawl.open`, Zeile der Verwaltung).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CrawlSummaryDto {
    /// Kennung.
    pub crawl_id: String,
    /// Datei.
    pub file: String,
    /// Name (Start-URL oder Listenname).
    pub name: String,
    /// Zustand.
    pub phase: CrawlPhase,
    /// Modus.
    pub mode: CrawlModeDto,
    /// Zeitpunkt des Starts (ISO 8601 UTC).
    pub started_at: String,
    /// Zeitpunkt des Endes.
    pub finished_at: Option<String>,
    /// Anzahl URLs.
    pub urls: u64,
    /// Größe der Datei in Byte.
    pub file_bytes: u64,
    /// Schemaversion der Datei.
    pub schema_version: u32,
    /// Besitzer; fremde laufende Crawls öffnet die App nur lesend live (F-14).
    pub owner: CrawlOwner,
}

/// Argument von `crawl.saveAs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveAsArgs {
    /// Zieldatei.
    pub dest: String,
}

/// Argument von `crawl.addUrls` (CRAWL-131).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddUrlsArgs {
    /// Hinzuzufügende URLs.
    pub urls: Vec<String>,
}

/// Ergebnis von `crawl.addUrls`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddUrlsResult {
    /// Neu eingeplant.
    pub added: u64,
    /// Bereits bekannt oder ungültig.
    pub skipped: u64,
}

/// Ein Schritt des Crawl-Pfads (UI-050, OUT-023).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CrawlPathDto {
    /// Tiefe (0 = Start-URL).
    pub depth: u32,
    /// URL-Kennung.
    pub url_id: i64,
    /// Adresse.
    pub address: String,
    /// Statuscode.
    pub status_code: Option<u16>,
    /// Linkart, über die die URL entdeckt wurde.
    pub via: Option<String>,
}

/// Argument von `analysis.start`: alle Schritte oder eine Auswahl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct AnalysisStartArgs {
    /// Schritte (`link_score`, …); `null` = alle eingeschalteten und veralteten.
    pub steps: Option<Vec<String>>,
}
