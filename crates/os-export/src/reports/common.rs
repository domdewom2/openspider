//! Gemeinsame Hilfen der Reports und des Issues-Exports.

use os_model::CrawlConfig;
use os_store::{CrawlFile, StoreError};

use crate::error::ExportError;
use crate::types::WriterOptions;

/// Schreiboptionen aus der Konfiguration (`export.utf8_bom`, `export.line_ending`, OUT-002).
pub(crate) fn writer_options(cfg: &CrawlConfig) -> WriterOptions {
    WriterOptions {
        utf8_bom: cfg.export.utf8_bom,
        line_ending: cfg.export.line_ending,
        ..WriterOptions::default()
    }
}

/// Wandelt Fehler der Datenbankschicht in `ExportError`.
pub(crate) fn db<T, E: Into<StoreError>>(r: Result<T, E>) -> Result<T, ExportError> {
    r.map_err(|e| ExportError::Store(e.into()))
}

/// Ergebnis einer `COUNT(*)`-Abfrage (oder einer anderen einzelnen Ganzzahl); `NULL` zählt als 0.
pub(crate) fn count(file: &CrawlFile, sql: &str) -> Result<u64, ExportError> {
    let n: Option<i64> = db(file.conn().query_row(sql, [], |r| r.get(0)))?;
    Ok(u64::try_from(n.unwrap_or(0)).unwrap_or(0))
}

/// Prozentwert auf zwei Nachkommastellen (kaufmännisch gerundet, ganzzahlig gerechnet); leere Basis → `None`
/// (Division durch 0 ergibt eine leere Zelle, OUT-080).
pub(crate) fn pct2(hits: u64, base: u64) -> Option<f64> {
    if base == 0 {
        return None;
    }
    let hundredths = (u128::from(hits) * 20_000 + u128::from(base)) / (2 * u128::from(base));
    Some(hundredths as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct2_rounds_to_two_places() {
        assert_eq!(pct2(1, 3), Some(33.33));
        assert_eq!(pct2(2, 3), Some(66.67));
        assert_eq!(pct2(3, 7), Some(42.86));
        assert_eq!(pct2(0, 5), Some(0.0));
        assert_eq!(pct2(5, 5), Some(100.0));
        assert_eq!(pct2(1, 0), None);
    }
}
