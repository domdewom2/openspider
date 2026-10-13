//! Kollation `OS_TEXT` (M1-P00c, docs/plan/M1.md 3.9): ICU4X-Collator mit Root-Locale (`und`), Stärke
//! Secondary. Groß-/Kleinschreibung bleibt unberücksichtigt, „ä“ steht unmittelbar nach „a“; die Ordnung ist
//! unabhängig von der UI-Sprache.
//!
//! Die Kollation wird nur in Abfragen verwendet (`ORDER BY x COLLATE OS_TEXT, x`), nie in Schemaobjekten
//! (Index, View, Trigger, CHECK), damit fremde Werkzeuge die Datei ohne registrierte Kollation lesen können.
//! Gleichstände der Kollation (z. B. „A“ und „a“) löst der Aufrufer über einen zweiten, bytewise
//! sortierten Schlüssel (`x` selbst bzw. `str::cmp`).

use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{CollatorBorrowed, CollatorPreferences};
use rusqlite::Connection;

use crate::error::StoreError;

/// Name der Kollation in SQL (`COLLATE OS_TEXT`).
pub const OS_TEXT: &str = "OS_TEXT";

/// Der prozessweit geteilte Collator (Root-Locale, Stärke Secondary). `None`, falls die Initialisierung
/// scheitert (mit eingebetteten Daten nicht zu erwarten); dann gilt der Bytevergleich.
fn collator() -> Option<&'static CollatorBorrowed<'static>> {
    static COLLATOR: OnceLock<Option<CollatorBorrowed<'static>>> = OnceLock::new();
    COLLATOR
        .get_or_init(|| {
            let mut options = CollatorOptions::default();
            options.strength = Some(Strength::Secondary);
            CollatorBorrowed::try_new(CollatorPreferences::default(), options).ok()
        })
        .as_ref()
}

/// Registriert `OS_TEXT` auf der Verbindung. Idempotent: erneutes Registrieren ersetzt die Funktion.
pub fn ensure(conn: &Connection) -> Result<(), StoreError> {
    if collator().is_none() {
        return Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            Some("Kollation OS_TEXT: ICU4X-Collator (Root-Locale) nicht initialisierbar".into()),
        )));
    }
    conn.create_collation(OS_TEXT, compare)?;
    Ok(())
}

/// Vergleich nach `OS_TEXT` für die Rust-Seite (identisch zur SQL-Kollation). Ergibt für Zeichenketten, die
/// sich nur in der Groß-/Kleinschreibung unterscheiden, `Equal`; der Gleichstand wird wie in SQL über einen
/// bytewise zweiten Schlüssel aufgelöst, z. B. `compare(a, b).then_with(|| a.cmp(b))`.
pub fn compare(a: &str, b: &str) -> Ordering {
    match collator() {
        Some(c) => c.compare(a, b),
        None => a.cmp(b),
    }
}
