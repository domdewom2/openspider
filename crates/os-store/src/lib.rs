//! `os-store` – Crawl-Datei (SQLite, WAL), Schema-Migrationen und Abfragen für UI-Tabellen
//! (Paging, Filter, Sortierung).

pub mod error;
pub mod meta;
pub mod open;
pub mod profile;
pub mod schema;
pub mod sink;

pub use error::StoreError;
pub use meta::CrawlMeta;
pub use open::{backup_path, CrawlFile, CrawlInit, OpenMode};
pub use profile::StorageProfile;
pub use schema::{Migration, APPLICATION_ID, MIGRATIONS, SCHEMA_VERSION};
pub use sink::CrawlSink;
