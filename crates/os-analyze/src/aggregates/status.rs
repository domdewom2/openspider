//! Teilschrittspezifischer Analyse-Status (UI-082, ONPAGE-140).
use os_model::{AnalysisStep, CrawlConfig, RenderingMode, StepState};
use os_store::{CrawlFile, StoreError};

use crate::analysis::{persist, runner::predetermined_state};
use crate::state::{step_config_hash, AnalysisSnapshot, Freshness};

/// F05-Vertrag: `steps` behält die Kernzustände; Hinweise stehen getrennt davon,
/// da „veraltet“ und „Recrawl nötig“ keine abgeschlossenen Analyseergebnisse sind.
#[derive(Debug, Clone, Default)]
pub struct AnalysisStatusData {
    pub steps: Vec<(AnalysisStep, StepState)>,
    pub stale: Vec<AnalysisStep>,
    pub needs_recrawl: Vec<AnalysisStep>,
    /// Ein Lesefehler darf nicht als „noch nie analysiert“ verschwinden.
    pub error: Option<String>,
}

pub fn analysis_status(file: &CrawlFile, cfg: &CrawlConfig) -> AnalysisStatusData {
    match try_analysis_status(file, cfg) {
        Ok(data) => data,
        Err(error) => AnalysisStatusData {
            error: Some(error.to_string()),
            ..Default::default()
        },
    }
}

/// Fehlerbehaftete Variante für Aufrufer, die Datenbankfehler weitergeben können.
pub fn try_analysis_status(
    file: &CrawlFile,
    cfg: &CrawlConfig,
) -> Result<AnalysisStatusData, crate::AnalyzeError> {
    let snapshot = AnalysisSnapshot::load(file)?;
    let json = serde_json::to_value(cfg).map_err(|e| StoreError::InvalidInput(e.to_string()))?;
    let stored = file.config()?;
    let mut data = AnalysisStatusData::default();
    let mut missing_html = None;
    for &step in AnalysisStep::ALL {
        let record = snapshot.record(step);
        let state = predetermined_state(step, cfg)
            .unwrap_or_else(|| record.map_or(StepState::Pending, |r| r.state));
        data.steps.push((step, state));
        let content_sensitive =
            matches!(step, AnalysisStep::Content | AnalysisStep::NearDuplicates)
                && cfg.analysis.step_enabled(step);
        let mut area_changed = content_sensitive && stored.content.area != cfg.content.area;
        if content_sensitive {
            let hash = record
                .and_then(|r| r.config_hash.as_deref())
                .unwrap_or("pending");
            if let Some(area) = file.get_meta(&format!("k03.area.{}.{}", step.as_str(), hash))? {
                area_changed |= area
                    != serde_json::to_string(&cfg.content.area)
                        .map_err(|e| StoreError::InvalidInput(e.to_string()))?;
            }
        }
        if area_changed
            || (state == StepState::Done && snapshot.freshness(step, &json) == Freshness::Stale)
        {
            data.stale.push(step);
        }
        if area_changed {
            let missing = match missing_html {
                Some(missing) => missing,
                None => {
                    let (table, column) = if cfg.rendering.mode == RenderingMode::JavaScript {
                        ("render_pages", "html")
                    } else {
                        ("page_html", "body")
                    };
                    // Eine einzige fehlende HTML-Seite verhindert die vollständige
                    // Neuberechnung. Ein Speicherschalter allein beweist nichts.
                    let sql = format!("SELECT EXISTS(SELECT 1 FROM urls u WHERE u.is_internal=1 AND u.content_class='HTML'
                        AND u.status_code BETWEEN 200 AND 299 AND NOT EXISTS(SELECT 1 FROM {table} h WHERE h.url_id=u.id AND h.{column} IS NOT NULL))");
                    let missing: bool = file.conn().query_row(&sql, [], |r| r.get(0))?;
                    missing_html = Some(missing);
                    missing
                }
            };
            if missing {
                data.needs_recrawl.push(step);
            }
        }
    }
    Ok(data)
}

/// Persistiert nur die tatsächlich betroffenen, bereits abgeschlossenen Schritte.
/// Alte Analysewerte bleiben erhalten. Der Inhaltsbereich zum Ergebnis wird auch
/// dann bewahrt, wenn `config.set` die neue Konfiguration bereits gespeichert hat.
pub fn on_config_changed(
    file: &CrawlFile,
    old: &CrawlConfig,
    new: &CrawlConfig,
) -> Result<(), crate::AnalyzeError> {
    let records = persist::read_steps(file)?;
    let tx = file.conn().unchecked_transaction()?;
    let mut stale = Vec::new();
    for &step in AnalysisStep::ALL {
        if step_config_hash(step, old) == step_config_hash(step, new) {
            continue;
        }
        let record = records.get(&step);
        if record.is_some_and(|r| r.state == StepState::Done) {
            stale.push(step);
        }
        if old.content.area != new.content.area
            && matches!(step, AnalysisStep::Content | AnalysisStep::NearDuplicates)
        {
            let hash = record
                .and_then(|r| r.config_hash.as_deref())
                .unwrap_or("pending");
            let key = format!("k03.area.{}.{}", step.as_str(), hash);
            if file.get_meta(&key)?.is_none() {
                let area = serde_json::to_string(&old.content.area)
                    .map_err(|e| StoreError::InvalidInput(e.to_string()))?;
                file.set_meta(&key, &area)?;
            }
        }
    }
    persist::mark_stale(file, &stale)?;
    tx.commit().map_err(StoreError::from)?;
    Ok(())
}

/// Stored robots bodies for the app's effective-agent pacing notice (CRAWL-145).
/// Only the small per-origin table is read; URL tables are never scanned.
pub fn robots_bodies(file: &CrawlFile) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = file
        .conn()
        .prepare("SELECT origin, body FROM robots_files WHERE body IS NOT NULL")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}
