//! API-Quellen (`api.*`, `sidebar.apiStatus`, 04 A – J).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::Selection;
use super::error::CmdError;

/// Datenquellen der Integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApiKind {
    /// Google Analytics 4.
    Ga4,
    /// Google Search Console.
    Gsc,
    /// PageSpeed Insights.
    Psi,
    /// Majestic.
    Majestic,
    /// Ahrefs.
    Ahrefs,
    /// Moz.
    Moz,
    /// Google Drive (Sheets-Export).
    Drive,
}

/// Art der Anmeldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApiAuthKind {
    /// OAuth im Systembrowser mit PKCE und Loopback.
    Oauth,
    /// API-Schlüssel.
    ApiKey,
    /// Keine Anmeldung.
    None,
}

/// Kontingent einer Quelle (INTEG-006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct QuotaDto {
    /// Verbraucht im Zeitraum.
    pub used: u64,
    /// Limit.
    pub limit: Option<u64>,
    /// Zurücksetzung (ISO 8601 UTC).
    pub resets_at: Option<String>,
    /// Vertragserweiterung M2-F05b: dimension.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub dimension: Option<String>,
}

/// Eintrag von `api.list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiSourceInfo {
    /// Quelle.
    pub kind: ApiKind,
    /// i18n-Schlüssel des Namens.
    pub label_key: String,
    /// Art der Anmeldung.
    pub auth: ApiAuthKind,
    /// Verbunden.
    pub connected: bool,
    /// Konto (E-Mail bzw. Kennung).
    pub account: Option<String>,
    /// Kontingent.
    pub quota: Option<QuotaDto>,
    /// Beim Start automatisch verbinden (INTEG-011).
    pub auto_connect: bool,
    /// Vertragserweiterung M2-F05b: state.
    pub state: ApiConnState,

    /// Vertragserweiterung M2-F05b: error.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub error: Option<CmdError>,

    /// Vertragserweiterung M2-F05b: accounts.
    pub accounts: Vec<ApiAccount>,
}

/// Argument von `api.connect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiConnectArgs {
    /// Quelle.
    pub kind: ApiKind,
    /// API-Schlüssel (nur bei Quellen mit Schlüssel; Ablage im Schlüsselbund).
    pub api_key: Option<String>,
    /// Vertragserweiterung M2-F05b: remember.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub remember: Option<bool>,

    /// Vertragserweiterung M2-F05b: account_id.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub account_id: Option<String>,

    /// Vertragserweiterung M2-F05b: client_secret.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub client_secret: Option<String>,
}

/// Ergebnis von `api.connect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiAccount {
    /// Quelle.
    pub kind: ApiKind,
    /// Konto.
    pub account: String,
    /// Gewährte Berechtigungen.
    pub scopes: Vec<String>,
    /// Ablauf des Tokens (ISO 8601 UTC).
    pub expires_at: Option<String>,
    /// Vertragserweiterung M2-F05b: account_id.
    pub account_id: String,

    /// Vertragserweiterung M2-F05b: key_suffix.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub key_suffix: Option<String>,
}

/// Argument mit einer Quelle (`api.disconnect`, `api.testConnection`, `api.getSettings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiKindArgs {
    /// Quelle.
    pub kind: ApiKind,
}

/// Ergebnis von `api.testConnection` (Zeitlimit 10 s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiTestResult {
    /// Verbindung funktioniert.
    pub ok: bool,
    /// Konto.
    pub account: Option<String>,
    /// Verbleibendes Kontingent.
    pub quota_remaining: Option<u64>,
    /// Fehler.
    pub error: Option<CmdError>,
}

/// Einstellungen einer Quelle: Inhalt des Abschnitts `api.<quelle>` der Konfiguration (Metriken, Zeitraum,
/// Dimensionen, Filter, Basis-URL); nie Geheimnisse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiSourceSettings {
    /// Quelle.
    pub kind: ApiKind,
    /// Einstellungen als JSON (Schlüssel wie in `api.<quelle>` der `.osconfig`).
    pub values: serde_json::Value,
}

/// Argument von `api.setSettings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiSetSettingsArgs {
    /// Quelle.
    pub kind: ApiKind,
    /// Neue Einstellungen.
    pub settings: ApiSourceSettings,
    /// Vertragserweiterung M2-F05b: decision.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub decision: Option<ApiSettingsDecision>,
}

/// Ergebnis von `api.setSettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiSettingsResult {
    /// Gespeicherte Daten dieser Quelle wurden ungültig (INTEG-010).
    pub invalidated: bool,
    /// Vertragserweiterung M2-F05b: confirmation_required.
    pub confirmation_required: bool,

    /// Vertragserweiterung M2-F05b: data_stale.
    pub data_stale: bool,
}

/// Umfang von `api.fetch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApiFetchScope {
    /// Alle URLs.
    All,
    /// Nur URLs ohne Daten.
    Missing,
    /// Auswahl.
    #[serde(rename_all = "camelCase")]
    Selection {
        /// URLs.
        selection: Selection,
    },
}

/// Argument von `api.fetch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiFetchArgs {
    /// Quellen; leer = alle verbundenen.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub kinds: Option<Vec<ApiKind>>,
    /// Umfang.
    pub scope: ApiFetchScope,
}

/// Ergebnis von `api.fetch` (Ergebnis der Operation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiFetchSummary {
    /// Angefordert.
    pub requested: u64,
    /// Abgerufen.
    pub fetched: u64,
    /// Fehlgeschlagen.
    pub failed: u64,
    /// Verbleibendes Kontingent.
    pub quota_left: Option<u64>,
}

/// Argument von `api.errors`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorsArgs {
    /// Quelle.
    pub kind: ApiKind,
    /// Offset.
    pub offset: u64,
    /// Anzahl.
    pub limit: u32,
}

/// Ein Fehler beim API-Abruf (INTEG-005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiErrorRow {
    /// Zeitpunkt (ISO 8601 UTC).
    pub at: String,
    /// URL.
    pub url: Option<String>,
    /// HTTP-Status der Quelle.
    pub status: Option<u16>,
    /// Meldung (redigiert).
    pub message: String,
}

/// Zeile der Seitenleisten-Ansicht „APIs“ (UI-070, INTEG-007); auch Antwort von `api.status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ApiRow {
    /// Quelle.
    pub kind: ApiKind,
    /// i18n-Schlüssel des Namens.
    pub label_key: String,
    /// Verbunden.
    pub connected: bool,
    /// Angefordert.
    pub requested: u64,
    /// Empfangen.
    pub received: u64,
    /// Fehler.
    pub errors: u64,
    /// Verbleibendes Kontingent.
    pub quota_left: Option<u64>,
    /// Vertragserweiterung M2-F05b: running.
    pub running: bool,

    /// Vertragserweiterung M2-F05b: completed.
    pub completed: u64,

    /// Vertragserweiterung M2-F05b: reset_at.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reset_at: Option<String>,
}

/// Command-Vertrag ApiAccountArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApiAccountArgs {
    /// Vertragserweiterung M2-F05b: kind.
    pub kind: ApiKind,

    /// Vertragserweiterung M2-F05b: account_id.
    pub account_id: String,
}

/// Vertrag M2-F05b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApiConnState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Vertrag M2-F05b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApiSettingsDecision {
    Ask,
    Keep,
    Reload,
}
