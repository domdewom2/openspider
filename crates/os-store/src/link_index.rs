//! Der Index `links_target (target_id, link_type)` und sein Lebenszyklus (K-74).
//!
//! Gleichverteilte Linkziele machen die Pflege dieses Index zur Hauptursache des Durchsatzeinbruchs großer
//! Crawls: Jede Seite berührt 30 verschiedene Stellen eines Index, der bei 1 Mio. Seiten nicht mehr in den Cache
//! passt. Deshalb gilt:
//!
//! * Der Index **fehlt** während eines laufenden oder pausierten Crawls und nach einem Abbruch. Der Writer
//!   entfernt ihn, wenn er eine Crawl-Datei zum Schreiben öffnet ([`crate::SqliteSink::open`]).
//! * Beim **regulären Ende** (Zustand `Completed` im letzten Batch: Ende, Stopp und Limit; nicht bei Pause oder
//!   Fehler) baut der Writer ihn in der Transaktion dieses Batches sortiert auf (`CREATE INDEX` sortiert die
//!   Schlüssel und füllt den Baum in einem Zug). Das Crawl-Ende und der Analyse-Hook verschieben sich um diese Zeit
//!   (Vorprüfung: 37 s bei 30 Mio. Links).
//! * [`ensure`] baut ihn bei Bedarf nach; die Crawl-Analyse und die Exporte einer nicht laufenden Datei rufen
//!   [`ensure_if_idle`] auf.
//!
//! Ohne den Index bleibt jede Abfrage nach `links.target_id` korrekt, kostet aber einen sequenziellen Lauf über
//! `links` (nie einen je Ergebniszeile); die Zähler je URL stehen immer in `url_counters`
//! (Tabelle in `docs/ARCHITECTURE.md`, Abschnitt Crawl-Datei).

use os_model::time::SystemClock;
use os_model::CrawlState;
use rusqlite::Connection;

use crate::error::StoreError;
use crate::open::CrawlFile;

/// Name des Index.
pub const INDEX_NAME: &str = "links_target";

/// Definition wie in Migration 0001.
const CREATE_SQL: &str = "CREATE INDEX IF NOT EXISTS links_target ON links(target_id, link_type)";

/// Cache-Größe (KiB) während des Indexaufbaus. Der Sortierer von `CREATE INDEX` hält bis zur Größe des Seitencaches
/// Schlüssel im Speicher, bevor er auf die Platte ausweicht; bei 256 MiB Cache (Profil Balanced) kostete das
/// zusätzlich rund 270 MiB Spitzen-RSS (Messung M1-P32, Schritt 1, PERF-003 verlangt unter 500 MB). Mit 32 MiB
/// bleibt der Aufbau schnell (der Baum wird sortiert gefüllt) und der Spitzen-RSS beim Stand des Crawls.
const BUILD_CACHE_KIB: i64 = 32 * 1024;

/// Ob der Index in der Datei steht.
pub fn present(conn: &Connection) -> Result<bool, StoreError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [INDEX_NAME],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Entfernt den Index (`DROP INDEX IF EXISTS`); ohne Index passiert nichts.
pub fn drop_index(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch("DROP INDEX IF EXISTS links_target")?;
    Ok(())
}

/// Baut den Index auf, wenn er fehlt. `true`, wenn er gebaut wurde. Braucht Schreibzugriff; dauert bei vielen
/// Links (30 Mio.) rund eine halbe Minute. Die Dauer geht in die Messzähler ([`crate::sink::diag`]).
pub fn ensure(conn: &Connection) -> Result<bool, StoreError> {
    if present(conn)? {
        return Ok(false);
    }
    let t = std::time::Instant::now();
    let cache: i64 = conn.pragma_query_value(None, "cache_size", |r| r.get(0))?;
    let shrink = cache >= 0 || -cache > BUILD_CACHE_KIB;
    if shrink {
        conn.pragma_update(None, "cache_size", -BUILD_CACHE_KIB)?;
    }
    let built = conn.execute_batch(CREATE_SQL);
    if shrink {
        conn.pragma_update(None, "cache_size", cache)?;
    }
    built?;
    crate::sink::diag::record_link_index(t);
    Ok(true)
}

/// Wie [`ensure`], aber nur für eine Datei mit Schreibzugriff, die nicht läuft (gespeicherter Zustand nicht
/// `Running`, oder ein `Running` mit ausgebliebenem Heartbeat). Ein laufender Crawl baut den Index selbst am
/// Ende; eine gerade gesperrte Datei (`SQLITE_BUSY`) bleibt unverändert und liefert `false`.
pub fn ensure_if_idle(file: &CrawlFile) -> Result<bool, StoreError> {
    let conn = file.conn();
    if conn.is_readonly("main")? || present(conn)? {
        return Ok(false);
    }
    if file.effective_state(&SystemClock)? == CrawlState::Running {
        return Ok(false);
    }
    match ensure(conn) {
        Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(f, _)))
            if matches!(
                f.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            Ok(false)
        }
        other => other,
    }
}
