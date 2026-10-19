//! Restzeit-Schätzung der Fortschrittsanzeige (INTEG-142).

use std::time::Duration;

/// Restzeit bei Quelle Remote: `max(offen ÷ (Anfragen je Fenster ÷ Fensterlänge); offen × mittlere Antwortzeit ÷
/// gleichzeitige Anfragen)`.
pub fn remote(
    open: u64,
    requests_per_window: u32,
    window_s: u32,
    mean_response: Duration,
    concurrency: u32,
) -> Duration {
    let rate = f64::from(requests_per_window.max(1)) / f64::from(window_s.max(1));
    let by_rate = open as f64 / rate;
    Duration::from_secs_f64(by_rate.max(by_response(open, mean_response, concurrency)))
}

/// Restzeit bei Quelle Lokal: nur gemessene Laufzeit je URL und Parallelität (kein Tageslimit).
pub fn local(open: u64, mean_run: Duration, parallel: u32) -> Duration {
    Duration::from_secs_f64(by_response(open, mean_run, parallel))
}

fn by_response(open: u64, mean: Duration, concurrency: u32) -> f64 {
    open as f64 * mean.as_secs_f64() / f64::from(concurrency.max(1))
}

/// Anzeige „ca. 1 h 23 min“ (Minuten abgerundet; unter einer Minute „ca. 1 min“).
pub fn format(d: Duration) -> String {
    let minutes = (d.as_secs() / 60).max(1);
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("ca. {m} min"),
        (h, 0) => format!("ca. {h} h"),
        (h, m) => format!("ca. {h} h {m} min"),
    }
}
