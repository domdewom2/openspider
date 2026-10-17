//! Fortschritt eines Crawls (CRAWL-130).

use serde::{Deserialize, Serialize};

use crate::enums::{CrawlState, EndReason};
use crate::enums_m2::MemoryStage;
use crate::time::Timestamp;

/// Fortschrittsstand. Die JSON-Feldnamen `crawled`, `queued`, `percent`, `rate`, `avg_response_time`,
/// `elapsed_s`, `eta_s`, `state` sind fest (`--json-progress`); `avg_rate`, `paused_s` und `discovered` sind
/// JSON-Zusatzfelder (M1-P00c, docs/plan/M1.md 3.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    #[serde(default)]
    pub memory_used_bytes: Option<u64>,
    #[serde(default)]
    pub memory_limit_bytes: Option<u64>,
    /// Zustand des Crawls.
    pub state: CrawlState,
    /// Grund des Endes (nur bei beendetem Crawl).
    pub end_reason: Option<EndReason>,
    /// Abgeschlossene Abrufe ohne robots.txt.
    pub crawled: u64,
    /// Eingeplante, noch nicht gestartete URLs.
    pub queued: u64,
    /// Fortschritt in Prozent (0–100).
    pub percent: u8,
    /// Abrufe pro Sekunde (gleitend über 10 s).
    pub rate: f64,
    /// Mittlere Antwortzeit in Sekunden.
    pub avg_response_time: Option<f64>,
    /// Verstrichene Zeit in Sekunden.
    pub elapsed_s: u64,
    /// Geschätzte Restzeit in Sekunden (erst nach 30 s und 100 URLs).
    pub eta_s: Option<u64>,
    /// Aktive Threads.
    pub active_threads: u32,
    /// Startzeitpunkt.
    pub started_at: Timestamp,
    /// Mittlere Rate: abgeschlossene Abrufe ÷ Laufzeit ohne Pausen (UI-013). Zusatzfeld (M1-P00c); die
    /// Pflichtfelder von CRAWL-130 bleiben unverändert.
    #[serde(default)]
    pub avg_rate: f64,
    /// Summe der Pausen in Sekunden. Zusatzfeld (M1-P00c).
    #[serde(default)]
    pub paused_s: u64,
    /// Bekannte URLs gesamt. Zusatzfeld (M1-P00c).
    #[serde(default)]
    pub discovered: u64,
    /// Speicherstufe des Ressourcenschutzes (CRAWL-129, ab M2); `None`, solange die Engine sie nicht ermittelt.
    /// Zusatzfeld (M2-F02, `docs/plan/M2.md` 3.3a); befüllt von M2-E02.
    #[serde(default)]
    pub memory_stage: Option<MemoryStage>,
    /// Größe der Crawl-Datei in Byte (CRAWL-129, ab M2); `None`, solange die Engine sie nicht misst.
    /// Zusatzfeld (M2-F02); befüllt von M2-E02.
    #[serde(default)]
    pub file_bytes: Option<u64>,
}
