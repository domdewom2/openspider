//! Absturzberichte (M3-C01, OUT-246/OUT-247; K2-61 additiv): Übersicht, Ansehen, Senden, Verwerfen, Löschen.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Einstellung `crash_reports` in `settings.toml` (OUT-247).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashReportSettingsDto {
    /// Nach einem Absturz beim Start fragen, ob ein Bericht gesendet werden soll.
    pub prompt: bool,
}

/// Eintrag der Liste Hilfe → „Absturzberichte…“.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashReportSummary {
    /// Kennung (`id` des Berichts, UUID v4).
    pub id: String,
    /// `panic`, `frontend` oder `unclean_exit`.
    pub kind: String,
    /// Zeitpunkt (ISO 8601 mit Offset).
    pub time: String,
    /// `app` oder `cli`.
    pub process: String,
    /// Bereinigte erste Zeile der Meldung.
    pub message: String,
    /// Zeitpunkt des Sendens oder `null`.
    pub sent: Option<String>,
    /// Referenz des Servers oder `null`.
    pub server_ref: Option<String>,
}

/// Antwort von `crash.overview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashOverview {
    /// Alle gespeicherten Berichte, neueste zuerst.
    pub reports: Vec<CrashReportSummary>,
    /// Kennungen für den Hinweis beim Start: ungesendete, nicht verworfene Berichte früherer Läufe (neueste zuerst).
    /// Leer, wenn die Einstellung aus ist oder eine Richtlinie Absturzberichte deaktiviert.
    pub pending: Vec<String>,
    /// Einstellung `crash_reports.prompt`.
    pub prompt_enabled: bool,
    /// Richtlinie `DisableCrashReports` gesetzt: kein Hinweis, „Senden“ ausgegraut.
    pub disabled_by_policy: bool,
}

/// Argument mit einer Berichtskennung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashIdArgs {
    pub id: String,
}

/// Argument mit mehreren Berichtskennungen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashIdsArgs {
    pub ids: Vec<String>,
}

/// Argument von `crash.send`: die Berichte (bei mehreren die neuesten 5) und die optionalen Felder aus „Bericht
/// ansehen…“, die nur gesendet und nie gespeichert werden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashSendArgs {
    pub ids: Vec<String>,
    /// „Was haben Sie zuletzt gemacht?“, höchstens 2 000 Zeichen.
    #[serde(default)]
    pub note: String,
    /// „E-Mail für Rückfragen“, höchstens 254 Zeichen, gültig oder leer.
    #[serde(default)]
    pub email: String,
}

/// Ursache eines fehlgeschlagenen Sendens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CrashSendFailure {
    /// Keine Antwort innerhalb von 15 s.
    Timeout,
    /// Keine Verbindung (DNS, TCP, TLS, Proxy).
    Network,
    /// Antwort mit diesem HTTP-Status (`400`, `413`, `429`, `500` …).
    Http(u16),
    /// Antwort `200`, aber ohne gültige Referenz.
    Response,
    /// Durch Richtlinie deaktiviert; es wurde nichts gesendet.
    Disabled,
    /// Bericht nicht gefunden oder nicht lesbar.
    NotFound,
    /// Bericht wurde bereits gesendet; es geht keine zweite Anfrage hinaus.
    AlreadySent,
}

/// Ergebnis je Bericht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashSendOutcome {
    pub id: String,
    /// Referenz des Servers bei Erfolg.
    pub server_ref: Option<String>,
    /// Ursache bei einem Fehler.
    pub failure: Option<CrashSendFailure>,
}

/// Antwort von `crash.send`; ein Fehler bricht die Reihe ab, die übrigen Berichte bleiben ungesendet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashSendResult {
    pub results: Vec<CrashSendOutcome>,
}

/// Argument von `crash.reportFrontend`: Fehler, der eine Fehlergrenze der Oberfläche ausgelöst hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrashFrontendArgs {
    /// Fehlermeldung.
    pub message: String,
    /// Komponenten-Stack von React.
    #[serde(default)]
    pub component_stack: String,
    /// Region der Fehlergrenze (`table`, `detail`, … oder `root` für die ganze Ansicht).
    #[serde(default)]
    pub region: String,
}
