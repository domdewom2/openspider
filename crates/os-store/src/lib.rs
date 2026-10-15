//! `os-store` – Crawl-Datei (SQLite, WAL), Schema-Migrationen und Abfragen für UI-Tabellen
//! (Paging, Filter, Sortierung).
//!
//! Kollation `OS_TEXT` (P00c): `collation`.
//!
//! Kern (P00a): `error`, `meta`, `open`, `profile`, `schema`, `sink`. Stubs (P00b): `writer`, `counters`,
//! `files`, `resume`, `registry`, `paths`, `queries` – umgesetzt von M1-P07.

pub mod collation;
pub mod counters;
pub mod error;
pub mod files;
pub mod link_index;
pub mod meta;
pub mod open;
pub mod paths;
pub mod profile;
pub mod queries;
pub mod registry;
pub mod resume;
pub mod schema;
pub mod sink;
pub mod writer;

pub use error::StoreError;
pub use meta::CrawlMeta;
pub use open::{backup_path, CrawlFile, CrawlInit, OpenMode};
pub use profile::StorageProfile;
pub use resume::{QueueEntry, ResumeState};
pub use schema::{Migration, APPLICATION_ID, MIGRATIONS, SCHEMA_VERSION};
pub use sink::CrawlSink;
pub use writer::SqliteSink;
