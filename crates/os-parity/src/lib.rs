//! `os-parity` – Paritätsabgleich gegen Referenz-Exporte (`diff`, `run`) und Spec-Abdeckung aus Tags und
//! Testergebnissen (`coverage`), Abschnitt 5 des M1-Plans.
//!
//! Gerüst (P00b); die Umsetzung liefert M1-P11. Das Werkzeug kennt bewusst keine OpenSpider-Crates außer
//! `os-model` und `os-testsite` (Start der Fixture-Sites für `run`).

pub mod coverage;
pub mod diff;
pub mod error;
pub mod run;

pub use error::ParityError;
