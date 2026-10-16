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
/// | `checkpoint_ns` / `checkpoints` / `checkpoint_max_ns` / `checkpoint_frames` | ausdrückliche `wal_checkpoint`-Aufrufe, `frames` = Frames im WAL zu Beginn (je 4 KiB) |
/// | `bg_checkpoints` / `bg_checkpoint_ns` / `bg_checkpoint_max_ns` / `bg_checkpoint_frames` | Durchläufe des Hintergrund-Checkpoints (K-76): Zahl, Summe, Maximum, Frames im WAL zu Beginn |
/// | `bg_checkpoint_errors` | Fehler des Hintergrund-Threads (Diagnose; der Crawl läuft weiter) |
/// | `wait_checkpoint_ns` / `wait_checkpoint_n` | Zeit und Zahl der Wartepausen des Writers auf den Hintergrund-Checkpoint (Soll 0) |
/// | `link_index_builds` / `link_index_build_ns` | Aufbau des Index `links_target` (K-74, am regulären Crawl-Ende bzw. durch `link_index::ensure`) |
/// | `url_index_builds` / `url_index_build_ns` / `url_index_build_max_ns` | Aufbau der Indizes mit zufälligen Schlüsseln über `urls` (N22, `url_index::ensure`): Zahl der Indizes, Gesamtdauer, längster einzelner Aufbau |
/// | `end_data_ns` / `end_commit_ns` | Endbatch (Schreibaufruf mit `Completed`, M1-N27): Dauer der Datenanweisungen vor dem Indexaufbau und des `COMMIT` (mit dem WAL-Checkpoint im Commit); dazwischen liegen `link_index_build_ns` und, nur in einer kleinen Datei, `url_index_build_ns` |
/// | `url_index_prefetch_ns` / `_max_ns` / `_rows` | Vorwärmen des Dateicaches für den Aufbau (Leser-Threads): Summe, längster Thread, gelesene Zeilen |
///
/// **Verlauf je 100 Batches (M1-N22, PERF-002):** Die Reihen `<name>_h00` … `<name>_h15` fassen je 100 Batches in
/// einem Eimer zusammen (Eimer = Nummer des Batches / 100; der letzte Eimer `h15` sammelt den Rest). So zeigt sich,
/// welcher Teil des Schreibpfads mit der Dateigröße wächst:
///
/// | Reihe | Bedeutung |
/// |---|---|
/// | `batches_hNN` | Zahl der Batches im Eimer (Teiler für Durchschnitte) |
/// | `build_ns_hNN` | Aufbauzeit (wie `build_ns`) |
/// | `commit_ns_hNN` / `commit_max_ns_hNN` | Summe/Maximum des `COMMIT` (wie `commit_ns`; ohne die Rückführung, wenn `OS_STORE_WAL_EXPLICIT=1`) |
/// | `g_urls_ins_ns_hNN` | Aufbau: neue `urls`-Zeilen und ihre `url_counters`-Zeile |
/// | `g_urls_upd_ns_hNN` | Aufbau: `UPDATE urls` der Seiten und `UrlUpdate`s (pflegt alle Sekundärindizes von `urls`) |
/// | `g_links_ns_hNN` | Aufbau: Einfügen in `links` |
/// | `g_counters_ns_hNN` | Aufbau: Zähler in `url_counters` (Beiträge sammeln und schreiben) |
/// | `g_queue_ns_hNN` | Aufbau: Einfügen und Löschen in `queue` |
/// | `g_rest_ns_hNN` | Aufbau: alles Übrige (Kindtabellen, Lesen des alten Zustands, Meta, Ereignisse, Zähler, Flags) |
/// | `wal_cp_ns_hNN` | Zeit der ausdrücklichen WAL-Rückführung nach dem Commit (nur `OS_STORE_WAL_EXPLICIT=1`) |
/// | `wal_bytes_max_hNN` | größte Länge der `-wal`-Datei nach einem Commit |
/// | `cache_miss_hNN` / `cache_write_hNN` | Seiten, die der Seitencache nachladen musste / die er in das WAL schrieb (`sqlite3_db_status`) |
///
/// Einzelwerte: `wal_cp_n` / `wal_cp_ns` / `wal_cp_max_ns` / `wal_cp_frames` / `wal_cp_frames_left` (ausdrückliche
/// Rückführungen: Zahl, Summe, Maximum, Frames im WAL davor, noch nicht zurückgeführte Frames danach),
/// `cache_hit` / `cache_miss` / `cache_write` / `cache_spill` (ganze Läufe; `spill` = Seiten, die der Cache vor dem
/// Commit ins WAL schreiben musste) und `wal_bytes_max`.
pub mod diag {

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// Commits ab dieser Dauer zählen als langsam (25 ms).
    pub const SLOW_COMMIT_NS: u64 = 25_000_000;

    /// Zahl der Eimer des Verlaufs.
    pub const BUCKETS: usize = 16;
    /// Batches je Eimer.
    pub const BUCKET_BATCHES: u64 = 100;

    macro_rules! counters {
        ($($name:ident),* $(,)?) => {
            /// Prozessweite Zähler.
            pub struct Stats { $(pub $name: AtomicU64,)* series: Series }
            impl Stats {
                const fn new() -> Self { Stats { $($name: AtomicU64::new(0),)* series: Series::new() } }
                /// Alle Zähler als `(Name, Wert)`, danach die Reihen je 100 Batches.
                pub fn snapshot(&self) -> Vec<(&'static str, u64)> {
                    let mut out = vec![$((stringify!($name), self.$name.load(Ordering::Relaxed))),*];
                    self.series.append_to(&mut out);
                    out
                }
                /// Setzt alle Zähler auf 0.
                pub fn reset(&self) {
                    $(self.$name.store(0, Ordering::Relaxed);)*
                    self.series.reset();
                }
            }
        };
    }

    macro_rules! bucket_names {
        ($p:literal) => {
            [
                concat!($p, "_h00"),
                concat!($p, "_h01"),
                concat!($p, "_h02"),
                concat!($p, "_h03"),
                concat!($p, "_h04"),
                concat!($p, "_h05"),
                concat!($p, "_h06"),
                concat!($p, "_h07"),
                concat!($p, "_h08"),
                concat!($p, "_h09"),
                concat!($p, "_h10"),
                concat!($p, "_h11"),
                concat!($p, "_h12"),
                concat!($p, "_h13"),
                concat!($p, "_h14"),
                concat!($p, "_h15"),
            ]
        };
    }

    /// Reihen je 100 Batches (ein Zähler je Eimer). `Sum`: Eimer addieren, `Max`: Maximum halten.
    macro_rules! series {
        ($($field:ident => $name:literal),* $(,)?) => {
            /// Die Reihen je 100 Batches.
            pub struct Series { $(pub $field: [AtomicU64; BUCKETS]),* }
            impl Series {
                const fn new() -> Self {
                    Series { $($field: [const { AtomicU64::new(0) }; BUCKETS]),* }
                }
                fn append_to(&self, out: &mut Vec<(&'static str, u64)>) {
                    $({
                        let names: [&'static str; BUCKETS] = bucket_names!($name);
                        for (i, n) in names.iter().enumerate() {
                            out.push((n, self.$field[i].load(Ordering::Relaxed)));
                        }
                    })*
                }
                fn reset(&self) {
                    $(for c in &self.$field { c.store(0, Ordering::Relaxed); })*
                }
            }
        };
    }

    series!(
        batches_h => "batches",
        build_ns_h => "build_ns",
        commit_ns_h => "commit_ns",
        commit_max_ns_h => "commit_max_ns",
        g_urls_ins_ns_h => "g_urls_ins_ns",
        g_urls_upd_ns_h => "g_urls_upd_ns",
        g_links_ns_h => "g_links_ns",
        g_counters_ns_h => "g_counters_ns",
        g_queue_ns_h => "g_queue_ns",
        g_rest_ns_h => "g_rest_ns",
        wal_cp_ns_h => "wal_cp_ns",
        wal_bytes_max_h => "wal_bytes_max",
        cache_miss_h => "cache_miss",
        cache_write_h => "cache_write",
    );

    counters!(
        batches,
        build_ns,
        commit_ns,
        commit_max_ns,
        commit_slow,
        checkpoints,
        checkpoint_ns,
        checkpoint_max_ns,
        checkpoint_frames,
        link_index_builds,
        link_index_build_ns,
        url_index_builds,
        url_index_build_ns,
        url_index_build_max_ns,
        url_index_prefetch_ns,
        url_index_prefetch_rows,
        url_index_prefetch_max_ns,
        end_data_ns,
        end_commit_ns,
        bg_checkpoints,
        bg_checkpoint_ns,
        bg_checkpoint_max_ns,
        bg_checkpoint_frames,
        bg_checkpoint_errors,
        wait_checkpoint_ns,
        wait_checkpoint_n,
        wal_cp_n,
        wal_cp_ns,
        wal_cp_max_ns,
        wal_cp_frames,
        wal_cp_frames_left,
        wal_bytes_max,
        cache_hit,
        cache_miss,
        cache_write,
        cache_spill,
    );

    /// Die Zähler dieses Prozesses.
    pub static STATS: Stats = Stats::new();

    impl Stats {
        /// Die Reihen je 100 Batches.
        pub fn series(&self) -> &Series {
            &self.series
        }
    }

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

    /// Eimer eines Batches mit der laufenden Nummer `index`.
    pub fn bucket_of(index: u64) -> usize {
        usize::try_from(index / BUCKET_BATCHES).map_or(BUCKETS - 1, |b| b.min(BUCKETS - 1))
    }

    /// Aufbauzeit je Anweisungsgruppe eines Batches (ns). Der Rest ergibt sich aus der Gesamtzeit.
    #[derive(Debug, Default, Clone, Copy)]
    pub(crate) struct Groups {
        pub urls_ins: u64,
        pub urls_upd: u64,
        pub links: u64,
        pub counters: u64,
        pub queue: u64,
    }

    impl Groups {
        /// Misst `f` und schlägt die Zeit der Gruppe zu, die `pick` auswählt.
        pub(crate) fn time<T>(
            &mut self,
            pick: fn(&mut Groups) -> &mut u64,
            f: impl FnOnce() -> T,
        ) -> T {
            let t = Instant::now();
            let out = f();
            *pick(self) += nanos(t);
            out
        }
    }

    /// Zähler des Seitencaches der Verbindung seit der letzten Abfrage.
    #[derive(Debug, Default, Clone, Copy)]
    pub(crate) struct CacheDelta {
        pub hit: u64,
        pub miss: u64,
        pub write: u64,
        pub spill: u64,
    }

    /// Liest und setzt die Cache-Zähler der Verbindung zurück (`sqlite3_db_status`, Diagnose).
    pub(crate) fn cache_delta(conn: &rusqlite::Connection) -> CacheDelta {
        use rusqlite::ffi;
        let read = |op: i32| -> u64 {
            let (mut cur, mut hi) = (0i32, 0i32);
            // SAFETY: `handle` ist für die Dauer des Aufrufs gültig; `sqlite3_db_status` liest nur Zähler der
            // Verbindung und setzt sie zurück (Argument `1`).
            let rc = unsafe { ffi::sqlite3_db_status(conn.handle(), op, &mut cur, &mut hi, 1) };
            if rc == ffi::SQLITE_OK {
                u64::try_from(cur).unwrap_or(0)
            } else {
                0
            }
        };
        CacheDelta {
            hit: read(ffi::SQLITE_DBSTATUS_CACHE_HIT),
            miss: read(ffi::SQLITE_DBSTATUS_CACHE_MISS),
            write: read(ffi::SQLITE_DBSTATUS_CACHE_WRITE),
            spill: read(ffi::SQLITE_DBSTATUS_CACHE_SPILL),
        }
    }

    /// Verbucht die Bauzeit eines Batches und liefert seinen Eimer.
    pub(crate) fn record_build(t: Instant, g: &Groups) -> usize {
        let index = STATS.batches.fetch_add(1, Ordering::Relaxed);
        let b = bucket_of(index);
        let ns = nanos(t);
        STATS.build_ns.fetch_add(ns, Ordering::Relaxed);
        let s = &STATS.series;
        s.batches_h[b].fetch_add(1, Ordering::Relaxed);
        s.build_ns_h[b].fetch_add(ns, Ordering::Relaxed);
        s.g_urls_ins_ns_h[b].fetch_add(g.urls_ins, Ordering::Relaxed);
        s.g_urls_upd_ns_h[b].fetch_add(g.urls_upd, Ordering::Relaxed);
        s.g_links_ns_h[b].fetch_add(g.links, Ordering::Relaxed);
        s.g_counters_ns_h[b].fetch_add(g.counters, Ordering::Relaxed);
        s.g_queue_ns_h[b].fetch_add(g.queue, Ordering::Relaxed);
        let named = g.urls_ins + g.urls_upd + g.links + g.counters + g.queue;
        s.g_rest_ns_h[b].fetch_add(ns.saturating_sub(named), Ordering::Relaxed);
        b
    }

    /// Verbucht die Cache-Zähler eines Batches (Aufbau und Commit) im Eimer `bucket`.
    pub(crate) fn record_cache(bucket: usize, cache: &CacheDelta) {
        STATS.series.cache_miss_h[bucket].fetch_add(cache.miss, Ordering::Relaxed);
        STATS.series.cache_write_h[bucket].fetch_add(cache.write, Ordering::Relaxed);
        STATS.cache_hit.fetch_add(cache.hit, Ordering::Relaxed);
        STATS.cache_miss.fetch_add(cache.miss, Ordering::Relaxed);
        STATS.cache_write.fetch_add(cache.write, Ordering::Relaxed);
        STATS.cache_spill.fetch_add(cache.spill, Ordering::Relaxed);
    }

    /// Verbucht einen `COMMIT` des Batches im Eimer `bucket`.
    pub(crate) fn record_commit(t: Instant, bucket: usize) {
        let ns = nanos(t);
        STATS.commit_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.commit_max_ns.fetch_max(ns, Ordering::Relaxed);
        STATS.series.commit_ns_h[bucket].fetch_add(ns, Ordering::Relaxed);
        STATS.series.commit_max_ns_h[bucket].fetch_max(ns, Ordering::Relaxed);
        if ns >= SLOW_COMMIT_NS {
            STATS.commit_slow.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Verbucht die Länge der `-wal`-Datei nach einem Commit.
    pub(crate) fn record_wal_bytes(bucket: usize, bytes: u64) {
        STATS.wal_bytes_max.fetch_max(bytes, Ordering::Relaxed);
        STATS.series.wal_bytes_max_h[bucket].fetch_max(bytes, Ordering::Relaxed);
    }

    /// Verbucht eine ausdrückliche WAL-Rückführung nach dem Commit (`OS_STORE_WAL_EXPLICIT=1`): `frames` im WAL
    /// davor, `left` nicht zurückgeführte Frames danach.
    pub(crate) fn record_wal_return(t: Instant, bucket: usize, frames: u64, left: u64) {
        let ns = nanos(t);
        STATS.wal_cp_n.fetch_add(1, Ordering::Relaxed);
        STATS.wal_cp_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.wal_cp_max_ns.fetch_max(ns, Ordering::Relaxed);
        STATS.wal_cp_frames.fetch_add(frames, Ordering::Relaxed);
        STATS.wal_cp_frames_left.fetch_add(left, Ordering::Relaxed);
        STATS.series.wal_cp_ns_h[bucket].fetch_add(ns, Ordering::Relaxed);
    }

    /// Verbucht einen Durchlauf des Hintergrund-Checkpoints.
    pub(crate) fn record_background_checkpoint(t: Instant, frames: u64) {
        let ns = nanos(t);
        STATS.bg_checkpoints.fetch_add(1, Ordering::Relaxed);
        STATS.bg_checkpoint_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.bg_checkpoint_max_ns.fetch_max(ns, Ordering::Relaxed);
        STATS
            .bg_checkpoint_frames
            .fetch_add(frames, Ordering::Relaxed);
    }

    /// Verbucht einen Fehler des Hintergrund-Threads.
    pub(crate) fn record_background_error() {
        STATS.bg_checkpoint_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Verbucht eine Wartepause des Writers auf den Hintergrund-Checkpoint.
    pub(crate) fn record_checkpoint_wait(t: Instant) {
        STATS.wait_checkpoint_n.fetch_add(1, Ordering::Relaxed);
        STATS
            .wait_checkpoint_ns
            .fetch_add(nanos(t), Ordering::Relaxed);
    }

    /// Verbucht den Aufbau von `n` Indizes über `urls` (`url_index::ensure`).
    pub(crate) fn record_url_index(t: Instant, n: u64) {
        STATS.url_index_builds.fetch_add(n, Ordering::Relaxed);
        STATS
            .url_index_build_ns
            .fetch_add(nanos(t), Ordering::Relaxed);
    }

    /// Verbucht die Dauer eines einzelnen Indexaufbaus über `urls`.
    pub(crate) fn record_url_index_one(t: Instant) {
        STATS
            .url_index_build_max_ns
            .fetch_max(nanos(t), Ordering::Relaxed);
    }

    /// Verbucht einen Leser des Vorwärmens (Summe über die Threads, Zeilen).
    pub(crate) fn record_prefetch(t: Instant, rows: u64) {
        let ns = nanos(t);
        STATS.url_index_prefetch_ns.fetch_add(ns, Ordering::Relaxed);
        STATS
            .url_index_prefetch_max_ns
            .fetch_max(ns, Ordering::Relaxed);
        STATS
            .url_index_prefetch_rows
            .fetch_add(rows, Ordering::Relaxed);
    }

    /// Verbucht den Aufbau des Index `links_target`.
    pub(crate) fn record_link_index(t: Instant) {
        STATS.link_index_builds.fetch_add(1, Ordering::Relaxed);
        STATS
            .link_index_build_ns
            .fetch_add(nanos(t), Ordering::Relaxed);
    }

    /// Verbucht die Teile des Endbatchs (M1-N27): Daten vor dem Indexaufbau und `COMMIT`.
    pub(crate) fn record_end_batch(data: std::time::Duration, commit: std::time::Duration) {
        let ns = |d: std::time::Duration| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        STATS.end_data_ns.store(ns(data), Ordering::Relaxed);
        STATS.end_commit_ns.store(ns(commit), Ordering::Relaxed);
    }

    /// Verbucht einen ausdrücklichen WAL-Checkpoint.
    pub(crate) fn record_checkpoint(t: Instant, frames: u64) {
        STATS.checkpoint_frames.fetch_add(frames, Ordering::Relaxed);
        let ns = nanos(t);
        STATS.checkpoints.fetch_add(1, Ordering::Relaxed);
        STATS.checkpoint_ns.fetch_add(ns, Ordering::Relaxed);
        STATS.checkpoint_max_ns.fetch_max(ns, Ordering::Relaxed);
    }
}
