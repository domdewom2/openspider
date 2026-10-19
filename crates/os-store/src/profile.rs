//! Speicherprofile (CRAWL-124): PRAGMAs der Crawl-Datei.
//!
//! | Profil | `cache_size` | `synchronous` während des Crawls | danach |
//! |---|---|---|---|
//! | Ausgewogen | −262144 (256 MiB) | `NORMAL` | `NORMAL` |
//! | Minimaler Speicher | −65536 (64 MiB) | `NORMAL` | `NORMAL` |
//! | Maximale Geschwindigkeit | −(25 % des physischen RAM in KiB, höchstens 4 GiB; K-16, an PERF-001 angeglichen) | `OFF` | `NORMAL` |
//!
//! Alle Profile laufen im WAL-Modus; das Profil ändert keine Funktion, nur Cache und Synchronisation.
//!
//! **WAL-Checkpoint (M1-P30):** SQLite führt das WAL standardmäßig nach 1 000 Seiten (4 MiB) in die Hauptdatei
//! zurück und synchronisiert dabei beide Dateien. Beim Crawl mit gleichverteilten Zielen schreibt jeder dieser
//! Checkpoints einen großen Teil der Datei neu; unter Windows (`FlushFileBuffers`, Echtzeitscan des Virenscanners)
//! kostete er rund 0,5–1,5 s und machte 94 % der Writer-Zeit aus (CI-Messung: 54 statt 2 233 URLs/s bei
//! macOS-arm64). Der Schwellwert wächst deshalb mit der Datei ([`wal_checkpoint_pages`]): Der Aufwand je Checkpoint
//! wächst mit der Dateigröße, die Zahl der Checkpoints sinkt im gleichen Maß, der Gesamtaufwand bleibt linear.
//! [`read`] liefert die tatsächlich gesetzten Werte (Hilfe › Diagnose, Test der Akzeptanzkriterien).
//!
//! **Hintergrund-Checkpoint (M1-P32, K-76):** Für den schreibenden Crawl (`SqliteSink`) gilt das nicht mehr: Die
//! Writer-Verbindung stellt `wal_autocheckpoint` auf 0, ein eigener Thread führt `PASSIVE`-Checkpoints aus, sobald das
//! WAL die Schwelle erreicht ([`wal_checkpoint_pages`] bestimmt sie weiter nach der Dateigröße, mit den Grenzen von
//! `with_checkpoint_bounds`). Der Writer wartet nie auf einen Checkpoint (siehe `checkpointer`). Die hier gesetzte
//! Schwelle gilt nur noch für sonstige Verbindungen mit Schreibzugriff (Analyse, Reparatur).

use rusqlite::Connection;

use crate::error::StoreError;

/// Leistungsprofil (CRAWL-124).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageProfile {
    /// Standard: 256 MiB SQLite-Cache.
    #[default]
    Balanced,
    /// Großer Cache, weniger Synchronisation.
    MaxSpeed,
    /// 64 MiB SQLite-Cache.
    MinMemory,
}

/// Obergrenze des Caches im Profil „Maximale Geschwindigkeit“: 4 GiB in KiB (K-16, PERF-001).
pub const MAX_SPEED_CACHE_CAP_KIB: i64 = 4 * 1024 * 1024;

/// Untergrenze des automatischen WAL-Checkpoints in Seiten: 65 536 Seiten (256 MiB bei 4 KiB Seitengröße). Ein
/// Crawl bis zu dieser WAL-Größe (rund 10 000 Seiten mit 30 Links in vier Gruppen-Commits) wird erst am Ende
/// zurückgeführt: ein Checkpoint mitten im Crawl schreibt und synchronisiert die Hauptdatei ein zweites Mal, und der
/// Echtzeitscan unter Windows prüft die Datei danach erneut (CI-Messung: 7,5 s zusätzlich bei 59 MB).
pub const WAL_CHECKPOINT_MIN_PAGES: i64 = 65_536;

/// Obergrenze des automatischen WAL-Checkpoints in Seiten: 131 072 Seiten (512 MiB bei 4 KiB Seitengröße). Das
/// WAL kann sich bis dahin füllen, ehe es zurückgeführt wird; größere Werte kosten Plattenplatz und
/// Wiederherstellungszeit nach einem Abbruch (das WAL wird beim Öffnen wiedereingespielt).
pub const WAL_CHECKPOINT_MAX_PAGES: i64 = 131_072;

/// Schwelle für `PRAGMA wal_autocheckpoint` bei einer Datei mit `db_pages` Seiten: die Dateigröße, begrenzt auf
/// [`WAL_CHECKPOINT_MIN_PAGES`] bis [`WAL_CHECKPOINT_MAX_PAGES`].
pub fn wal_checkpoint_pages(db_pages: i64) -> i64 {
    wal_checkpoint_pages_between(db_pages, WAL_CHECKPOINT_MIN_PAGES, WAL_CHECKPOINT_MAX_PAGES)
}

/// Wie [`wal_checkpoint_pages`] mit eigenen Grenzen (Tests).
pub fn wal_checkpoint_pages_between(db_pages: i64, min_pages: i64, max_pages: i64) -> i64 {
    db_pages.clamp(min_pages, max_pages.max(min_pages))
}

/// Untergrenze des Caches im Profil „Maximale Geschwindigkeit“: der Wert von „Ausgewogen“ (nur relevant auf
/// Rechnern mit weniger als 1 GiB RAM, damit das schnelle Profil nie kleiner ausfällt als das Standardprofil).
const BALANCED_CACHE_KIB: i64 = 262_144;
const MIN_MEMORY_CACHE_KIB: i64 = 65_536;

impl StorageProfile {
    /// Wert für `PRAGMA cache_size` (negativ = KiB); „Maximale Geschwindigkeit“ nutzt den physischen RAM
    /// dieses Rechners.
    pub fn cache_size_kib(&self) -> i64 {
        self.cache_size_kib_for_ram(physical_ram_bytes())
    }

    /// Wie [`cache_size_kib`](Self::cache_size_kib) für einen vorgegebenen physischen RAM (Bytes; `None` =
    /// unbekannt → Wert von „Ausgewogen“).
    pub fn cache_size_kib_for_ram(&self, ram_bytes: Option<u64>) -> i64 {
        match self {
            StorageProfile::Balanced => -BALANCED_CACHE_KIB,
            StorageProfile::MinMemory => -MIN_MEMORY_CACHE_KIB,
            StorageProfile::MaxSpeed => match ram_bytes {
                Some(bytes) => {
                    let quarter_kib = i64::try_from(bytes / 4 / 1024).unwrap_or(i64::MAX);
                    -quarter_kib.clamp(BALANCED_CACHE_KIB, MAX_SPEED_CACHE_CAP_KIB)
                }
                None => -BALANCED_CACHE_KIB,
            },
        }
    }

    /// Wert für `PRAGMA synchronous` während des Crawls.
    pub fn synchronous_during_crawl(&self) -> &'static str {
        match self {
            StorageProfile::MaxSpeed => "OFF",
            StorageProfile::Balanced | StorageProfile::MinMemory => "NORMAL",
        }
    }

    /// Wert für `PRAGMA synchronous` nach Abschluss des Crawls (alle Profile).
    pub fn synchronous_after_crawl(&self) -> &'static str {
        "NORMAL"
    }
}

/// Tatsächlich gesetzte PRAGMAs einer Verbindung (Diagnose).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedPragmas {
    /// `journal_mode` (`wal`).
    pub journal_mode: String,
    /// `synchronous`: 0 = OFF, 1 = NORMAL, 2 = FULL, 3 = EXTRA.
    pub synchronous: i64,
    /// `cache_size` (negativ = KiB).
    pub cache_size: i64,
    /// `foreign_keys`.
    pub foreign_keys: bool,
}

impl AppliedPragmas {
    /// Name des `synchronous`-Werts.
    pub fn synchronous_name(&self) -> &'static str {
        match self.synchronous {
            0 => "OFF",
            1 => "NORMAL",
            2 => "FULL",
            _ => "EXTRA",
        }
    }
}

/// Optionale app-weite Cache-Obergrenze. Null belässt die Profilgrenzen (OUT-267).
static CACHE_LIMIT_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn set_cache_limit(bytes: Option<u64>) {
    CACHE_LIMIT_BYTES.store(bytes.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
}
fn apply_cache_limit(conn: &Connection, profile: StorageProfile) -> Result<(), StoreError> {
    let bytes = CACHE_LIMIT_BYTES.load(std::sync::atomic::Ordering::Relaxed);
    if bytes > 0 {
        let kib = (bytes / 1024).max(1).min(i64::MAX as u64) as i64;
        conn.pragma_update(
            None,
            "cache_size",
            -kib.min(profile.cache_size_kib().saturating_abs()),
        )?;
    }
    Ok(())
}

/// Setzt die Verbindungs-PRAGMAs: WAL, `synchronous` nach Profil (während des Crawls), `foreign_keys = OFF`
/// (Konsistenz sichert der Writer), Cache nach Profil, Checkpoint-Schwelle ([`WAL_CHECKPOINT_MIN_PAGES`]) und
/// Wartezeit bei gesperrter Datei.
pub fn apply(conn: &Connection, profile: StorageProfile) -> Result<(), StoreError> {
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", profile.synchronous_during_crawl())?;
    conn.pragma_update(None, "foreign_keys", false)?;
    conn.pragma_update(None, "cache_size", profile.cache_size_kib())?;
    conn.pragma_update(None, "wal_autocheckpoint", WAL_CHECKPOINT_MIN_PAGES)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    apply_cache_limit(conn, profile)?;
    Ok(())
}

/// Stellt nach Abschluss des Crawls `synchronous` auf den Wert nach dem Crawl zurück (`NORMAL`).
pub fn settle(conn: &Connection, profile: StorageProfile) -> Result<(), StoreError> {
    conn.pragma_update(None, "synchronous", profile.synchronous_after_crawl())?;
    Ok(())
}

/// Liest die tatsächlich gesetzten PRAGMAs der Verbindung.
pub fn read(conn: &Connection) -> Result<AppliedPragmas, StoreError> {
    Ok(AppliedPragmas {
        journal_mode: conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?,
        synchronous: conn.pragma_query_value(None, "synchronous", |r| r.get(0))?,
        cache_size: conn.pragma_query_value(None, "cache_size", |r| r.get(0))?,
        foreign_keys: conn.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))? != 0,
    })
}

/// Physischer Arbeitsspeicher in Bytes; `None`, wenn er nicht ermittelbar ist.
#[cfg(unix)]
pub fn physical_ram_bytes() -> Option<u64> {
    // SAFETY: `sysconf` hat keine Vorbedingungen und liest nur Systemkonstanten.
    let (pages, page_size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    u64::try_from(pages)
        .ok()?
        .checked_mul(u64::try_from(page_size).ok()?)
}

/// Physischer Arbeitsspeicher in Bytes; `None`, wenn er nicht ermittelbar ist.
#[cfg(windows)]
pub fn physical_ram_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    // SAFETY: `status` ist vollständig initialisiert (Nullen) und `dwLength` gesetzt, wie die API verlangt.
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) == 0 {
            return None;
        }
        Some(status.ullTotalPhys)
    }
}

#[cfg(not(any(unix, windows)))]
pub fn physical_ram_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Infrastruktur (M1-P30), kein Spec-Tag.
    #[test]
    fn the_checkpoint_threshold_follows_the_file_size_within_its_bounds() {
        assert_eq!(wal_checkpoint_pages(0), WAL_CHECKPOINT_MIN_PAGES);
        assert_eq!(wal_checkpoint_pages(100_000), 100_000);
        assert_eq!(wal_checkpoint_pages(i64::MAX), WAL_CHECKPOINT_MAX_PAGES);
    }
}
