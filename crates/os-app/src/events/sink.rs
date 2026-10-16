//! Senke eines Fensters für `UiEvent` (Tauri `Channel`, Bridge-WebSocket, Test).
//!
//! STUB(M2-F03): Signatur aus `docs/plan/M2.md` 3.6.

use crate::dto::UiEvent;

/// Die Senke ist geschlossen (Fenster weg).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkClosed;

/// Empfänger der Ereignisse eines Fensters.
pub trait EventSink: Send + Sync {
    /// Sendet ein Ereignis.
    fn send(&self, ev: UiEvent) -> Result<(), SinkClosed>;
}
