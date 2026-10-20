//! Aufgabenplanung (`tasks.*`, OUT-160 … OUT-170, `docs/plan/M2.md` 3.6a).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::api::ApiKind;
use super::export::ExportFormatDto;

/// Zeitplan einer Aufgabe (OUT-161).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScheduleDef {
    /// Einmalig.
    #[serde(rename_all = "camelCase")]
    Once {
        /// Zeitpunkt (ISO 8601, lokale Zeit mit Offset).
        at: String,
    },
    /// Täglich.
    #[serde(rename_all = "camelCase")]
    Daily {
        /// Uhrzeit `HH:MM`.
        time: String,
    },
    /// Wöchentlich.
    #[serde(rename_all = "camelCase")]
    Weekly {
        /// Wochentage 1 (Montag) bis 7 (Sonntag).
        days: Vec<u8>,
        /// Uhrzeit `HH:MM`.
        time: String,
    },
    /// Monatlich.
    #[serde(rename_all = "camelCase")]
    Monthly {
        /// Tag des Monats 1 – 31.
        day: u8,
        /// Uhrzeit `HH:MM`.
        time: String,
    },
}

/// Quelle der Aufgabe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TaskSource {
    /// Spider ab Start-URL.
    #[serde(rename_all = "camelCase")]
    Spider {
        /// Start-URL.
        url: String,
    },
    /// Liste aus Datei.
    #[serde(rename_all = "camelCase")]
    List {
        /// Pfad.
        path: String,
    },
}

/// Crawl-Einstellungen der Aufgabe (OUT-162).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskCrawl {
    /// Quelle.
    pub source: TaskSource,
    /// Konfigurationsprofil; `null` = Standard.
    pub profile: Option<String>,
    /// Separates Auth-Profil (`auth.listProfiles`); `null` = keine Anmeldung.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile: Option<String>,
    /// Überschreibungen einzelner Schlüssel (`--set`-Form: Pfad → Wert).
    pub overrides: Option<serde_json::Value>,
}

/// Ausführung (OUT-163).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskExecution {
    /// Ohne Oberfläche ausführen (CLI).
    pub headless: bool,
    /// Höchstlaufzeit in Minuten.
    pub timeout_min: Option<u32>,
    /// Verpassten Lauf nachholen.
    pub run_if_missed: bool,
    /// Mit dem vorherigen Crawl der Aufgabe vergleichen (OUT-152).
    pub auto_compare: bool,
}

/// Art eines Exports der Aufgabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TaskExportKind {
    /// Tab-Ansicht.
    Tab,
    /// Bulk-Export.
    Bulk,
    /// Bericht.
    Report,
    /// Sitemap.
    Sitemap,
}

/// Ein Export der Aufgabe (OUT-164).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskExport {
    /// Art.
    pub kind: TaskExportKind,
    /// Name aus dem Katalog bzw. `Tab:Filter`.
    pub name: String,
    /// Format.
    pub format: ExportFormatDto,
    /// Zielordner; `null` = Exportordner.
    pub dir: Option<String>,
}

/// Benachrichtigung (OUT-165, OUT-280).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskNotify {
    /// Bei Erfolg senden.
    pub on_success: bool,
    /// Bei Fehler senden.
    pub on_failure: bool,
    /// Mail-Konto (`mail.listAccounts`).
    pub account: Option<String>,
    /// Empfänger.
    pub to: Vec<String>,
}

/// Definition einer Aufgabe (`tasks.get`, `tasks.save`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskDef {
    /// Kennung; `null` bei neuen Aufgaben.
    pub id: Option<String>,
    /// Name.
    pub name: String,
    /// Projekt.
    pub project: Option<String>,
    /// Eingeschaltet.
    pub enabled: bool,
    /// Zeitplan.
    pub schedule: ScheduleDef,
    /// Crawl-Einstellungen.
    pub crawl: TaskCrawl,
    /// Ausführung.
    pub execution: TaskExecution,
    /// API-Quellen, die abgefragt werden.
    pub apis: Vec<ApiKind>,
    /// Exporte.
    pub exports: Vec<TaskExport>,
    /// Benachrichtigung.
    pub notify: TaskNotify,
    /// Exportoptionen der geplanten Aufgabe.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub export_options: Option<super::export::ExportMultiArgs>,

    /// Name der für die Aufgabe verwendeten Exportvorlage.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub export_preset: Option<String>,

    /// Konfiguration der benutzerdefinierten Zusammenfassung.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub custom_summary: Option<super::export::CustomSummaryArgs>,
}

/// Zustand eines Laufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TaskRunState {
    /// Läuft.
    Running,
    /// Erfolgreich.
    Succeeded,
    /// Fehlgeschlagen.
    Failed,
    /// Abgebrochen.
    Cancelled,
    /// Übersprungen (z. B. Projekt gesperrt).
    Skipped,
}

/// Ein Lauf einer Aufgabe (OUT-169).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskRunRow {
    /// Kennung des Laufs.
    pub run_id: String,
    /// Aufgabe.
    pub task_id: String,
    /// Beginn (ISO 8601 UTC).
    pub started_at: String,
    /// Ende.
    pub ended_at: Option<String>,
    /// Zustand.
    pub state: TaskRunState,
    /// Exit-Code des Prozesses.
    pub exit_code: Option<i32>,
    /// Entstandener Crawl.
    pub crawl_id: Option<String>,
    /// Geschriebene Exportdateien.
    pub exports: Vec<String>,
    /// Fehler (redigiert).
    pub error: Option<String>,
    /// Abbruchzuordnung des Kerns, auch für vom OS-Planer gestartete Läufe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<super::common::OpHandle>,
    /// Anzahl gecrawlter URLs; unbekannt bei älteren Historieneinträgen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urls_crawled: Option<u64>,
    /// Tatsächlicher Ausgabeordner dieses Laufs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_folder: Option<String>,
    /// Lauf-Logdatei; `tasks.openLog` löst die Laufkennung im Kern auf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

/// Eintrag von `tasks.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TaskInfo {
    /// Kennung.
    pub id: String,
    /// Name.
    pub name: String,
    /// Projekt.
    pub project: Option<String>,
    /// Zeitplan.
    pub schedule: ScheduleDef,
    /// Nächster Lauf (ISO 8601 UTC).
    pub next_run: Option<String>,
    /// Letzter Lauf.
    pub last_run: Option<TaskRunRow>,
    /// Eingeschaltet.
    pub enabled: bool,
    /// Beim Betriebssystem-Planer eingetragen.
    pub registered: bool,
}

/// Argument von `tasks.validate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskArgs {
    /// Aufgabe.
    pub task: TaskDef,
}

/// Argument von `tasks.history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct TasksHistoryArgs {
    /// Nur diese Aufgabe; `null` = alle.
    pub id: Option<String>,
    /// Offset.
    pub offset: u64,
    /// Anzahl.
    pub limit: u32,
}

/// Argument von `tasks.register`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TaskRegisterArgs {
    /// Aufgabe.
    pub id: String,
    /// Nur anzeigen, was geschrieben würde.
    pub dry_run: bool,
}

/// Backend des Betriebssystem-Planers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PlannerBackend {
    /// launchd (macOS).
    Launchd,
    /// Windows-Aufgabenplanung.
    TaskScheduler,
}

/// Eine vom Planer-Backend geschriebene Datei.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlannerFile {
    /// Pfad.
    pub path: String,
    /// Inhalt.
    pub content: String,
}

/// Ergebnis von `tasks.register` (OUT-166).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlannerPreview {
    /// Backend.
    pub backend: PlannerBackend,
    /// Dateien.
    pub files: Vec<PlannerFile>,
    /// Aufruf, der registriert.
    pub command: Vec<String>,
}

/// Argument von `tasks.export` (OUT-170).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TasksExportArgs {
    /// Aufgaben.
    pub ids: Vec<String>,
    /// Zieldatei.
    pub path: String,
}
