//! Aufbau des `Tick` aus dem Sitzungszustand und Taktregeln.
//!
//! Der Coalescer (M2-K04) entscheidet, **wann** ein Tick gesendet wird; dieses Modul baut ihn nur zusammen:
//!
//! * [`TickSource`]: Beitragende (die Sitzung von K04, später MCP) tragen ihre Teile in [`TickParts`] ein.
//! * [`TickAssembler`]: vergibt die laufende Nummer, hält die letzten Versionsstände und liefert **keinen** Tick,
//!   wenn sich nichts geändert hat (Leerlauf ohne Nachrichten, UI-143).
//! * [`effective_period`]: Taktabstand eines Fensters (Standard 500 ms, 250 – 5000 ms, nie unter 100 ms, verdeckt
//!   höchstens 1 Hz).

use std::time::Duration;

use crate::dto::{AnalysisStatus, CrawlStatus, McpStatus, Notice, OpStatus, Tick, ViewChanged};

/// Kleinster Abstand zweier Ticks (≤ 10 Nachrichten/s über alle Ereignisarten, PERF-004).
pub const MIN_TICK_INTERVAL_MS: u32 = 100;
/// Kleinster Abstand bei unsichtbarem, minimiertem oder verdecktem Fenster (UI-143: ≤ 1 Hz).
pub const HIDDEN_TICK_INTERVAL_MS: u32 = 1000;

/// Taktabstand für ein Fenster: das eingestellte Intervall (mindestens 100 ms), bei verdecktem Fenster mindestens
/// eine Sekunde.
pub fn effective_period(interval_ms: u32, visible: bool) -> Duration {
    let base = interval_ms.max(MIN_TICK_INTERVAL_MS);
    let ms = if visible {
        base
    } else {
        base.max(HIDDEN_TICK_INTERVAL_MS)
    };
    Duration::from_millis(u64::from(ms))
}

/// Beiträge zu einem Tick. Leer = keine Änderung.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TickParts {
    /// Crawl-Status (neuer Stand).
    pub crawl: Option<CrawlStatus>,
    /// Analyse-Status (neuer Stand).
    pub analysis: Option<AnalysisStatus>,
    /// Views mit neuer Version.
    pub views: Vec<ViewChanged>,
    /// Neue Version der Filterzähler.
    pub counts_version: Option<u64>,
    /// Neue Version der Issues.
    pub issues_version: Option<u64>,
    /// Neue Version der Seitenleiste.
    pub sidebar_version: Option<u64>,
    /// MCP-Server (neuer Stand).
    pub mcp: Option<McpStatus>,
    /// Meldungen (seltene Ereignisse, erscheinen im nächsten Tick).
    pub notices: Vec<Notice>,
}

impl TickParts {
    /// Enthält keinen Beitrag?
    pub fn is_empty(&self) -> bool {
        self.crawl.is_none()
            && self.analysis.is_none()
            && self.views.is_empty()
            && self.counts_version.is_none()
            && self.issues_version.is_none()
            && self.sidebar_version.is_none()
            && self.mcp.is_none()
            && self.notices.is_empty()
    }
}

/// Beitragender zu den Ticks. `collect` darf nicht blockieren (keine Datenbankabfrage, kein Warten auf Sperren).
pub trait TickSource: Send + Sync {
    /// Trägt die seit dem letzten Aufruf geänderten Teile in `parts` ein.
    fn collect(&self, parts: &mut TickParts);
}

/// Baut `Tick`s: laufende Nummer und letzte Versionsstände.
#[derive(Debug, Default)]
pub struct TickAssembler {
    seq: u64,
    counts_version: u64,
    issues_version: u64,
    sidebar_version: u64,
}

impl TickAssembler {
    /// Neuer Zusammenbau, Nummerierung ab 1.
    pub fn new() -> TickAssembler {
        TickAssembler::default()
    }

    /// Nummer des zuletzt gebauten Ticks (0 = noch keiner).
    pub fn last_seq(&self) -> u64 {
        self.seq
    }

    /// Baut einen Tick aus `parts` und den Ständen der Operationen. `None`, wenn nichts zu melden ist.
    pub fn assemble(&mut self, at: String, parts: TickParts, ops: Vec<OpStatus>) -> Option<Tick> {
        if parts.is_empty() && ops.is_empty() {
            return None;
        }
        if let Some(v) = parts.counts_version {
            self.counts_version = v;
        }
        if let Some(v) = parts.issues_version {
            self.issues_version = v;
        }
        if let Some(v) = parts.sidebar_version {
            self.sidebar_version = v;
        }
        self.seq += 1;
        Some(Tick {
            seq: self.seq,
            at,
            crawl: parts.crawl,
            analysis: parts.analysis,
            views: parts.views,
            counts_version: self.counts_version,
            issues_version: self.issues_version,
            sidebar_version: self.sidebar_version,
            ops,
            mcp: parts.mcp,
            notices: parts.notices,
        })
    }
}

/// Baut einen einzelnen Tick ohne Zustand (Tests, einmalige Ticks); `seq` und Versionen setzt der Aufrufer.
pub fn assemble_tick(seq: u64, at: String, parts: TickParts, ops: Vec<OpStatus>) -> Option<Tick> {
    let mut a = TickAssembler::new();
    a.seq = seq.saturating_sub(1);
    a.assemble(at, parts, ops)
}
