//! Filterzustände (`FilterState`): bereit, Crawl-Analyse nötig, Konfiguration nötig, Rendering nötig, veraltet
//! (ONPAGE-007, TECH-005, CRAWL-141).
//!
//! Reihenfolge der Prüfung: (1) Konfigurationsschlüssel aus `FilterDef.requires` → `NeedsConfig`,
//! (2) Rendering → `NeedsRendering`, (3) Live-Filter → `Ready`, (4) Nachlauf-Filter: Teilschritt abgewählt
//! oder nicht (erfolgreich) gelaufen → `NeedsCrawlAnalysis`; gelaufen, aber seither veraltet →
//! `Stale`; sonst `Ready`. Ein Filter in einem anderen Zustand als `Ready` liefert nie eine Trefferzahl
//! und keine Zeilen – nie 0 an Stelle eines Zustands.

use std::collections::BTreeMap;

use os_model::{AnalysisStep, CrawlConfig, FilterState, RenderingMode, StepState};
use os_store::CrawlFile;
use serde_json::Value as Json;

use crate::analysis::persist::{self, StaleSet, StepRecord};
use crate::defs::{FilterDef, FilterKind, Requirement};
use crate::error::AnalyzeError;
use crate::names;

/// Konfigurationsabschnitte, deren Änderung einen Teilschritt veralten lässt (CRAWL-141, Punkt 6).
///
/// Der Schritt „Sitemaps“ hängt zusätzlich an `spider.*`: Linkspeicherung, hreflang und Canonical bestimmen,
/// welche URLs als verlinkt gelten und damit die Orphan-Bewertung (TECH-005).
fn relevant_sections(step: AnalysisStep) -> &'static [&'static str] {
    match step {
        AnalysisStep::LinkScore | AnalysisStep::Links => &["links"],
        AnalysisStep::Canonicals | AnalysisStep::Pagination | AnalysisStep::Amp => &["spider"],
        AnalysisStep::Hreflang => &["hreflang"],
        AnalysisStep::Sitemaps => &["sitemaps", "spider"],
        AnalysisStep::Content => &["content", "duplicates"],
        AnalysisStep::Images => &["image", "rendering"],
        AnalysisStep::Analytics | AnalysisStep::SearchConsole | AnalysisStep::LinkMetrics => &[],
    }
}

/// Hash (16 Hexziffern, FNV-1a) der für den Teilschritt relevanten Konfiguration.
pub fn step_config_hash(step: AnalysisStep, cfg: &CrawlConfig) -> String {
    let json = serde_json::to_value(cfg).unwrap_or(Json::Null);
    step_config_hash_json(step, &json)
}

/// Wie [`step_config_hash`], mit bereits in JSON gewandelter Konfiguration.
pub fn step_config_hash_json(step: AnalysisStep, cfg_json: &Json) -> String {
    let mut text = String::new();
    for section in relevant_sections(step) {
        text.push_str(section);
        text.push('=');
        if let Some(value) = cfg_json.get(*section) {
            text.push_str(&value.to_string());
        }
        text.push(';');
    }
    crate::cursor::query_hash(&[&text])
}

/// Wie ein Teilschritt zum aktuellen Zeitpunkt dasteht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Fertig und aktuell.
    Fresh,
    /// Fertig, aber seither veraltet (Markierung oder geänderte Konfiguration).
    Stale,
    /// Nicht fertig (nie gelaufen, abgebrochen, übersprungen, nicht anwendbar).
    Missing,
}

/// Stand der Crawl-Analyse einer Datei, einmal gelesen und für viele Filter verwendet.
#[derive(Debug, Clone, Default)]
pub struct AnalysisSnapshot {
    steps: BTreeMap<AnalysisStep, StepRecord>,
    stale: StaleSet,
}

impl AnalysisSnapshot {
    /// Liest `analysis_steps` und die Veraltet-Markierungen.
    pub fn load(file: &CrawlFile) -> Result<Self, AnalyzeError> {
        Ok(AnalysisSnapshot {
            steps: persist::read_steps(file)?,
            stale: persist::read_stale(file)?,
        })
    }

    /// Gespeicherter Zustand eines Teilschritts, falls vorhanden.
    pub fn record(&self, step: AnalysisStep) -> Option<&StepRecord> {
        self.steps.get(&step)
    }

    /// Frische des Teilschritts bezogen auf die aktuelle Konfiguration.
    pub fn freshness(&self, step: AnalysisStep, cfg_json: &Json) -> Freshness {
        match self.steps.get(&step) {
            Some(rec) if rec.state == StepState::Done => {
                if self.stale.contains(step) {
                    return Freshness::Stale;
                }
                let now = step_config_hash_json(step, cfg_json);
                match &rec.config_hash {
                    Some(stored) if *stored != now => Freshness::Stale,
                    _ => Freshness::Fresh,
                }
            }
            _ => Freshness::Missing,
        }
    }
}

/// Ist der Konfigurationsschlüssel (`duplicates.near_enabled`) vorhanden und `true`?
pub fn config_key_enabled(cfg_json: &Json, key: &str) -> bool {
    matches!(names::config_value(cfg_json, key), Some(Json::Bool(true)))
}

/// Zustand eines Filters. `cfg_json` ist `serde_json::to_value(cfg)`.
pub fn filter_state(
    def: &FilterDef,
    cfg: &CrawlConfig,
    cfg_json: &Json,
    analysis: &AnalysisSnapshot,
) -> FilterState {
    for req in &def.requires {
        if let Requirement::Config(key) = req {
            if !config_key_enabled(cfg_json, key) {
                return FilterState::NeedsConfig((*key).to_string());
            }
        }
    }
    if def.requires.contains(&Requirement::Rendering)
        && cfg.rendering.mode != RenderingMode::JavaScript
    {
        return FilterState::NeedsRendering;
    }
    match def.kind {
        FilterKind::Live => FilterState::Ready,
        FilterKind::PostCrawl(step) => {
            if !cfg.analysis.step_enabled(step) {
                return FilterState::NeedsCrawlAnalysis;
            }
            match analysis.freshness(step, cfg_json) {
                Freshness::Fresh => FilterState::Ready,
                Freshness::Stale => FilterState::Stale,
                Freshness::Missing => FilterState::NeedsCrawlAnalysis,
            }
        }
    }
}
