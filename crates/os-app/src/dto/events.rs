//! Ereignisse und Statusanzeigen (`Tick`, `UiEvent`, `M2-ui-concept.md` 4.3, UI-013, UI-018, UI-143).
//!
//! Ein Ereigniskanal je Fenster; alle Ereignisarten zusammen höchstens 10 Nachrichten je Sekunde (≤ 1 Hz bei
//! verdecktem Fenster). Der Tick trägt nur Versionen und Kennzahlen; Inhalte holt die Oberfläche über Commands.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::OpStatus;

/// Zustand des Crawls in der Statusleiste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CrawlPhase {
    /// Bereit (kein Crawl läuft).
    Idle,
    /// Läuft.
    Running,
    /// Pausiert.
    Paused,
    /// Fertig.
    Completed,
    /// Unterbrochen.
    Incomplete,
    /// Fehler.
    Failed,
}

/// Modus des Crawls (UI-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CrawlModeDto {
    /// Spider.
    Spider,
    /// Liste.
    List,
    /// SERP.
    Serp,
    /// APIs.
    Apis,
    /// Vergleich.
    Compare,
}

/// Speicherstufe (CRAWL-129).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MemoryStageDto {
    /// Unter 80 % der Obergrenze.
    Normal,
    /// Ab 80 %: Hinweis.
    Warning,
    /// Ab 90 %: automatisch pausiert.
    Paused,
}

/// Wem der Crawl gehört (F-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OwnerKind {
    /// Diese App.
    App,
    /// CLI.
    Cli,
    /// MCP-Server (STDIO).
    Mcp,
    /// Geplante Aufgabe.
    Scheduler,
}

/// Besitzer des Crawls: fremde Crawls öffnet die App nur lesend live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CrawlOwner {
    /// Art.
    pub kind: OwnerKind,
    /// Prozess-ID.
    pub pid: Option<u32>,
    /// Heartbeat frisch.
    pub live: bool,
    /// Die App darf den Crawl steuern (Pause, Stopp, Fortsetzen).
    pub controllable: bool,
}

/// Fortschritt des Crawls (CRAWL-130, UI-013).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CrawlStatus {
    /// Crawl.
    pub crawl_id: String,
    /// Zustand.
    pub phase: CrawlPhase,
    /// Modus.
    pub mode: CrawlModeDto,
    /// Grund des Endes (`queue_empty`, `limit_reached`, `stopped`).
    pub end_reason: Option<String>,
    /// Abgeschlossene Abrufe.
    pub crawled: u64,
    /// Eingeplante URLs.
    pub queued: u64,
    /// Entdeckte URLs gesamt.
    pub discovered: u64,
    /// Fortschritt 0 – 100.
    pub percent: u8,
    /// Aktuelle Rate (URL/s).
    pub rate: f64,
    /// Durchschnittsrate ohne Pausen (URL/s).
    pub avg_rate: f64,
    /// Mittlere Antwortzeit in Sekunden.
    pub avg_response_time: Option<f64>,
    /// Verstrichene Zeit in Sekunden.
    pub elapsed_s: u64,
    /// Pausenzeit in Sekunden.
    pub paused_s: u64,
    /// Geschätzte Restzeit in Sekunden.
    pub eta_s: Option<u64>,
    /// Aktive Threads.
    pub active_threads: u32,
    /// Speicherstufe.
    pub memory_stage: MemoryStageDto,
    /// Größe der Crawl-Datei in Byte.
    pub file_bytes: Option<u64>,
    /// Besitzer.
    pub owner: CrawlOwner,
}

/// Zustand eines Analyse-Teilschritts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StepStateDto {
    /// Ausstehend.
    Pending,
    /// Läuft.
    Running,
    /// Fertig.
    Done,
    /// Übersprungen.
    Skipped,
    /// Nicht anwendbar.
    NotApplicable,
}

/// Ein Teilschritt der Crawl-Analyse (UI-082).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStepStatus {
    /// Schritt (`link_score`, `canonicals`, …).
    pub step: String,
    /// Zustand.
    pub state: StepStateDto,
    /// Ergebnis ist veraltet (Konfiguration oder Daten geändert).
    pub stale: bool,
}

/// Status der Crawl-Analyse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct AnalysisStatus {
    /// Alle Teilschritte in Ausführungsreihenfolge.
    pub steps: Vec<AnalysisStepStatus>,
    /// Läuft gerade.
    pub running: bool,
    /// Aktueller Schritt.
    pub current: Option<String>,
    /// Fortschritt des aktuellen Schritts in Prozent.
    pub percent: Option<u8>,
}

/// Eine View hat eine neue Version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct ViewChanged {
    /// View.
    pub view_id: String,
    /// Neue Version.
    pub version: u64,
    /// Neue Zeilenzahl.
    pub total: Option<u64>,
}

/// Zustand des MCP-Servers in der App (OUT-285).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum McpStatus {
    /// Aus.
    Off,
    /// Bereit.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Port auf Loopback.
        port: u16,
    },
    /// Verbunden.
    #[serde(rename_all = "camelCase")]
    Connected {
        /// Port auf Loopback.
        port: u16,
        /// Anzahl Clients.
        clients: u32,
    },
}

/// Stufe einer Meldung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NoticeLevel {
    /// Information.
    Info,
    /// Warnung.
    Warning,
    /// Fehler.
    Error,
}

/// Eine Meldung an die Oberfläche (Startfehler, Plattenplatz, Crawl-Ende …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// Stufe.
    pub level: NoticeLevel,
    /// Code (i18n-Schlüssel `notice-<code>`).
    pub code: String,
    /// Argumente.
    pub args: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Gebündeltes Ereignis (`M2-ui-concept.md` 4.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct Tick {
    /// Laufende Nummer.
    pub seq: u64,
    /// Zeitpunkt (ISO 8601 UTC).
    pub at: String,
    /// Crawl-Status.
    pub crawl: Option<CrawlStatus>,
    /// Analyse-Status.
    pub analysis: Option<AnalysisStatus>,
    /// Views mit neuer Version.
    pub views: Vec<ViewChanged>,
    /// Version der Filterzähler.
    pub counts_version: u64,
    /// Version der Issues.
    pub issues_version: u64,
    /// Version der Seitenleiste.
    pub sidebar_version: u64,
    /// Operationen mit Fortschritt oder Ergebnis.
    pub ops: Vec<OpStatus>,
    /// MCP-Server.
    pub mcp: Option<McpStatus>,
    /// Meldungen.
    pub notices: Vec<Notice>,
}

/// Ereignis an ein Fenster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiEvent {
    /// Gebündelte Statusmeldung.
    #[serde(rename_all = "camelCase")]
    Tick {
        /// Inhalt (in einer Box, weil der Tick die übrigen Varianten um ein Vielfaches übertrifft).
        tick: Box<Tick>,
    },
    /// App-Einstellungen haben sich geändert (andere Fenster, Datei).
    #[serde(rename_all = "camelCase")]
    SettingsChanged {
        /// Geänderte Schlüssel (TOML-Pfade).
        keys: Vec<String>,
    },
    /// Einzelne Meldung außerhalb des Ticks (Startfehler).
    #[serde(rename_all = "camelCase")]
    Notice {
        /// Inhalt.
        notice: Notice,
    },
}
