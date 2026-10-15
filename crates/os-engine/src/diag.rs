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
//! | Writer-Thread | `w_batches`, `w_pages`, `w_pages_max` (größter geschriebener Batch in Seiten, M1-P33), `w_write_ns`/`w_write_max_ns`, `w_idle_ns` (wartet auf Arbeit), `w_poll_sleeps`/`w_poll_sleep_ns` (Warten mit Zeitgrenze auf weitere Nachrichten, Sammelzeit des Gruppen-Commits), `w_queue_max`, `w_b_le50`, `w_b_le200`, `w_b_le1000`, `w_b_gt1000` (Batchgrößen in Seiten) |
//! | Speicher (M1-P33) | Spitzenwerte der geschätzten Größe der Kernstrukturen, abgetastet bei jedem Fortschrittstakt (100 ms) und am Ende; Bytes = Zahl der Einträge bzw. Kapazität mal Elementgröße plus Heap-Anteil der Adressen (Schätzung, keine Messung des Allokators): `mem_seen_entries`/`mem_seen_bytes`/`mem_seen_overflow` (Seen-Menge, Überlaufmenge), `mem_queue_items`/`mem_queue_bytes` (Einträge der Warteschlange im RAM samt Adressen), `mem_queue_disk` (nur in der Tabelle `queue` stehende Einträge), `mem_queue_pinned` (Einträge, die nicht ausgelagert werden dürfen), `mem_heap_entries`/`mem_heap_bytes` (Sortierstruktur), `mem_redirect_entries`/`mem_redirect_bytes`, `mem_sitemap_entries`/`mem_sitemap_bytes` (`sitemap_keys`, hreflang-Zähler), `mem_core_bytes` (Summe), `mem_writer_batch_bytes_max` (größter zusammengefasster Batch des Writer-Threads), `q_spilled`/`q_loaded`/`q_refills`/`q_trims`/`q_improved_disk` (Auslagern und Nachladen der Warteschlange), `seen_collisions` (echte Schlüsselkollisionen) |

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
    w_pages_max,
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
    mem_seen_entries,
    mem_seen_bytes,
    mem_seen_overflow,
    mem_queue_items,
    mem_queue_bytes,
    mem_queue_disk,
    mem_queue_pinned,
    mem_heap_entries,
    mem_heap_bytes,
    mem_redirect_entries,
    mem_redirect_bytes,
    mem_sitemap_entries,
    mem_sitemap_bytes,
    mem_core_bytes,
    mem_writer_batch_bytes_max,
    q_spilled,
    q_loaded,
    q_refills,
    q_trims,
    q_improved_disk,
    seen_collisions,
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

/// Hält `value` als Spitzenwert in `peak`.
pub fn peak(peak: &AtomicU64, value: u64) {
    peak.fetch_max(value, Ordering::Relaxed);
}

/// Geschätzte Größe der Kernstrukturen der Engine (M1-P33), siehe Tabelle oben.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoreMemory {
    /// Einträge der Seen-Menge (einschließlich Überlaufmenge).
    pub seen_entries: u64,
    /// Geschätzte Bytes der Seen-Menge.
    pub seen_bytes: u64,
    /// Einträge der Überlaufmenge (volle Adresse).
    pub seen_overflow: u64,
    /// Einträge der Warteschlange im RAM.
    pub queue_items: u64,
    /// Geschätzte Bytes der Warteschlange im RAM (Einträge und Adressen).
    pub queue_bytes: u64,
    /// Einträge, die nur in der Tabelle `queue` stehen.
    pub queue_disk: u64,
    /// RAM-Einträge, die nicht ausgelagert werden dürfen.
    pub queue_pinned: u64,
    /// Einträge der Sortierstruktur (einschließlich ungültiger).
    pub heap_entries: u64,
    /// Geschätzte Bytes der Sortierstruktur.
    pub heap_bytes: u64,
    /// Kanten und Markierungen des Weiterleitungsgraphen.
    pub redirect_entries: u64,
    /// Geschätzte Bytes des Weiterleitungsgraphen.
    pub redirect_bytes: u64,
    /// Einträge der Sitemap-Strukturen.
    pub sitemap_entries: u64,
    /// Geschätzte Bytes der Sitemap-Strukturen.
    pub sitemap_bytes: u64,
}

impl CoreMemory {
    /// Summe der geschätzten Bytes.
    pub fn total_bytes(&self) -> u64 {
        self.seen_bytes
            + self.queue_bytes
            + self.heap_bytes
            + self.redirect_bytes
            + self.sitemap_bytes
    }
}

/// Übernimmt eine Schätzung als Spitzenwerte in die Zähler.
pub fn record_core_memory(m: &CoreMemory) {
    let s = &STATS;
    peak(&s.mem_seen_entries, m.seen_entries);
    peak(&s.mem_seen_bytes, m.seen_bytes);
    peak(&s.mem_seen_overflow, m.seen_overflow);
    peak(&s.mem_queue_items, m.queue_items);
    peak(&s.mem_queue_bytes, m.queue_bytes);
    peak(&s.mem_queue_disk, m.queue_disk);
    peak(&s.mem_queue_pinned, m.queue_pinned);
    peak(&s.mem_heap_entries, m.heap_entries);
    peak(&s.mem_heap_bytes, m.heap_bytes);
    peak(&s.mem_redirect_entries, m.redirect_entries);
    peak(&s.mem_redirect_bytes, m.redirect_bytes);
    peak(&s.mem_sitemap_entries, m.sitemap_entries);
    peak(&s.mem_sitemap_bytes, m.sitemap_bytes);
    peak(&s.mem_core_bytes, m.total_bytes());
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
