//! `os-parity` – Paritätsabgleich gegen Referenz-Exporte (`diff`, `run`) und Spec-Abdeckung aus Tags und
//! Testergebnissen (`coverage`), Abschnitt 5 des M1-Plans.
//!
//! Das Werkzeug kennt bewusst keine OpenSpider-Crates außer `os-testsite` (Start der Fixture-Sites für
//! `run`); `diff` und `coverage` arbeiten nur auf Dateien.
//!
//! - [`csvio`]: toleranter CSV-Leser (BOM, CRLF/LF, `,`/`;`, Verwerfen von Beschreibungsspalten)
//! - [`normalize`]: Basis-URL-Normalisierung und Zellvergleich (Zahlen, Toleranz, Bool, Wertabbildung)
//! - [`mapping`], [`deviations`], [`thresholds`], [`manifest`]: Eingabedateien aus `tests/parity` und
//!   `tests/golden/parity`
//! - [`diff`]: Schlüsselabgleich und Bericht; [`report`]: `report.json`/`report.md`
//! - [`run`]: Site starten, `openspider` aufrufen, alle Exporte des Manifests vergleichen
//! - [`coverage`]: Tag-Scan, Testergebnisse, Status je Spec-ID

pub mod coverage;
pub mod csvio;
pub mod deviations;
pub mod diff;
pub mod error;
pub mod fake;
pub mod manifest;
pub mod mapping;
pub mod normalize;
pub mod report;
pub mod run;
pub mod thresholds;

pub use error::ParityError;

mod xml;
