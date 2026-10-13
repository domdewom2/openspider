//! Kennungen: `UrlId`, `SitemapId`, `CrawlId`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::ModelError;

/// Kennung einer URL (= `urls.id`), von der Engine in Entdeckungsreihenfolge vergeben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UrlId(pub i64);

/// Kennung einer Sitemap-Datei (= `sitemaps.id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SitemapId(pub i64);

/// Stabile Kennung eines Crawls (UUID v7, CRAWL-128). `Display`/`FromStr`: klein, mit Bindestrichen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CrawlId(pub uuid::Uuid);

impl fmt::Display for UrlId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for SitemapId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for CrawlId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl FromStr for CrawlId {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s.trim())
            .map(CrawlId)
            .map_err(|_| ModelError::InvalidCrawlId(s.to_string()))
    }
}
