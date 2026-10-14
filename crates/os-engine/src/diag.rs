//! Messzähler des Crawl-Pfads (M1-P30, Windows-Durchsatz): wo geht die Zeit je URL hin?
//!
//! Prozessweit und nur aus atomaren Zählern (kein Sperren, keine Zuteilung): je URL einige `Instant::now()` und
//! `fetch_add`, das sind Nanosekunden gegen Millisekunden eines Abrufs; die Zähler stehen deshalb dauerhaft im
//! Produkt. Der Lasttest (`os-loadtest crawl-child`) liest sie mit [`snapshot`] am Ende des Crawls und legt sie in
//! den Bericht (`diag_engine_*`); `perf-diag.yml` wertet sie je Runner aus.
//!
//! Alle `*_ns`-Werte sind Summen über alle Worker (bei 8 Workern kann die Summe die Wanduhrzeit übersteigen).
//!
//! | Gruppe | Zähler |
//! |---|---|
//! | Worker | `items`, `item_ns` (gesamt), `host_wait_ns`, `robots_ns`, `fetch_ns`/`fetch_max_ns`/`fetch_slow` (Abrufe ab 250 ms), `analyze_ns`, `assemble_ns` (einschließlich `reserve_ns`), `reserve_ns`/`reserve_max_ns` (Platz im Kanal), `send_ns`/`send_max_ns` (Senden mit Gegendruck) |
//! | Steuerschleife | `run_loops`, `run_pops` |
//! | Writer-Thread | `w_batches`, `w_pages`, `w_write_ns`/`w_write_max_ns`, `w_idle_ns` (wartet auf Arbeit), `w_poll_sleeps`/`w_poll_sleep_ns` (Abfragen mit Schlafen), `w_queue_max`, `w_b_le50`, `w_b_le200`, `w_b_le1000`, `w_b_gt1000` (Batchgrößen in Seiten) |

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Abrufe ab dieser Dauer zählen als langsam (250 ms).
pub const SLOW_FETCH_NS: u64 = 250_000_000;

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
    items,
    item_ns,
    host_wait_ns,
    robots_ns,
    fetch_ns,
    fetch_max_ns,
    fetch_slow,
    analyze_ns,
    assemble_ns,
    reserve_ns,
    reserve_max_ns,
    send_ns,
    send_max_ns,
    run_loops,
    run_pops,
    w_batches,
    w_pages,
    w_write_ns,
    w_write_max_ns,
    w_idle_ns,
    w_poll_sleeps,
    w_poll_sleep_ns,
    w_queue_max,
    w_b_le50,
    w_b_le200,
    w_b_le1000,
    w_b_gt1000,
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

/// Nanosekunden seit `t`.
pub fn nanos(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Addiert die seit `t` vergangene Zeit auf `sum` und gibt sie zurück.
pub fn add(sum: &AtomicU64, t: Instant) -> u64 {
    let ns = nanos(t);
    sum.fetch_add(ns, Ordering::Relaxed);
    ns
}

/// Wie [`add`], zusätzlich das Maximum in `max`.
pub fn add_max(sum: &AtomicU64, max: &AtomicU64, t: Instant) -> u64 {
    let ns = add(sum, t);
    max.fetch_max(ns, Ordering::Relaxed);
    ns
}

/// Erhöht `c` um 1.
pub fn inc(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

/// Verbucht die Größe eines geschriebenen Batches in Seiten.
pub fn record_batch_pages(pages: usize) {
    let bucket = match pages {
        0..=50 => &STATS.w_b_le50,
        51..=200 => &STATS.w_b_le200,
        201..=1000 => &STATS.w_b_le1000,
        _ => &STATS.w_b_gt1000,
    };
    inc(bucket);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Infrastruktur (M1-P30), kein Spec-Tag.
    #[test]
    fn snapshot_lists_every_counter_and_reset_clears_them() {
        // Eigene Zähler: die Prozess-Zähler teilen sich parallel laufende Tests.
        let local = Stats::new();
        local.items.fetch_add(3, Ordering::Relaxed);
        let snap = local.snapshot();
        assert_eq!(
            snap.iter().find(|(n, _)| *n == "items"),
            Some(&("items", 3))
        );
        assert!(snap.iter().any(|(n, _)| *n == "w_b_gt1000"));
        local.reset();
        assert!(local.snapshot().iter().all(|(_, v)| *v == 0));
    }
}
