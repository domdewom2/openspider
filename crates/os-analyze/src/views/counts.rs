//! Übersichtszähler je Tab/Filter (ONPAGE-190, ONPAGE-001): Trefferzahl, Prozent und Filterzustand.
//!
//! Je Tab genügt ein Durchlauf über `urls`: jeder Filter trägt ein `SUM(CASE WHEN <filter_sql> …)` bei,
//! das aus demselben [`ViewPlan`](super::ViewPlan) stammt wie die Zeilenansicht.

use os_model::{CrawlConfig, FilterState};
use os_store::{queries, CrawlFile};

use super::build::build;
use super::exec::bind_named;
use super::{FilterCount, ViewQuery};
use crate::defs::TabDef;
use crate::error::{AnalyzeError, ViewError};
use crate::names;
use crate::state::{filter_state, AnalysisSnapshot};

/// Prozent = Treffer ÷ Basis × 100, kaufmännisch auf eine Nachkommastelle; leere Basis ergibt 0,0.
pub fn percent(hits: u64, base: u64) -> f64 {
    if base == 0 {
        return 0.0;
    }
    // Ganzzahlig auf Zehntelprozent runden (halbe Werte nach oben), damit 3/7 → 42,9 ohne Gleitkommafehler.
    let tenths = (u128::from(hits) * 2000 + u128::from(base)) / (2 * u128::from(base));
    tenths as f64 / 10.0
}

/// Fehler der Analyse-Persistenz als Ansichtsfehler.
pub(super) fn view_error(e: AnalyzeError) -> ViewError {
    match e {
        AnalyzeError::Store(s) => ViewError::Store(s),
        AnalyzeError::Sql(s) => ViewError::Sql(s),
        AnalyzeError::NotImplemented(m) => ViewError::NotImplemented(m),
    }
}

/// Liest den Stand der Crawl-Analyse.
pub(super) fn analysis_snapshot(file: &CrawlFile) -> Result<AnalysisSnapshot, ViewError> {
    AnalysisSnapshot::load(file).map_err(view_error)
}

pub(super) fn counts_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
) -> Result<Vec<FilterCount>, ViewError> {
    let conn = file.conn();
    let tx = conn.unchecked_transaction()?;
    let version = queries::change_seq(conn)?;
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let analysis = analysis_snapshot(file)?;

    let mut out = Vec::new();
    for tab in tabs {
        let states: Vec<FilterState> = tab
            .filters
            .iter()
            .map(|f| filter_state(f, cfg, &cfg_json, &analysis))
            .collect();

        // Ein Plan je zählbarem Filter; Population und Parameter sind für den ganzen Tab gleich.
        let mut sums: Vec<String> = Vec::new();
        let mut slots: Vec<Option<usize>> = Vec::new();
        let mut population = None;
        let mut params = Vec::new();
        for (filter, state) in tab.filters.iter().zip(&states) {
            if *state != FilterState::Ready {
                slots.push(None);
                continue;
            }
            let built = build(tabs, &ViewQuery::new(tab.id, filter.id), cfg)?;
            population.get_or_insert_with(|| built.plan.population_sql.clone());
            if params.is_empty() {
                params = built.plan.params.clone();
            }
            slots.push(Some(sums.len()));
            sums.push(format!(
                "COALESCE(SUM(CASE WHEN {} THEN 1 ELSE 0 END), 0)",
                built.plan.filter_sql
            ));
        }

        let counts: Vec<i64> = match population {
            Some(population) => {
                let sql = format!("SELECT {} FROM urls u WHERE {population}", sums.join(", "));
                let mut stmt = conn.prepare(&sql)?;
                bind_named(&mut stmt, &params, &[])?;
                let mut rows = stmt.raw_query();
                let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
                (0..sums.len())
                    .map(|i| row.get::<_, i64>(i))
                    .collect::<Result<_, _>>()?
            }
            None => Vec::new(),
        };

        // Basis der Prozentwerte: Größe des Filters „All“.
        let base = tab
            .filters
            .iter()
            .position(|f| f.id == "All")
            .and_then(|i| slots[i])
            .map_or(0, |s| counts[s].max(0) as u64);

        for ((filter, state), slot) in tab.filters.iter().zip(states).zip(slots) {
            let (count, pct) = match slot {
                Some(s) => {
                    let hits = counts[s].max(0) as u64;
                    (Some(hits), Some(percent(hits, base)))
                }
                None => (None, None),
            };
            out.push(FilterCount {
                tab: tab.id.to_string(),
                filter: filter.id.to_string(),
                label: names::label_with(filter.id, &cfg_json),
                count,
                pct,
                state,
                version,
            });
        }
    }
    drop(tx);
    Ok(out)
}
