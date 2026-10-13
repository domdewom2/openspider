//! `os-store` – Crawl-Datei (SQLite, WAL), Schema-Migrationen und Abfragen für UI-Tabellen
//! (Paging, Filter, Sortierung).

use rusqlite::Connection;

/// Öffnet eine flüchtige In-Memory-Datenbank (Gerüst; echte Crawl-Dateien folgen).
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    Connection::open_in_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_is_usable() {
        let conn = open_in_memory().unwrap();
        let one: i64 = conn.query_row("SELECT 1", [], |row| row.get(0)).unwrap();
        assert_eq!(one, 1);
    }
}
