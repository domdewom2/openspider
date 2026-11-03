//! Eingedämmte Panics (OUT-246, Klärung `cc57ea03`): Parser für Fremddaten (PDF, hochgeladene Tabellen) fangen
//! einen Panic der Bibliothek gezielt ab und zeigen dem Nutzer eine saubere Fehlermeldung. Solche Panics sind kein
//! Absturz: Der Panic-Hook von App und CLI schreibt die Logzeile mit Backtrace (OUT-240), aber keinen Absturzbericht
//! und keinen Hinweis (OUT-247). Die Markierung ist thread-lokal und gilt nur, solange [`contained`] läuft; interne
//! Panics (Pools, Schreibwarteschlange, Laufzeit, Operationen) bleiben unmarkiert und erzeugen weiter Berichte.
//!
//! Liegt in `os-model`, weil `os-parse`, `os-engine`, `os-app` und `os-cli` dieses Crate bereits nutzen und kein
//! Kern-Crate von `os-app` abhängen darf (`docs/ARCHITECTURE.md`); nur Standardbibliothek.

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Setzt die Markierung zurück, auch wenn `f` mit einem Panic endet.
struct Mark;

impl Mark {
    fn enter() -> Mark {
        let _ = DEPTH.try_with(|d| d.set(d.get().saturating_add(1)));
        Mark
    }
}

impl Drop for Mark {
    fn drop(&mut self) {
        let _ = DEPTH.try_with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Führt `f` aus und fängt einen Panic darin ab (`Err` mit der Nutzlast). Während `f` läuft, gilt der Thread als
/// „eingedämmt“ ([`is_contained`]); verschachtelte Aufrufe sind erlaubt.
pub fn contained<T>(f: impl FnOnce() -> T) -> std::thread::Result<T> {
    let _mark = Mark::enter();
    catch_unwind(AssertUnwindSafe(f))
}

/// Läuft der aktuelle Thread gerade in [`contained`]? Für den Panic-Hook; panikt nie.
pub fn is_contained() -> bool {
    DEPTH.try_with(|d| d.get() > 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-246
    #[test]
    fn marks_only_while_running_and_survives_the_panic() {
        assert!(!is_contained());
        assert!(contained(is_contained).unwrap());
        // Der Panic wird abgefangen; die Markierung gilt bis dahin (den Hook prüfen die Tests in os-app/os-cli).
        let result = contained(|| -> u8 {
            assert!(is_contained());
            panic!("fremde Daten")
        });
        assert!(result.is_err());
        assert!(!is_contained(), "nach dem Abfangen zurückgesetzt");
        // verschachtelt
        contained(|| {
            let _ = contained(|| ());
            assert!(is_contained());
        })
        .unwrap();
        assert!(!is_contained());
        // anderer Thread: nicht markiert
        contained(|| assert!(!std::thread::spawn(is_contained).join().unwrap())).unwrap();
    }
}
