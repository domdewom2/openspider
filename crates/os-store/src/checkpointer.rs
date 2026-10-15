//! Checkpoint-Thread des Writers (K-76): Das WAL wird im Hintergrund in die Hauptdatei zurückgeführt.
//!
//! Bisher lief der automatische Checkpoint von SQLite im `COMMIT` des Writers (`wal_autocheckpoint`): Commit und
//! Checkpoint waren ein Aufruf, der Writer wartete auf den Checkpoint (N14: 31,4 s = Commit 20,4 s + Checkpoint 16,0 s).
//! Jetzt steht `wal_autocheckpoint` auf 0, und ein eigener Thread mit **eigener Verbindung** führt
//! `PRAGMA wal_checkpoint(PASSIVE)` aus. `PASSIVE` blockiert den Writer nicht: Er hängt weiter Frames an das WAL, während der
//! Thread die Seiten in die Hauptdatei kopiert. Beginnt der Writer eine Transaktion, wenn der Thread alle Frames
//! zurückgeführt hat, beginnt das WAL von vorn (SQLite setzt es zurück); `journal_size_limit` kürzt die Datei danach.
//!
//! **Auslöser** (Werte aus der Messung M1-P32, Begründung im Bericht `docs/plan/M1-acceptance/loadtest/probe-p32/`):
//! * Größe: das WAL hat mindestens [`Shared::trigger_pages`] Frames (Schwelle wächst mit der Datei,
//!   [`crate::profile::wal_checkpoint_pages_between`]), oder
//! * Zeit: seit dem letzten Durchlauf sind [`MAX_INTERVAL`] vergangen und das WAL ist größer als [`TIME_MIN_FRAMES`]
//!   (ein langsamer Crawl hält so die Wiederherstellung nach einem Abbruch kurz).
//!
//! Die Größe des WAL liest der Thread aus der Länge der `-wal`-Datei (kein Sperren, keine Transaktion). Fehler des
//! Threads stoppen den Crawl nicht: sie gehen in die Diagnose ([`Checkpointer::last_error`]); der abschließende
//! `TRUNCATE`-Checkpoint des Writers bleibt.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};

use crate::error::StoreError;
use crate::profile;

/// Abstand der Prüfungen des Threads.
const POLL: Duration = Duration::from_millis(100);
/// Längste Zeit zwischen zwei Durchläufen, wenn das WAL überhaupt etwas enthält.
pub const MAX_INTERVAL: Duration = Duration::from_secs(5);
/// Unterhalb dieser Frames lohnt kein zeitgesteuerter Durchlauf (4 MiB bei 4 KiB Seitengröße).
pub const TIME_MIN_FRAMES: i64 = 1_024;

/// Zustand, den Writer und Thread teilen.
struct Shared {
    stop: AtomicBool,
    /// Auslöser in Frames; der Thread lässt ihn mit der Datei wachsen.
    trigger_pages: AtomicI64,
    /// Grenzen des Auslösers in Seiten.
    bounds: (i64, i64),
    /// Zahl der abgeschlossenen Durchläufe (der Writer wartet bei zu großem WAL auf den nächsten).
    passes: Mutex<u64>,
    passes_changed: Condvar,
    wake: Mutex<()>,
    wake_changed: Condvar,
    error: Mutex<Option<String>>,
}

/// Der Hintergrund-Checkpoint eines [`crate::SqliteSink`].
pub(crate) struct Checkpointer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    wal_path: PathBuf,
    page_size: i64,
}

fn wal_path_of(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push("-wal");
    PathBuf::from(name)
}

impl Checkpointer {
    /// Startet den Thread für die Datei `path`. `synchronous` ist der Wert des Writers während des Crawls
    /// (`NORMAL`/`OFF`), `bounds` die Grenzen des Auslösers in Seiten.
    pub fn start(
        path: &Path,
        synchronous: &'static str,
        bounds: (i64, i64),
    ) -> Result<Self, StoreError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        crate::collation::ensure(&conn)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "synchronous", synchronous)?;
        conn.pragma_update(None, "wal_autocheckpoint", 0)?;
        let page_size: i64 = conn.pragma_query_value(None, "page_size", |r| r.get(0))?;
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            trigger_pages: AtomicI64::new(bounds.0),
            bounds,
            passes: Mutex::new(0),
            passes_changed: Condvar::new(),
            wake: Mutex::new(()),
            wake_changed: Condvar::new(),
            error: Mutex::new(None),
        });
        let wal_path = wal_path_of(path);
        let thread = {
            let shared = Arc::clone(&shared);
            let wal_path = wal_path.clone();
            std::thread::Builder::new()
                .name("os-store-checkpoint".to_string())
                .spawn(move || run(&conn, &shared, &wal_path, page_size))
                .map_err(|e| StoreError::io(path, e))?
        };
        Ok(Checkpointer {
            shared,
            thread: Some(thread),
            wal_path,
            page_size,
        })
    }

    /// Aktueller Auslöser in Frames.
    pub fn trigger_pages(&self) -> i64 {
        self.shared.trigger_pages.load(Ordering::Relaxed)
    }

    /// Frames im WAL laut Dateilänge (obere Schätzung nach einem Zurücksetzen, bis `journal_size_limit` greift).
    pub fn wal_frames(&self) -> i64 {
        wal_frames(&self.wal_path, self.page_size)
    }

    /// Fehler des Threads (erster), falls einer auftrat.
    pub fn last_error(&self) -> Option<String> {
        self.shared
            .error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Wartet höchstens `timeout`, bis der Thread einen weiteren Durchlauf beendet hat; `true`, wenn einer beendet wurde.
    pub fn wait_for_pass(&self, timeout: Duration) -> bool {
        let guard = self.shared.passes.lock().unwrap_or_else(|p| p.into_inner());
        let start = *guard;
        let (guard, _) = self
            .shared
            .passes_changed
            .wait_timeout_while(guard, timeout, |n| *n == start)
            .unwrap_or_else(|p| p.into_inner());
        *guard != start
    }

    /// Beendet den Thread und wartet auf ihn (ein laufender Durchlauf wird zu Ende geführt).
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        {
            let _g = self.shared.wake.lock().unwrap_or_else(|p| p.into_inner());
            self.shared.wake_changed.notify_all();
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Checkpointer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Frames laut Länge der `-wal`-Datei (Kopf 32 Byte, je Frame 24 Byte Kopf + Seite).
fn wal_frames(wal: &Path, page_size: i64) -> i64 {
    let len = std::fs::metadata(wal).map_or(0, |m| i64::try_from(m.len()).unwrap_or(i64::MAX));
    (len - 32).max(0) / (page_size + 24)
}

fn run(conn: &Connection, shared: &Shared, wal_path: &Path, page_size: i64) {
    let mut last_pass = Instant::now();
    loop {
        {
            let guard = shared.wake.lock().unwrap_or_else(|p| p.into_inner());
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }
            // Das Warten gibt die Sperre frei; `stop()` weckt den Thread sofort.
            drop(
                shared
                    .wake_changed
                    .wait_timeout(guard, POLL)
                    .unwrap_or_else(|p| p.into_inner()),
            );
        }
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        // Schwelle mit der Dateigröße wachsen lassen
        if let Ok(pages) = conn.pragma_query_value(None, "page_count", |r| r.get::<_, i64>(0)) {
            let (min_pages, max_pages) = shared.bounds;
            shared.trigger_pages.store(
                profile::wal_checkpoint_pages_between(pages, min_pages, max_pages),
                Ordering::Relaxed,
            );
        }
        let frames = wal_frames(wal_path, page_size);
        let by_size = frames >= shared.trigger_pages.load(Ordering::Relaxed);
        let by_time = frames >= TIME_MIN_FRAMES && last_pass.elapsed() >= MAX_INTERVAL;
        if !(by_size || by_time) {
            continue;
        }
        let t = Instant::now();
        // Antwort: (busy, Frames im WAL, davon zurückgeführt)
        let result = conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        });
        match result {
            Ok((_busy, log)) => {
                crate::sink::diag::record_background_checkpoint(t, u64::try_from(log).unwrap_or(0));
            }
            Err(e) => {
                crate::sink::diag::record_background_error();
                let mut slot = shared.error.lock().unwrap_or_else(|p| p.into_inner());
                if slot.is_none() {
                    *slot = Some(e.to_string());
                }
            }
        }
        last_pass = Instant::now();
        {
            let mut passes = shared.passes.lock().unwrap_or_else(|p| p.into_inner());
            *passes += 1;
            shared.passes_changed.notify_all();
        }
    }
}
