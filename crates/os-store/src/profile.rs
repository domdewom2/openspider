//! Speicherprofile (CRAWL-124): PRAGMAs der Crawl-Datei.
//!
//! STUB(M1-P07): P00a setzt WAL, `synchronous = NORMAL` und die Cache-Größen für „Ausgewogen“ und
//! „Minimaler Speicher“. P07 ergänzt „Maximale Geschwindigkeit“ (25 % des physischen RAM, höchstens
//! 4 GiB, K-16; `synchronous = OFF` während des Crawls) und den Diagnose-Zugriff auf die gesetzten Werte.

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

impl StorageProfile {
    /// Wert für `PRAGMA cache_size` (negativ = KiB).
    // STUB(M1-P07): MaxSpeed wie Balanced, bis P07 die RAM-Größe einbezieht.
    pub fn cache_size_kib(&self) -> i64 {
        match self {
            StorageProfile::Balanced | StorageProfile::MaxSpeed => -262_144,
            StorageProfile::MinMemory => -65_536,
        }
    }
}

/// Setzt die Verbindungs-PRAGMAs: WAL, `synchronous = NORMAL`, `foreign_keys = OFF` (Konsistenz sichert der
/// Writer), Cache nach Profil und Wartezeit bei gesperrter Datei.
pub fn apply(conn: &Connection, profile: StorageProfile) -> Result<(), StoreError> {
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", false)?;
    conn.pragma_update(None, "cache_size", profile.cache_size_kib())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}
