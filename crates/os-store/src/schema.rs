//! Schema, Dateikennung und Migrationsrahmen (OUT-230, CRAWL-127).
//!
//! Schemaänderungen kommen ausschließlich als neue Datei `migrations/000N_*.sql` plus Eintrag in
//! [`MIGRATIONS`]; bestehende Migrationen werden nie geändert.

use rusqlite::Connection;

use crate::error::StoreError;

/// `PRAGMA application_id` jeder Crawl-Datei: 0x4F535044 („OSPD“).
pub const APPLICATION_ID: i32 = 0x4F53_5044;

/// Eine Schemamigration.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// Zielversion (`user_version` nach der Migration); beginnt bei 1 und zählt lückenlos.
    pub version: u32,
    /// Kurzname.
    pub name: &'static str,
    /// SQL (mehrere Anweisungen), läuft in einer Transaktion.
    pub sql: &'static str,
}

/// Alle Migrationen in Reihenfolge.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "url_counters",
        sql: include_str!("../migrations/0002_url_counters.sql"),
    },
];

/// Aktuelle Schemaversion dieses Programmstands.
pub const SCHEMA_VERSION: u32 = 2;

/// Neueste Version einer Migrationsliste (0 bei leerer Liste).
pub fn latest_version(migrations: &[Migration]) -> u32 {
    migrations.last().map_or(0, |m| m.version)
}

/// Liest `PRAGMA user_version`.
pub fn user_version(conn: &Connection) -> Result<u32, StoreError> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    Ok(u32::try_from(v).unwrap_or(0))
}

/// Liest `PRAGMA application_id`.
pub fn application_id(conn: &Connection) -> Result<i32, StoreError> {
    Ok(conn.pragma_query_value(None, "application_id", |r| r.get(0))?)
}

/// Wendet alle Migrationen mit `version > from` an – je Migration eine Transaktion, in der auch
/// `user_version` gesetzt wird. Liefert die erreichte Version.
pub fn migrate(
    conn: &mut Connection,
    from: u32,
    migrations: &[Migration],
) -> Result<u32, StoreError> {
    for (expected, m) in (1u32..).zip(migrations) {
        if m.version != expected {
            return Err(StoreError::Migration(format!(
                "Version {} an Position {expected} (erwartet {expected})",
                m.version
            )));
        }
    }
    let mut version = from;
    for m in migrations.iter().filter(|m| m.version > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        version = m.version;
    }
    Ok(version)
}
