//! Fortschritt eines Crawls (CRAWL-130).

use serde::{Deserialize, Serialize};

use crate::enums::{CrawlState, EndReason};
use crate::time::Timestamp;

/// Fortschrittsstand. Die JSON-Feldnamen `crawled`, `queued`, `percent`, `rate`, `avg_response_time`,
/// `elapsed_s`, `eta_s`, `state` sind fest (`--json-progress`); `avg_rate`, `paused_s` und `discovered` sind
/// JSON-Zusatzfelder (M1-P00c, docs/plan/M1.md 3.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
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
}
