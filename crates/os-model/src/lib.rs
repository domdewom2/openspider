//! `os-model` – gemeinsame Typen für alle OpenSpider-Crates.
//!
//! Enthält Kennungen, Zeit-/ID-Quellen, Enums, das Parse- und Schreibmodell, die Crawl-Konfiguration
//! und die Indexierbarkeitsbewertung (M1-Plan 3.3/3.4). Das Crate kennt weder HTTP noch SQLite noch HTML.

pub mod config;
pub mod enums;
pub mod error;
pub mod ids;
pub mod indexability;
pub mod issue;
pub mod parse;
pub mod progress;
pub mod time;
pub mod value;
pub mod write;

pub use config::*;
pub use enums::*;
pub use error::ModelError;
pub use ids::*;
pub use indexability::{evaluate, IndexabilityInput};
pub use issue::*;
pub use parse::*;
pub use progress::Progress;
pub use time::*;
pub use value::Value;
pub use write::*;
