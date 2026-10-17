//! Senke eines Fensters für `UiEvent` (Tauri `Channel`, Bridge-WebSocket, Test).
//!
//! Signatur aus `docs/plan/M2.md` 3.6. Eine Senke darf nicht lange blockieren; schlägt `send` mit [`SinkClosed`] fehl,
//! entfernt `App` das Abonnement (Fenster weg).

use std::sync::Mutex;

use crate::dto::UiEvent;

/// Die Senke ist geschlossen (Fenster weg).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkClosed;

/// Empfänger der Ereignisse eines Fensters.
pub trait EventSink: Send + Sync {
    /// Sendet ein Ereignis.
    fn send(&self, ev: UiEvent) -> Result<(), SinkClosed>;
}

/// Senke, die alle Ereignisse sammelt (Tests, Diagnose). `close()` lässt weitere Sendungen mit [`SinkClosed`]
/// scheitern.
#[derive(Debug, Default)]
pub struct CollectingSink {
    events: Mutex<Vec<UiEvent>>,
    closed: std::sync::atomic::AtomicBool,
}

impl CollectingSink {
    /// Leere Senke.
    pub fn new() -> CollectingSink {
        CollectingSink::default()
    }

    /// Alle bisher empfangenen Ereignisse (und leert die Senke).
    pub fn take(&self) -> Vec<UiEvent> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Anzahl der noch nicht abgeholten Ereignisse.
    pub fn len(&self) -> usize {
        self.events.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Ohne Ereignisse?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Schließt die Senke.
    pub fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl EventSink for CollectingSink {
    fn send(&self, ev: UiEvent) -> Result<(), SinkClosed> {
        if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SinkClosed);
        }
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(ev);
        Ok(())
    }
}
