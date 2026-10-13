//! Speicherplatzwächter: Warnung bei 2 GB, automatische Pause unter 1 GB (CRAWL-124).
//!
//! Die Messung selbst steckt in `os_store::files::disk_space_status`. Dieses Modul legt fest, *wann* gemessen
//! wird und was daraus folgt:
//!
//! * **Start** ([`start_warning`]): unter 2 GiB frei meldet es den Text „Wenig freier Speicherplatz (x,x GB) –
//!   der Crawl wird bei < 1 GB automatisch pausiert“. Die Engine schreibt ihn zusätzlich als Ereignis
//!   (`disk_space_low`) in die Crawl-Datei; die Kommandozeile gibt ihn auf stderr aus, die Oberfläche fragt vor
//!   dem ersten Abruf.
//! * **Laufzeit:** Die Steuerschleife prüft alle [`CHECK_INTERVAL`] (5 s, auf der Zeit von tokio, also mit
//!   `start_paused` simulierbar). Unter 1 GiB pausiert der Crawl geordnet („Speicherplatz knapp“): Zustand
//!   `Paused`, Warteschlange und WAL vollständig geschrieben, `crawl_meta.pause_reason = disk_space`.
//!
//! Tests ersetzen die Messung über [`set_probe`] (prozessweit, nur für Tests gedacht).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use os_store::files::{disk_space_status, DiskLevel, DiskSpaceStatus};
use os_store::StoreError;

/// Abstand der Prüfungen während des Crawls (CRAWL-124).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Meldung bei automatischer Pause.
pub const PAUSE_MESSAGE: &str = "Speicherplatz knapp";

/// Wert von `crawl_meta.pause_reason` bei einer Pause wegen Speicherplatzmangel.
pub const PAUSE_REASON: &str = "disk_space";

/// Messfunktion (Pfad → freier Speicher).
pub type Probe = Arc<dyn Fn(&Path) -> Result<DiskSpaceStatus, StoreError> + Send + Sync>;

static PROBE: Mutex<Option<Probe>> = Mutex::new(None);

/// Ersetzt die Messung (nur Tests); `None` stellt die echte Messung wieder her. Wirkt auf den ganzen Prozess.
pub fn set_probe(probe: Option<Probe>) {
    *PROBE.lock().unwrap_or_else(|p| p.into_inner()) = probe;
}

/// Misst den freien Speicher auf dem Laufwerk von `path`.
pub fn status(path: &Path) -> Result<DiskSpaceStatus, StoreError> {
    let probe = PROBE.lock().unwrap_or_else(|p| p.into_inner()).clone();
    match probe {
        Some(p) => p(path),
        None => disk_space_status(path),
    }
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskCheck {
    /// Bewertung.
    pub level: DiskLevel,
    /// Freier Speicher in GiB.
    pub free_gib: f64,
}

/// Prüft den Speicher; ein Messfehler gilt als „genug“ (der Crawl soll nicht an der Messung scheitern).
pub fn check(path: &Path) -> DiskCheck {
    match status(path) {
        Ok(s) => DiskCheck {
            level: s.level,
            free_gib: s.free_gib(),
        },
        Err(_) => DiskCheck {
            level: DiskLevel::Ok,
            free_gib: f64::INFINITY,
        },
    }
}

fn gib(x: f64) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Warnung beim Crawlstart: `Some(Text)`, wenn weniger als 2 GiB frei sind.
pub fn start_warning(path: &Path) -> Option<String> {
    let s = status(path).ok()?;
    s.warn_at_start().then(|| {
        format!(
            "Wenig freier Speicherplatz ({} GB) – der Crawl wird bei < 1 GB automatisch pausiert",
            gib(s.free_gib())
        )
    })
}

/// Text des Ereignisses bei automatischer Pause.
pub fn pause_message(free_gib: f64) -> String {
    format!(
        "{PAUSE_MESSAGE} ({} GB frei) – Crawl pausiert",
        gib(free_gib)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn status_with(free_bytes: u64) -> DiskSpaceStatus {
        DiskSpaceStatus {
            checked: PathBuf::from("."),
            free_bytes,
            level: os_store::files::classify_free_space(free_bytes),
        }
    }

    // spec: CRAWL-124
    #[test]
    fn warning_below_two_gib_and_pause_below_one_gib() {
        const GIB: u64 = 1 << 30;
        set_probe(Some(Arc::new(|_| Ok(status_with(3 * GIB / 2)))));
        let w = start_warning(Path::new("x.ospider")).expect("Warnung unter 2 GiB");
        assert!(w.starts_with("Wenig freier Speicherplatz (1,5 GB)"), "{w}");
        assert_eq!(check(Path::new("x")).level, DiskLevel::Low);
        set_probe(Some(Arc::new(|_| Ok(status_with(GIB / 2)))));
        assert_eq!(check(Path::new("x")).level, DiskLevel::Critical);
        set_probe(Some(Arc::new(|_| Ok(status_with(10 * GIB)))));
        assert!(start_warning(Path::new("x")).is_none());
        assert_eq!(check(Path::new("x")).level, DiskLevel::Ok);
        set_probe(None);
    }
}
