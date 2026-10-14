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

/// Messzähler des Schreibpfads (M1-P30, Windows-Durchsatz): Zeit je Phase eines `SqliteSink`-Batches.
///
/// Prozessweit, nur atomare Zähler (kein Sperren, keine Zuteilung): Sie kosten je Batch einige Nanosekunden und
/// stehen deshalb dauerhaft im Produkt. Der Lasttest (`os-loadtest`, Kindprozess) liest sie mit [`snapshot`] am
/// Ende des Crawls und legt sie in den Bericht (`diag_store_*`); `perf-diag.yml` wertet sie je Runner aus.
///
/// | Zähler | Bedeutung |
/// |---|---|
/// | `batches` | geschriebene Batches |
/// | `build_ns` | Zeit von `BEGIN` bis vor `COMMIT` (Einfügen und Ändern der Zeilen) |
/// | `commit_ns` / `commit_max_ns` | Summe/Maximum von `COMMIT` (enthält den automatischen WAL-Checkpoint) |
/// | `commit_slow` | Commits ab [`SLOW_COMMIT_NS`] |
/// | `checkpoint_ns` / `checkpoints` / `checkpoint_max_ns` | ausdrückliche `wal_checkpoint`-Aufrufe |
pub mod diag {

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// Commits ab dieser Dauer zählen als langsam (25 ms).
    pub const SLOW_COMMIT_NS: u64 = 25_000_000;

    macro_rules! counters {
        ($($name:ident),* $(,)?) => {
            /// Prozessweite Zähler.
            pub struct Stats { $(pub $name: AtomicU64),* }
            impl Stats {
                const fn new() -> Self { Stats { $($name: AtomicU64::new(0)),* } }
                /// Alle Zähler als `(Name, Wert)`.
                pub fn snapshot(&self) -> Vec<(&'static str, u64)> {
                    vec![$((stringify!($name), self.$name.load(Ordering::Relaxed))),*]
                }
                /// Setzt alle Zähler auf 0.
                pub fn reset(&self) { $(self.$name.store(0, Ordering::Relaxed);)* }
            }
        };
    }

    counters!(
        batches,
        build_ns,
        commit_ns,
        commit_max_ns,
        commit_slow,
        checkpoints,
        checkpoint_ns,
        checkpoint_max_ns,
    );

    /// Die Zähler dieses Prozesses.
    pub static STATS: Stats = Stats::new();

    /// Alle Zähler als `(Name, Wert)`.
    pub fn snapshot() -> Vec<(&'static str, u64)> {
        STATS.snapshot()
    }

    /// Setzt alle Zähler zurück (Tests, Messläufe).
    pub fn reset() {
        STATS.reset();
    }

    fn nanos(t: Instant) -> u64 {
        u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// Verbucht die Bauzeit eines Batches.
    pub(crate) fn record_build(t: Instant) {
        STATS.batches.fetch_add(1, Ordering::Relaxed);
        STATS.build_ns.fetch_add(nanos(t), Ordering::Relaxed);
    }

    /// Verbucht einen `COMMIT`.
    pub(crate) fn record_commit(t: Instant) {
        let ns = nanos(t);
        STATS.commit_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.commit_max_ns.fetch_max(ns, Ordering::Relaxed);
        if ns >= SLOW_COMMIT_NS {
            STATS.commit_slow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Verbucht einen ausdrücklichen WAL-Checkpoint.
    pub(crate) fn record_checkpoint(t: Instant) {
        let ns = nanos(t);
        STATS.checkpoints.fetch_add(1, Ordering::Relaxed);
        STATS.checkpoint_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.checkpoint_max_ns.fetch_max(ns, Ordering::Relaxed);
    }
}
