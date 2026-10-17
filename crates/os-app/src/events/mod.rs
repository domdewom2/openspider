//! Ereigniskanal je Fenster: Senke, Tick-Zusammenbau (M2-F03) und Coalescer (M2-K04).
//!
//! Ein einziger Kanal je Fenster (M2-ui-concept 2.5): alle Änderungen laufen gebündelt als `Tick`. `sink` definiert
//! die Senke (Tauri-`Channel`, Bridge-WebSocket, Test), `tick` baut den `Tick` aus Teilen zusammen und kennt die
//! Taktregeln (Taktabstand, verdecktes Fenster); `coalescer` (M2-K04) entscheidet, wann gesendet wird.
//!
//! STUB(M2-K04): `coalescer` füllt K04.

pub mod coalescer;
pub mod sink;
pub mod tick;

pub use sink::{CollectingSink, EventSink, SinkClosed};
pub use tick::{
    assemble_tick, effective_period, TickAssembler, TickParts, TickSource, HIDDEN_TICK_INTERVAL_MS,
    MIN_TICK_INTERVAL_MS,
};
