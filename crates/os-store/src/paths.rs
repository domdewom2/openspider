//! Speicherorte der Anwendung (OUT-227); `OPENSPIDER_HOME` verlegt alles unter einen Ordner.
//!
//! STUB(M1-P07)

use std::path::PathBuf;

/// Standardordner der Anwendung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Einstellungen.
    pub settings: PathBuf,
    /// Crawl-Dateien.
    pub crawls: PathBuf,
    /// Protokolle.
    pub logs: PathBuf,
    /// Zwischenspeicher.
    pub cache: PathBuf,
}

/// Ermittelt die Ordner (plattformüblich über `dirs`, überschreibbar mit `OPENSPIDER_HOME`).
// STUB(M1-P07): Unterordner von `OPENSPIDER_HOME` bzw. des Datenordners des Benutzers.
pub fn app_paths() -> AppPaths {
    let root = std::env::var_os("OPENSPIDER_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|d| d.join("OpenSpider")))
        .unwrap_or_else(|| PathBuf::from("OpenSpider"));
    AppPaths {
        settings: root.join("settings"),
        crawls: root.join("crawls"),
        logs: root.join("logs"),
        cache: root.join("cache"),
    }
}
