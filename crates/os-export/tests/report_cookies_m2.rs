#[path = "bulk_web_support.rs"]
mod support;
use os_export::{save_report, Format};
use support::*;

// spec: OUT-111
// spec: CRAWL-062
#[test]
fn cookie_summary_groups_by_the_seven_attributes_and_counts_distinct_urls() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    let result = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    let text = std::fs::read_to_string(&result.files[0].path).unwrap();
    golden("cookie-summary.csv", &text);
    let rows = records(&text);
    let shared = rows.iter().find(|r| r[0] == "shared").unwrap();
    assert_eq!(
        shared,
        &[
            "shared",
            ".site.test",
            "/",
            "1",
            "True",
            "True",
            "Lax",
            "HTTP",
            "True",
            "12",
            "https://www.site.test/01"
        ]
    );
    assert_eq!(rows.iter().filter(|r| r[0] == "third").count(), 2);
    assert!(rows
        .iter()
        .filter(|r| r[0] == "third")
        .all(|r| r[8] == "False"));
    assert_eq!(rows.iter().find(|r| r[0] == "expires").unwrap()[3], "1");
    assert_eq!(
        rows.iter().find(|r| r[0] == "session").unwrap()[3],
        "Session"
    );
    // Each attribute in the specified identity produces a separate group.
    for (position, assignment) in [
        (10, "name='other'"),
        (11, "domain='other.test'"),
        (12, "path='/other'"),
        (13, "secure=0"),
        (14, "http_only=0"),
        (15, "same_site='Strict'"),
        (16, "origin='javascript'"),
    ] {
        file.conn().execute(&format!("INSERT INTO cookies SELECT url_id,{position},origin,name,value,domain,path,expires,max_age,secure,http_only,same_site,raw FROM cookies WHERE url_id=1 AND position=0"),[]).unwrap();
        file.conn()
            .execute(
                &format!("UPDATE cookies SET {assignment} WHERE url_id=1 AND position={position}"),
                [],
            )
            .unwrap();
    }
    let changed = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(changed.files[0].rows, result.files[0].rows + 7);
}

// spec: OUT-112 [M2]
#[test]
fn cookie_report_availability_and_cli_warning_explain_missing_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.extraction.cookies = false;
    let entry = &os_export::reports::cookies::entries()[0];
    let state = os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg);
    assert!(!state.enabled);
    assert_eq!(
        state.reason.as_deref(),
        Some("report 'Cookie Summary' requires cookie extraction")
    );
    let result = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(
        result.warnings,
        vec!["report 'Cookie Summary' requires cookie extraction"]
    );
    assert_eq!(result.files[0].rows, 0);
    cfg.extraction.cookies = true;
    assert!(
        os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg).enabled
    );
}

// spec: OUT-111
#[test]
fn cookie_summary_uses_legacy_expiries_for_persistence_in_days() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let cases = cookie_dates(&file);
    let result = save_report(&file, &config(), "Cookie Summary", dir.path(), Format::Csv).unwrap();
    let rows = records(&std::fs::read_to_string(&result.files[0].path).unwrap());
    let at = os_model::Timestamp::from_db("2030-01-01T00:00:00.000Z").unwrap();
    for (i, (_, expected)) in cases.iter().enumerate() {
        let row = rows.iter().find(|r| r[0] == format!("date-{i}")).unwrap();
        if let Some(expected) = expected {
            let expires = time::OffsetDateTime::parse(
                expected,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap();
            let days = (expires - at.0).as_seconds_f64().max(0.0) / 86400.0;
            assert!((row[3].parse::<f64>().unwrap() - days).abs() < 1e-9);
        } else {
            assert_eq!(row[3], "Session");
        }
    }
}

// spec: OUT-010
// spec: OUT-111
#[test]
fn cancellation_during_cookie_aggregation_stops_before_any_output_row() {
    use os_export::{ExportError, ExportFileInfo, TableWriter, WriterOptions};
    use os_model::CancelToken;
    struct CancelAfter<'a> {
        writer: &'a mut dyn TableWriter,
        cancel: &'a CancelToken,
        checks: &'a std::cell::Cell<u64>,
        rows: &'a std::cell::Cell<u64>,
    }
    impl TableWriter for CancelAfter<'_> {
        fn checkpoint(&mut self) -> Result<(), ExportError> {
            self.checks.set(self.checks.get() + 1);
            if self.checks.get() == 5 {
                self.cancel.cancel();
            }
            self.writer.checkpoint()
        }
        fn header(&mut self, columns: &[String]) -> Result<(), ExportError> {
            self.writer.header(columns)
        }
        fn row(
            &mut self,
            kinds: &[os_analyze::ColumnKind],
            values: &[os_model::Value],
        ) -> Result<(), ExportError> {
            self.rows.set(self.rows.get() + 1);
            self.writer.row(kinds, values)
        }
        fn finish(self: Box<Self>) -> Result<ExportFileInfo, ExportError> {
            unreachable!()
        }
    }
    for existing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let file = file(dir.path());
        let cfg = config();
        let cancel = CancelToken::new();
        let checks = std::cell::Cell::new(0);
        let rows = std::cell::Cell::new(0);
        let mut ctx = os_export::types::ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: false,
            workbook: None,
            skipped: Vec::new(),
        };
        let path = dir.path().join("cookies.csv");
        if existing {
            std::fs::write(&path, b"previous complete summary").unwrap();
        }
        let entry = &os_export::reports::cookies::entries()[0];
        let job = os_export::catalog::TableJob {
            entry_name: entry.name,
            tabs: &[],
            file: &file,
            cfg: &cfg,
        };
        let started = std::time::Instant::now();
        let error = os_export::output::write_table(
            &path,
            Format::Csv,
            entry.name,
            &WriterOptions::default(),
            &mut ctx,
            |writer| {
                writer.header(
                    &entry
                        .header
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                )?;
                (entry.run)(
                    &job,
                    &mut CancelAfter {
                        writer,
                        cancel: &cancel,
                        checks: &checks,
                        rows: &rows,
                    },
                )?;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("abgebrochen"));
        assert_eq!(checks.get(), 5);
        assert_eq!(rows.get(), 0);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        if existing {
            assert_eq!(std::fs::read(&path).unwrap(), b"previous complete summary");
        } else {
            assert!(!path.exists());
        }
        assert!(std::fs::read_dir(dir.path()).unwrap().all(|e| {
            e.unwrap()
                .path()
                .extension()
                .is_none_or(|ext| ext != "part")
        }));
    }
}
