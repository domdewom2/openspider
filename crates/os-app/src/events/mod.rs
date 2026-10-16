//! Ereigniskanal je Fenster: Coalescer (≤ 10 Nachrichten/s), Tick, Senke.
//!
//! STUB(M2-F03, M2-K04): `sink`, `tick` (F03) und `coalescer` (K04) füllen die Dateien.

pub mod coalescer;
pub mod sink;
pub mod tick;

pub use sink::{EventSink, SinkClosed};
