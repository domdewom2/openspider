//! Bericht `target/loadtest/report.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use crate::scenario::Size;

/// Schema: `{scenario, size, started_at, metrics{...}, passed}`.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Szenarioname, z. B. `perf-001`.
    pub scenario: String,
    /// `small` oder `large`.
    pub size: Size,
    /// Startzeit (UTC, RFC 3339).
    pub started_at: String,
    /// Messwerte; Schlüssel ohne Präfix gehören zum Testserver, `crawler_*` zum Crawler.
    pub metrics: BTreeMap<String, Value>,
    /// Alle Kriterien erfüllt?
    pub passed: bool,
}

impl Report {
    /// Schreibt den Bericht (legt das Verzeichnis an) als formatiertes JSON.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json + "\n")
    }
}

/// Standardpfad `<target-dir>/loadtest/report.json`.
pub fn default_report_path() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join("loadtest").join("report.json")
}

/// Aktuelle Zeit als RFC 3339 (UTC, Sekunden).
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339_utc(secs)
}

/// Formatiert Unix-Sekunden als `YYYY-MM-DDTHH:MM:SSZ` (Algorithmus nach H. Hinnant).
pub fn rfc3339_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(rfc3339_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }
}
