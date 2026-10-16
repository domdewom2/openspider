//! Vergleichsmodus (`compare.*`, OUT-140 ff., TECH-300 ff.).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::crawl::CrawlSummaryDto;

/// Eine Regel des URL-Mappings (OUT-146).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MappingRuleDto {
    /// Suchmuster (Regex).
    pub pattern: String,
    /// Ersetzung mit `$1`-Gruppen.
    pub replacement: String,
}

/// Argument von `compare.open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct CompareOpenArgs {
    /// Aktueller Crawl.
    pub current: String,
    /// Vorheriger Crawl.
    pub previous: String,
    /// URL-Mapping.
    pub mapping: Vec<MappingRuleDto>,
    /// Elemente der Change Detection (Kennungen laut TECH-300).
    pub elements: Vec<String>,
    /// Schwelle für Inhaltsänderungen in Prozent.
    pub content_threshold_pct: Option<u8>,
}

/// Eine geöffnete Vergleichssitzung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CompareSession {
    /// Aktueller Crawl.
    pub current: CrawlSummaryDto,
    /// Vorheriger Crawl.
    pub previous: CrawlSummaryDto,
    /// URL-Mapping.
    pub mapping: Vec<MappingRuleDto>,
    /// Aktive Elemente der Change Detection.
    pub elements: Vec<String>,
    /// Schwelle für Inhaltsänderungen in Prozent.
    pub content_threshold_pct: u8,
    /// Version der Klassifikation (ändert sich beim Tausch und bei Änderungen).
    pub version: u64,
}

/// Argument von `compare.testMapping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TestMappingArgs {
    /// Regeln.
    pub rules: Vec<MappingRuleDto>,
    /// Eingabe-URL.
    pub input: String,
}
