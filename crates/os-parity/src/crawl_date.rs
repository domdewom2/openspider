//! OUT-123: Seitenspezifische Normalisierung des ersatzweisen Crawl-Datums.

use os_model::Timestamp;

use crate::{csvio::Table, error::ParityError};

/// Beobachtetes UTC-Zeitfenster eines einzelnen Exporter-Aufrufs samt Crawl.
pub(crate) struct CrawlWindow {
    started: Timestamp,
    finished: Timestamp,
}

impl CrawlWindow {
    pub(crate) fn new(started: Timestamp, finished: Timestamp) -> Self {
        Self { started, finished }
    }

    fn date(&self) -> Result<String, ParityError> {
        let start = self.started.to_db()[..10].to_string();
        let end = self.finished.to_db()[..10].to_string();
        // Innerhalb dieses Fensters liegt jeder URL-Abruf. Über Mitternacht ist
        // ohne URL-genaue Metadaten keine eindeutige Normalisierung möglich.
        if start != end || self.finished < self.started {
            return Err(ParityError::Invalid(
                "Crawl-Datum nicht eindeutig: UTC-Tageswechsel oder rückläufige Uhr im Crawl"
                    .into(),
            ));
        }
        Ok(start)
    }
}

fn reference_date(exported_at: Option<&str>) -> Result<String, ParityError> {
    let value = exported_at.ok_or_else(|| {
        ParityError::Invalid("OUT-123: exported_at im Referenzmanifest fehlt".into())
    })?;
    let timestamp = if value.len() == 10 {
        format!("{value}T00:00:00Z")
    } else {
        value.to_string()
    };
    Timestamp::from_db(&timestamp)
        .map(|time| time.to_db()[..10].to_string())
        .map_err(|_| ParityError::Invalid("OUT-123: ungültiges exported_at".into()))
}

pub(crate) fn normalize(
    reference: &mut Table,
    os: &mut Table,
    exported_at: Option<&str>,
    window: &CrawlWindow,
) -> Result<(), ParityError> {
    // Andere Datumsspalten (z. B. Last Modified im Tabellenexport) sind keine
    // Ersatzwerte nach OUT-123 und bleiben vollständig im Vergleich.
    let has_lastmod = |table: &Table| {
        table
            .index_of("Lastmod")
            .is_some_and(|i| table.rows.iter().any(|row| !row[i].trim().is_empty()))
    };
    if !has_lastmod(reference) && !has_lastmod(os) {
        return Ok(());
    }
    let ref_date = reference_date(exported_at)?;
    let os_date = window.date()?;
    for (table, date) in [(reference, ref_date), (os, os_date)] {
        if let Some(index) = table.index_of("Lastmod") {
            for row in &mut table.rows {
                if row[index].trim() == date {
                    row[index] = "<CRAWLDATE>".into();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-123
    #[test]
    fn midnight_and_backwards_clock_fail_closed() {
        for (start, end) in [
            ("2026-10-30T23:59:59Z", "2026-10-31T00:00:01Z"),
            ("2026-10-30T12:00:01Z", "2026-10-30T12:00:00Z"),
        ] {
            assert!(CrawlWindow::new(
                Timestamp::from_db(start).unwrap(),
                Timestamp::from_db(end).unwrap(),
            )
            .date()
            .is_err());
        }
    }

    // spec: OUT-123
    #[test]
    fn reference_metadata_must_be_present_and_valid() {
        assert!(reference_date(None).is_err());
        assert!(reference_date(Some("2026-02-30")).is_err());
        assert_eq!(
            reference_date(Some("2026-10-30T23:00:00-02:00")).unwrap(),
            "2026-10-31"
        );
    }
}
