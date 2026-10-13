//! Schreibschnittstelle der Engine (objektsicher; die Engine hält immer `Box<dyn CrawlSink>`).

use os_model::WriteBatch;

use crate::error::StoreError;

/// Senke für die Ergebnisse eines Crawls. Implementierung: `SqliteSink` (M1-P07); Tests und Lasttests
/// setzen eigene Senken ein (`EngineDeps.sink_factory`).
pub trait CrawlSink: Send {
    /// Schreibt einen Batch in einer Transaktion.
    fn write(&mut self, batch: WriteBatch) -> Result<(), StoreError>;

    /// Schreibt ausstehende Daten dauerhaft (WAL-Checkpoint).
    fn checkpoint(&mut self) -> Result<(), StoreError>;
}
