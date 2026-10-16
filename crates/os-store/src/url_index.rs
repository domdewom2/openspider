//! Sekundärindizes von `urls` mit zufälligen Schlüsseln und ihr Lebenszyklus (M1-N22, K-80; geändert durch M1-N27, K-83).
//!
//! Der Index über eine Textspalte, deren Werte zur Einfügereihenfolge nichts mit der Sortierung zu tun haben
//! (`title_1`, `meta_desc_1`, `h1_1`, `h2_1`, `hash_md5`), verteilt die Änderungen eines Batches auf beliebige Blätter:
//! 2 000 Seiten ändern 2 000 verschiedene Seiten je Index. Jede dieser Seiten wandert ins WAL und beim Checkpoint
//! einzeln in die Hauptdatei. Deshalb gilt:
//!
//! * Die Indizes **fehlen** in einem großen laufenden oder pausierten Crawl und nach einem Abbruch: Der Writer entfernt
//!   sie vor dem ersten Schreiben, sobald die Datei [`DROP_FROM_URLS`] URLs hat (mitten im Crawl, wenn er die Schwelle
//!   überschreitet; beim Fortsetzen sofort). Kleine Crawls behalten sie.
//! * Sie **entstehen nicht wieder**, weder beim regulären Ende noch über [`crate::link_index::ensure_if_idle`]
//!   (K-83): Kein M1-Leser sucht über diese Spalten (die Filter „Duplicate“ und „Exact Duplicates“ gruppieren in einem
//!   Lauf über `urls`, Inventur in `docs/ARCHITECTURE.md`). Der Aufbau kostete im Endbatch bei 1 Mio. URLs 52 bis 58 s.
//!   Eine kleine Datei behält die Indizes oder bekommt sie über [`ensure_if_idle`](crate::link_index::ensure_if_idle) zurück.
//! * [`ensure`] baut sie weiterhin auf, jeden sortiert (`CREATE INDEX` sortiert die Schlüssel und füllt den Baum in
//!   einem Zug). Ein Leser, der eine Suche über eine dieser Spalten braucht (M2), ruft es selbst auf.
//!
//! Ohne die Indizes bleibt jede Abfrage korrekt, kostet aber einen sequenziellen Lauf über `urls` statt einer Suche
//! (Inventur der Leser: `docs/ARCHITECTURE.md`, Abschnitt Crawl-Datei). Nicht betroffen sind `UNIQUE(url)` (Dublettenschutz
//! des Writers), `urls_change_seq` (Änderungsfeed) und die Indizes über Spalten mit wenigen Werten oder einem NULL-Anteil
//! (`status_code`, `state`, `content_class`, `crawl_depth`, `indexability`, `redirect_target_id`,
//! `canonical_target_id`): deren Änderungen liegen nahe beieinander.

use rusqlite::Connection;

use crate::error::StoreError;

/// Ein Index von `urls`: Name und Definition (wie in Migration 0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UrlIndex {
    /// Name des Index.
    pub name: &'static str,
    /// `CREATE INDEX IF NOT EXISTS …`.
    pub create_sql: &'static str,
}

/// Die Indizes, die während des Schreibens fehlen.
pub const WRITE_PHASE_DROPPED: &[UrlIndex] = &[
    UrlIndex {
        name: "urls_title1",
        create_sql: "CREATE INDEX IF NOT EXISTS urls_title1 ON urls(title_1)",
    },
    UrlIndex {
        name: "urls_md1",
        create_sql: "CREATE INDEX IF NOT EXISTS urls_md1 ON urls(meta_desc_1)",
    },
    UrlIndex {
        name: "urls_h1",
        create_sql: "CREATE INDEX IF NOT EXISTS urls_h1 ON urls(h1_1)",
    },
    UrlIndex {
        name: "urls_h2",
        create_sql: "CREATE INDEX IF NOT EXISTS urls_h2 ON urls(h2_1)",
    },
    UrlIndex {
        name: "urls_hash",
        create_sql: "CREATE INDEX IF NOT EXISTS urls_hash ON urls(hash_md5)",
    },
];

/// Leser-Threads des Vorwärmens ([`Prefetch`]).
const PREFETCH_THREADS: usize = 4;
/// Erst ab dieser größten Kennung lohnt das Vorwärmen (kleine Dateien liegen ohnehin im Cache des Betriebssystems).
const PREFETCH_MIN_ID: i64 = 100_000;

/// Ab dieser größten URL-Kennung entfernt der Writer die Indizes während des Schreibens. Darunter passen die Indizes
/// mit allen Blättern in den Seitencache (250 000 URLs: rund 125 MB von 256 MiB), ihre Pflege kostet nichts Messbares
/// (Kurzprobe 250 000 Seiten: Abschnitte 85 % bis 114 %). Von dieser Größe an werden sie nicht mehr aufgebaut (K-83).
pub const DROP_FROM_URLS: i64 = 250_000;

/// Schwelle des Writers (größte URL-Kennung) für das Entfernen der Indizes. Diagnoseschalter
/// `OS_STORE_DROP_URL_INDEXES` (N22): `1` = immer entfernen (Schwelle 0), `0` = nie, sonst [`DROP_FROM_URLS`].
pub fn drop_threshold() -> i64 {
    match std::env::var("OS_STORE_DROP_URL_INDEXES").ok().as_deref() {
        Some("1") => 0,
        Some("0") => i64::MAX,
        _ => DROP_FROM_URLS,
    }
}

/// Ob die Datei „groß“ im Sinn von [`drop_threshold`] ist (größte URL-Kennung mindestens so hoch wie die Schwelle):
/// Der Writer hat die Indizes entfernt oder entfernt sie mit dem ersten Schreiben, und sie werden nicht wieder aufgebaut
/// (K-83; [`crate::link_index::ensure_if_idle`]). Dieselbe Entscheidung wie die des Writers, nur ohne dessen Zustand.
pub fn is_large(conn: &Connection) -> Result<bool, StoreError> {
    let threshold = drop_threshold();
    if threshold == i64::MAX {
        return Ok(false);
    }
    let max: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
    Ok(max >= threshold)
}

/// Ob der Index in der Datei steht.
fn exists(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Ob alle Indizes aus [`WRITE_PHASE_DROPPED`] in der Datei stehen.
pub fn present(conn: &Connection) -> Result<bool, StoreError> {
    for index in WRITE_PHASE_DROPPED {
        if !exists(conn, index.name)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Entfernt die Indizes (`DROP INDEX IF EXISTS`); fehlende bleiben unberührt.
pub fn drop_indexes(conn: &Connection) -> Result<(), StoreError> {
    for index in WRITE_PHASE_DROPPED {
        conn.execute_batch(&format!("DROP INDEX IF EXISTS {}", index.name))?;
    }
    Ok(())
}

/// Wärmt den Dateicache des Betriebssystems für den Aufbau vor (N22). Seit K-83 nur noch für eine kleine Datei, der ausnahmsweise
/// ein Index fehlt: in einem großen Crawl wird nichts aufgebaut.
///
/// Die Seiten von `urls` liegen nach dem Crawl verstreut in der Datei (Zeilen wachsen beim Eintragen der Seitendaten,
/// die Blätter teilen sich und landen zwischen den Seiten von `links`). Jeder der fünf Aufbauten liest sie in einem
/// Lauf mit dem kleinen Cache des Indexaufbaus einzeln und synchron nach: bei 1 Mio. URLs und 6 GB Datei
/// rund 10 s je Index (Messung N22: 46 s für alle fünf). Mehrere Leser-Threads auf eigenen Leseverbindungen lesen die
/// Kennungsbereiche gleichzeitig (WAL: sie blockieren den Writer nicht und sehen denselben Stand), so dass die
/// Aufbauten aus dem Dateicache arbeiten. Fehler der Leser bleiben folgenlos: der Aufbau liest dann selbst.
pub struct Prefetch {
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Prefetch {
    /// Startet die Leser für die Datei `path`; `None`, wenn `urls` klein ist oder die Datei sich nicht öffnen lässt.
    pub fn start(path: &std::path::Path) -> Option<Self> {
        Self::start_with(path, PREFETCH_THREADS, PREFETCH_MIN_ID)
    }

    /// Wie [`start`](Self::start) mit eigener Thread-Zahl und Mindestgröße (Tests).
    pub fn start_with(path: &std::path::Path, threads: usize, min_id: i64) -> Option<Self> {
        let open = || -> Option<Connection> {
            let conn = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .ok()?;
            conn.busy_timeout(std::time::Duration::from_secs(5)).ok()?;
            conn.pragma_update(None, "cache_size", -1024).ok()?;
            Some(conn)
        };
        let max_id: i64 = open()?
            .query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))
            .ok()?;
        if max_id < min_id.max(1) {
            return None;
        }
        let threads = threads.max(1);
        let step = max_id / i64::try_from(threads).unwrap_or(1) + 1;
        let mut handles = Vec::new();
        for i in 0..threads {
            let lo = i64::try_from(i).unwrap_or(0) * step;
            let hi = lo + step;
            let Some(conn) = open() else {
                continue;
            };
            let handle = std::thread::Builder::new()
                .name("os-store-prefetch".to_string())
                .spawn(move || {
                    let t = std::time::Instant::now();
                    // Liest die Spalten der fünf Indizes: der Aufbau braucht dieselben Blätter.
                    let rows: Result<i64, _> = conn.query_row(
                        "SELECT COUNT(*) + COALESCE(MAX(LENGTH(title_1) + LENGTH(meta_desc_1) + LENGTH(h1_1) \
                         + LENGTH(h2_1) + LENGTH(hash_md5)), 0) * 0 FROM urls WHERE id >= ?1 AND id < ?2",
                        [lo, hi],
                        |r| r.get(0),
                    );
                    crate::sink::diag::record_prefetch(t, rows.unwrap_or(0).max(0) as u64);
                });
            if let Ok(h) = handle {
                handles.push(h);
            }
        }
        Some(Prefetch { threads: handles })
    }
}

impl Drop for Prefetch {
    fn drop(&mut self) {
        for h in self.threads.drain(..) {
            let _ = h.join();
        }
    }
}

/// Baut fehlende Indizes auf, jeden sortiert. Liefert die Zahl der gebauten Indizes. Braucht Schreibzugriff. Die
/// Dauer geht in die Messzähler ([`crate::sink::diag`], `url_index_build_ns`).
pub fn ensure(conn: &Connection) -> Result<usize, StoreError> {
    let mut missing = Vec::new();
    for index in WRITE_PHASE_DROPPED {
        if !exists(conn, index.name)? {
            missing.push(index);
        }
    }
    if missing.is_empty() {
        return Ok(0);
    }
    let t = std::time::Instant::now();
    crate::link_index::with_build_settings(conn, || {
        for index in &missing {
            let one = std::time::Instant::now();
            let built = conn.execute_batch(index.create_sql);
            crate::sink::diag::record_url_index_one(one);
            built?;
        }
        Ok(())
    })?;
    crate::sink::diag::record_url_index(t, missing.len() as u64);
    Ok(missing.len())
}
