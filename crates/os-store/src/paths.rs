//! Speicherorte der Anwendung (OUT-227, OUT-231); `OPENSPIDER_HOME` verlegt alles unter einen Ordner.
//!
//! | Ordner | macOS | Windows |
//! |---|---|---|
//! | Einstellungen | `~/Library/Application Support/OpenSpider/` | `%APPDATA%\OpenSpider\` |
//! | Crawls (Standard) | `~/Library/Application Support/OpenSpider/crawls/` | `%LOCALAPPDATA%\OpenSpider\crawls\` |
//! | Logs | `~/Library/Logs/OpenSpider/` | `%LOCALAPPDATA%\OpenSpider\logs\` |
//! | Cache | `~/Library/Caches/OpenSpider/` | `%LOCALAPPDATA%\OpenSpider\cache\` |
//!
//! Mit `OPENSPIDER_HOME=<Ordner>` liegen die Einstellungen direkt in `<Ordner>`, die übrigen Ordner als
//! `<Ordner>/crawls`, `<Ordner>/logs` und `<Ordner>/cache`; die App schreibt dann nichts außerhalb davon. Auf
//! anderen Systemen (Linux, nur Entwicklung) gelten die `dirs`-Standardorte.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::StoreError;

/// Name der Umgebungsvariablen, die alle Basispfade überschreibt.
pub const HOME_ENV: &str = "OPENSPIDER_HOME";

/// Name des Anwendungsordners unter den Benutzerverzeichnissen.
const APP_DIR: &str = "OpenSpider";

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

impl AppPaths {
    /// Alle Ordner (Einstellungen, Crawls, Logs, Cache).
    pub fn all(&self) -> [&Path; 4] {
        [&self.settings, &self.crawls, &self.logs, &self.cache]
    }

    /// Legt fehlende Ordner an.
    pub fn create_all(&self) -> Result<(), StoreError> {
        for dir in self.all() {
            std::fs::create_dir_all(dir).map_err(|e| StoreError::io(dir, e))?;
        }
        Ok(())
    }
}

/// Ermittelt die Ordner (plattformüblich über `dirs`, überschreibbar mit `OPENSPIDER_HOME`; ein leerer Wert
/// zählt als nicht gesetzt).
pub fn app_paths() -> AppPaths {
    app_paths_with_home(std::env::var_os(HOME_ENV).as_deref())
}

/// Wie [`app_paths`] mit ausdrücklich gegebenem Wert von `OPENSPIDER_HOME` (`None` oder leer = Standardorte).
pub fn app_paths_with_home(home: Option<&OsStr>) -> AppPaths {
    match home.filter(|h| !h.is_empty()) {
        Some(home) => {
            let root = PathBuf::from(home);
            AppPaths {
                crawls: root.join("crawls"),
                logs: root.join("logs"),
                cache: root.join("cache"),
                settings: root,
            }
        }
        None => default_paths(),
    }
}

fn relative_fallback() -> PathBuf {
    PathBuf::from(APP_DIR)
}

fn default_paths() -> AppPaths {
    let settings = dirs::config_dir()
        .map(|d| d.join(APP_DIR))
        .unwrap_or_else(relative_fallback);
    let crawls = dirs::data_local_dir()
        .map(|d| d.join(APP_DIR).join("crawls"))
        .unwrap_or_else(|| relative_fallback().join("crawls"));
    AppPaths {
        settings,
        crawls,
        logs: default_logs(),
        cache: default_cache(),
    }
}

#[cfg(target_os = "macos")]
fn default_logs() -> PathBuf {
    dirs::home_dir()
        .map(|d| d.join("Library").join("Logs").join(APP_DIR))
        .unwrap_or_else(|| relative_fallback().join("logs"))
}

#[cfg(not(target_os = "macos"))]
fn default_logs() -> PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join(APP_DIR).join("logs"))
        .unwrap_or_else(|| relative_fallback().join("logs"))
}

#[cfg(windows)]
fn default_cache() -> PathBuf {
    // Unter Windows ist der Cache-Ordner von `dirs` bereits `%LOCALAPPDATA%`.
    dirs::cache_dir()
        .map(|d| d.join(APP_DIR).join("cache"))
        .unwrap_or_else(|| relative_fallback().join("cache"))
}

#[cfg(not(windows))]
fn default_cache() -> PathBuf {
    dirs::cache_dir()
        .map(|d| d.join(APP_DIR))
        .unwrap_or_else(|| relative_fallback().join("cache"))
}
