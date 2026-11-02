//! Chromium-Bereitstellung (M3-K01, K3-01): Status, Download, Abbruch, Entfernen.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Herkunft des Chromium, das ein Crawl im JavaScript-Modus verwenden würde (Auswahlreihenfolge K3-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChromiumSource {
    /// Pfad aus `rendering.chromium_path`.
    Configured,
    /// Pfad aus `OPENSPIDER_CHROMIUM`.
    Environment,
    /// Von OpenSpider bereitgestellt.
    Provisioned,
    /// Installiertes Chrome/Chromium.
    System,
    /// Kein Chromium gefunden (bzw. vorgegebener Pfad fehlt).
    None,
}

/// Fehler des letzten Downloads (`messageKey` = `err-chromium-…`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChromiumFailure {
    pub message_key: String,
    pub detail: String,
}

/// Zustand des Downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChromiumJob {
    Idle,
    Downloading { done: u64, total: u64 },
    Verifying,
    Installing,
    Failed(ChromiumFailure),
}

/// Antwort aller `render_provision.*`-Commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ChromiumStatus {
    /// Für diese Plattform gibt es einen Download (macOS arm64/x64, Windows x64 inkl. ARM per Emulation).
    pub supported: bool,
    /// Feste Version laut Versionsdatei.
    pub version: String,
    /// Größe des Downloads in Byte.
    pub download_bytes: Option<u64>,
    /// Bereitgestellte Version, falls installiert.
    pub installed: Option<String>,
    /// Programmpfad der bereitgestellten Version.
    pub installed_path: Option<String>,
    /// Chromium, das ein Crawl verwenden würde.
    pub active: ChromiumSource,
    /// Pfad dazu bzw. der fehlende vorgegebene Pfad.
    pub active_path: Option<String>,
    /// Bereits geladene Bytes eines abgebrochenen Downloads (fortsetzbar).
    pub partial_bytes: u64,
    pub job: ChromiumJob,
}

/// Argumente: aktueller Wert von `rendering.chromium_path` (Entwurf der Konfiguration) für die Auswahl.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ChromiumStatusArgs {
    #[serde(default)]
    pub configured_path: Option<String>,
}
