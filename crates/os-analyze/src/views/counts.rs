//! Übersichtszähler je Tab/Filter (ONPAGE-190, ONPAGE-001): Trefferzahl, Prozent und Filterzustand.
//!
//! Je Tab genügt ein Durchlauf über `urls`: jeder Filter trägt ein `SUM(CASE WHEN <filter_sql> …)` bei,
//! das aus demselben [`ViewPlan`](super::ViewPlan) stammt wie die Zeilenansicht.

use os_model::{CrawlConfig, FilterState};
use os_store::{queries, CrawlFile};

use super::build::build;
use super::exec::bind_named;
use super::{FilterCount, ViewQuery, URLS_FROM};
use crate::defs::TabDef;
use crate::error::{AnalyzeError, ViewError};
use crate::names;
use crate::state::{filter_states_for_file, AnalysisSnapshot};

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

/// Counting plan of one tab: one `SUM(CASE …)` per countable filter over the tab population.
struct TabPlan<'a> {
    tab: &'a TabDef,
    states: Vec<FilterState>,
    slots: Vec<Option<usize>>,
    query: Option<SumQuery>,
}

/// SQL of one tab's sums: whole crawl, or one ID range of it.
struct SumQuery {
    sql: String,
    ranged: String,
    params: Vec<(String, os_model::Value)>,
}

fn plan_tab<'a>(
    tabs: &[TabDef],
    tab: &'a TabDef,
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cfg_json: &serde_json::Value,
    analysis: &AnalysisSnapshot,
) -> Result<TabPlan<'a>, ViewError> {
    let states: Vec<FilterState> =
        filter_states_for_file(&tab.filters, tab, file, cfg, cfg_json, analysis)?;
    // Ein Plan je zählbarem Filter; Population und Parameter sind für den ganzen Tab gleich.
    let mut sums: Vec<String> = Vec::new();
    let mut slots: Vec<Option<usize>> = Vec::new();
    let mut population = None;
    let mut params = Vec::new();
    for (filter, state) in tab.filters.iter().zip(&states) {
        if !state.yields_rows() {
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
    // UI-145: the ID range lets disjoint parts of the crawl be counted on several connections.
    let query = population.map(|population| {
        let sums = sums.join(", ");
        // UI-145: the counter join costs a primary-key probe per URL; only tabs whose
        // population or filters read `c` need it (conservative textual test).
        let from = if sums.contains("c.") || population.contains("c.") {
            URLS_FROM
        } else {
            "urls u"
        };
        let sql = format!("SELECT {sums} FROM {from} WHERE {population}");
        SumQuery {
            ranged: format!("{sql} AND u.id > :os_count_after AND u.id <= :os_count_last"),
            sql,
            params,
        }
    });
    Ok(TabPlan {
        tab,
        states,
        slots,
        query,
    })
}

fn run_sums(
    conn: &rusqlite::Connection,
    query: Option<&SumQuery>,
    range: Option<(i64, i64)>,
) -> Result<Vec<i64>, ViewError> {
    let Some(query) = query else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(if range.is_some() {
        &query.ranged
    } else {
        &query.sql
    })?;
    let extra = range.map_or_else(Vec::new, |(after, last)| {
        vec![
            (
                ":os_count_after".into(),
                rusqlite::types::Value::Integer(after),
            ),
            (
                ":os_count_last".into(),
                rusqlite::types::Value::Integer(last),
            ),
        ]
    });
    bind_named(&mut stmt, &query.params, &extra)?;
    let columns = stmt.column_count();
    let mut rows = stmt.raw_query();
    let row = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    Ok((0..columns)
        .map(|i| row.get::<_, i64>(i))
        .collect::<Result<_, _>>()?)
}

fn assemble(
    plan: TabPlan<'_>,
    counts: &[i64],
    version: u64,
    cfg_json: &serde_json::Value,
    out: &mut Vec<FilterCount>,
) {
    let tab = plan.tab;
    // Basis der Prozentwerte: Größe des Filters „All“.
    let base = tab
        .filters
        .iter()
        .position(|f| f.id == "All")
        .and_then(|i| plan.slots[i])
        .map_or(0, |s| counts[s].max(0) as u64);
    for ((filter, state), slot) in tab.filters.iter().zip(plan.states).zip(plan.slots) {
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
            label: names::label_with(filter.id, cfg_json),
            count,
            pct,
            state,
            version,
        });
    }
}

pub(super) fn counts_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
) -> Result<Vec<FilterCount>, ViewError> {
    counts_with(tabs, file, cfg, false, &os_model::CancelToken::new())
}

/// Like [`counts_in`]; with `parallel` large crawls are counted in disjoint ID ranges on
/// additional read connections of the same data version (UI-145). Background counting uses
/// a quarter of the cores. Any version difference falls back to the caller's snapshot.
pub(super) fn counts_with(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    parallel: bool,
    cancel: &os_model::CancelToken,
) -> Result<Vec<FilterCount>, ViewError> {
    let conn = file.conn();
    let tx = conn
        .is_autocommit()
        .then(|| conn.unchecked_transaction())
        .transpose()?;
    let version = queries::change_seq(conn)?;
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let analysis = analysis_snapshot(file)?;
    let plans = tabs
        .iter()
        .map(|tab| plan_tab(tabs, tab, file, cfg, &cfg_json, &analysis))
        .collect::<Result<Vec<_>, _>>()?;
    let max_id: i64 = conn.query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
    let workers = if parallel {
        super::parallel_workers(max_id, 4)
    } else {
        1
    };
    let parallel = if workers > 1 {
        let queries: Vec<_> = plans.iter().map(|p| p.query.as_ref()).collect();
        parallel_sums(file, &queries, version, max_id, workers, cancel)?
    } else {
        None
    };
    let sums = match parallel {
        Some(sums) => {
            super::parallel_pass_done();
            sums
        }
        None => plans
            .iter()
            .map(|plan| {
                if cancel.is_cancelled() {
                    return Err(crate::search::SearchError::Cancelled.into());
                }
                run_sums(conn, plan.query.as_ref(), None)
            })
            .collect::<Result<Vec<_>, ViewError>>()?,
    };
    let mut out = Vec::new();
    for (plan, counts) in plans.into_iter().zip(sums) {
        assemble(plan, &counts, version, &cfg_json, &mut out);
    }
    drop(tx);
    Ok(out)
}

/// Parts per worker: small enough to balance tabs of very different cost.
const PARTS_PER_WORKER: i64 = 4;

fn parallel_sums(
    file: &CrawlFile,
    plans: &[Option<&SumQuery>],
    version: u64,
    max_id: i64,
    workers: usize,
    cancel: &os_model::CancelToken,
) -> Result<Option<Vec<Vec<i64>>>, ViewError> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let stamp = super::filter_cache_stamp(file)?;
    let parts = workers as i64 * PARTS_PER_WORKER;
    let step = max_id / parts + 1;
    let ranges: Vec<(i64, i64)> = (0..parts)
        .map(|i| {
            let after = if i == 0 { i64::MIN } else { i * step };
            let last = if i + 1 == parts {
                i64::MAX
            } else {
                (i + 1) * step
            };
            (after, last)
        })
        .collect();
    let items: Vec<(usize, (i64, i64))> = (0..plans.len())
        .filter(|p| plans[*p].is_some())
        .flat_map(|p| ranges.iter().map(move |r| (p, *r)))
        .collect();
    let next = AtomicUsize::new(0);
    let path = file.path();
    type Part = Vec<(usize, Vec<i64>)>;
    let results: Vec<Result<Option<Part>, ViewError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (items, next, stamp) = (&items, &next, &stamp);
                scope.spawn(move || -> Result<Option<Part>, ViewError> {
                    let worker = CrawlFile::open(path, os_store::OpenMode::ReadOnly)?;
                    let interrupt = worker.conn().get_interrupt_handle();
                    let _registration = cancel.on_cancel(move || interrupt.interrupt());
                    let _snapshot = worker.conn().unchecked_transaction()?;
                    if queries::change_seq(worker.conn())? != version
                        || super::filter_cache_stamp(&worker)? != *stamp
                    {
                        return Ok(None);
                    }
                    let mut done = Vec::new();
                    loop {
                        if cancel.is_cancelled() {
                            return Err(crate::search::SearchError::Cancelled.into());
                        }
                        let Some((plan, (after, last))) =
                            items.get(next.fetch_add(1, Ordering::Relaxed)).copied()
                        else {
                            return Ok(Some(done));
                        };
                        done.push((
                            plan,
                            run_sums(worker.conn(), plans[plan], Some((after, last)))?,
                        ));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    Err(ViewError::Store(os_store::StoreError::InvalidInput(
                        "Zähldurchlauf abgebrochen".into(),
                    )))
                })
            })
            .collect()
    });
    let mut sums: Vec<Vec<i64>> = vec![Vec::new(); plans.len()];
    for result in results {
        let Some(parts) = result? else {
            return Ok(None);
        };
        for (plan, counts) in parts {
            let total = &mut sums[plan];
            total.resize(counts.len(), 0);
            for (total, n) in total.iter_mut().zip(counts) {
                *total += n;
            }
        }
    }
    Ok(Some(sums))
}
