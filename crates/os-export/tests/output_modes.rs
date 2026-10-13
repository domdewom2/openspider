//! Tests für `output::prepare` (OUT-013) und `open_writer`.

use std::fs;
use std::str::FromStr;
use std::time::Duration;

use os_export::output::{open_writer, prepare, prepare_with_offset, OutputMode};
use os_export::{ExportError, Format, WriterOptions};
use os_model::{FixedClock, ManualClock, Timestamp};
use time::UtcOffset;

fn start() -> Timestamp {
    Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap()
}

// spec: OUT-013
#[test]
fn overwrite_replaces_only_same_named_files() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("exports");
    let clock = FixedClock::new(start(), Duration::ZERO);
    // Zielordner existiert nicht -> wird angelegt
    let dir = prepare(&base, OutputMode::Overwrite, &clock).unwrap();
    assert_eq!(dir, base);
    assert!(base.is_dir());

    fs::write(base.join("internal_all.csv"), "alt").unwrap();
    fs::write(base.join("notiz.txt"), "bleibt").unwrap();
    let dir = prepare(&base, OutputMode::Overwrite, &clock).unwrap();
    assert_eq!(dir, base);
    // nach prepare ist noch nichts angefasst
    assert_eq!(
        fs::read_to_string(base.join("internal_all.csv")).unwrap(),
        "alt"
    );

    // der Schreiber ersetzt die gleichnamige Datei, andere bleiben
    let opts = WriterOptions {
        utf8_bom: false,
        ..WriterOptions::default()
    };
    let mut w = open_writer(
        &dir.join("internal_all.csv"),
        Format::Csv,
        "Internal",
        &opts,
    )
    .unwrap();
    w.header(&["Address".to_string()]).unwrap();
    w.finish().unwrap();
    assert_eq!(
        fs::read_to_string(base.join("internal_all.csv")).unwrap(),
        "Address\r\n"
    );
    assert_eq!(
        fs::read_to_string(base.join("notiz.txt")).unwrap(),
        "bleibt"
    );
}

// spec: OUT-013
#[test]
fn timestamped_creates_distinct_folders_with_suffix_on_collision() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("runs");
    let plus2 = UtcOffset::from_hms(2, 0, 0).unwrap();
    let clock = FixedClock::new(start(), Duration::ZERO);

    let a = prepare_with_offset(&base, OutputMode::Timestamped, &clock, plus2).unwrap();
    assert_eq!(a, base.join("2026-10-13_09-35-00"));
    assert!(a.is_dir());

    // gleiche Sekunde -> _2, _3
    let b = prepare_with_offset(&base, OutputMode::Timestamped, &clock, plus2).unwrap();
    assert_eq!(b, base.join("2026-10-13_09-35-00_2"));
    let c = prepare_with_offset(&base, OutputMode::Timestamped, &clock, plus2).unwrap();
    assert_eq!(c, base.join("2026-10-13_09-35-00_3"));

    // andere Sekunde -> anderer Ordner ohne Suffix (zwei Läufe in derselben Minute)
    let manual = ManualClock::new(start());
    manual.advance(Duration::from_secs(7));
    let d = prepare_with_offset(&base, OutputMode::Timestamped, &manual, plus2).unwrap();
    assert_eq!(d, base.join("2026-10-13_09-35-07"));

    // lokale Zeitzone wird über den Offset der Uhrzeit angewendet (Datumswechsel)
    let late = FixedClock::new(
        Timestamp::from_db("2026-10-13T22:30:00Z").unwrap(),
        Duration::ZERO,
    );
    let e = prepare_with_offset(&base, OutputMode::Timestamped, &late, plus2).unwrap();
    assert_eq!(e, base.join("2026-10-14_00-30-00"));
}

// spec: OUT-013
#[test]
fn timestamped_with_local_time_has_expected_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FixedClock::new(start(), Duration::ZERO);
    let dir = prepare(tmp.path(), OutputMode::Timestamped, &clock).unwrap();
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    let expected: String = start()
        .to_export()
        .chars()
        .take(19)
        .map(|c| match c {
            'T' => '_',
            ':' => '-',
            o => o,
        })
        .collect();
    assert_eq!(name, expected);
    assert_eq!(name.len(), 19);
}

// spec: OUT-013
#[test]
fn require_empty_fails_for_non_empty_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = FixedClock::new(start(), Duration::ZERO);

    // nicht existent -> wird angelegt
    let fresh = tmp.path().join("neu");
    assert_eq!(
        prepare(&fresh, OutputMode::RequireEmpty, &clock).unwrap(),
        fresh
    );
    assert!(fresh.is_dir());
    // leer -> ok
    assert_eq!(
        prepare(&fresh, OutputMode::RequireEmpty, &clock).unwrap(),
        fresh
    );

    // nicht leer -> Fehler mit verständlicher Meldung, nichts verändert
    fs::write(fresh.join("x.csv"), "x").unwrap();
    let err = prepare(&fresh, OutputMode::RequireEmpty, &clock).unwrap_err();
    match err {
        ExportError::Output(msg) => assert!(msg.contains("nicht leer"), "{msg}"),
        other => panic!("falscher Fehler: {other:?}"),
    }
    assert_eq!(fs::read_to_string(fresh.join("x.csv")).unwrap(), "x");

    // ein Unterordner zählt auch als Inhalt
    let with_dir = tmp.path().join("mit_ordner");
    fs::create_dir_all(with_dir.join("unter")).unwrap();
    assert!(matches!(
        prepare(&with_dir, OutputMode::RequireEmpty, &clock),
        Err(ExportError::Output(_))
    ));
}

// spec: OUT-013
#[test]
fn not_creatable_target_is_an_error_in_every_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("datei");
    fs::write(&blocker, "ich bin eine Datei").unwrap();
    let clock = FixedClock::new(start(), Duration::ZERO);
    for mode in [
        OutputMode::Overwrite,
        OutputMode::Timestamped,
        OutputMode::RequireEmpty,
    ] {
        // unter einer Datei lässt sich kein Ordner anlegen
        let err = prepare(&blocker.join("unter"), mode, &clock).unwrap_err();
        assert!(matches!(err, ExportError::Output(_)), "{mode:?}: {err:?}");
        // eine Datei als Zielordner ist ebenfalls ungeeignet
        let err = prepare(&blocker, mode, &clock).unwrap_err();
        assert!(matches!(err, ExportError::Output(_)), "{mode:?}: {err:?}");
    }
}

// spec: OUT-013
#[test]
fn unsupported_formats_are_reported_for_exit_code_2() {
    assert_eq!(Format::from_str("csv").unwrap(), Format::Csv);
    assert_eq!(Format::from_str("XLSX").unwrap(), Format::Xlsx);
    for f in ["xls", "gsheet"] {
        assert!(matches!(
            Format::from_str(f),
            Err(ExportError::Unsupported(_))
        ));
    }
}
