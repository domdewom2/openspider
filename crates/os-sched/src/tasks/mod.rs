//! Aufgabendefinition, Zeitplan, Validierung und Ablage (OUT-160 … OUT-165, OUT-168, OUT-170).
//!
//! * [`model`]: die Aufgabe als TOML-Datei `schedules/<task-id>.ostask` (OS-Festlegung).
//! * [`zone`] und [`schedule`]: lokale Zeit mit Sommerzeit und die Terminberechnung `next_run` (OUT-161).
//! * [`store`]: Ablage der Aufgabendateien (atomar, Namen eindeutig).
//! * [`validate`]: Gültigkeitsprüfung mit Gründen (OUT-168).
//! * [`bundle`]: Export und Import ohne Geheimnisse (OUT-170).

pub mod bundle;
pub mod model;
pub mod schedule;
pub mod store;
pub mod validate;
pub mod zone;

pub use model::{
    CrawlSpec, CustomSummary, EffectiveExports, Execution, ExportFormat, ExportItem, ExportOptions,
    Notify, Output, OutputMode, Overlap, Schedule, Source, Task,
};
pub use schedule::next_run;
pub use store::{StoreError, TaskStore};
pub use zone::{EuZone, FixedZone, SystemZone, Zone};
