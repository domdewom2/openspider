//! `os-model` – gemeinsame Typen für alle OpenSpider-Crates.
//!
//! Enthält `CrawlConfig`, `UrlRecord`, `Link`, `Issue` und IDs. Das Crate hängt nur an
//! `serde` und ist die Basis, auf die alle anderen Crates aufsetzen.

use serde::{Deserialize, Serialize};

/// Konfiguration eines Crawls (Gerüst; wird mit der Spec erweitert).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// Start-URL des Crawls.
    pub start_url: String,
    /// Optionales Limit für die Anzahl gecrawlter URLs.
    pub max_urls: Option<u64>,
}

impl CrawlConfig {
    /// Neue Konfiguration mit Standardwerten für die gegebene Start-URL.
    pub fn new(start_url: impl Into<String>) -> Self {
        Self {
            start_url: start_url.into(),
            max_urls: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_has_defaults() {
        let cfg = CrawlConfig::new("https://example.com/");
        assert_eq!(cfg.start_url, "https://example.com/");
        assert_eq!(cfg.max_urls, None);
    }
}
