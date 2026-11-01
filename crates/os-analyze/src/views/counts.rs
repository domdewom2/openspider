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

/// SQL of one tab's sums over its whole population, and of per-row memberships for given IDs.
struct SumQuery {
    sql: String,
    members: String,
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
    let mut flags: Vec<String> = Vec::new();
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
        flags.push(format!(
            "CASE WHEN {} THEN 1 ELSE 0 END",
            built.plan.filter_sql
        ));
    }
    let query = population.map(|population| {
        let sums = sums.join(", ");
        // UI-145: the counter join costs a primary-key probe per URL; only tabs whose
        // population or filters read `c` need it (conservative textual test).
        let from = if sums.contains("c.") || population.contains("c.") {
            URLS_FROM
        } else {
            "urls u"
        };
        SumQuery {
            sql: format!("SELECT {sums} FROM {from} WHERE {population}"),
            // UI-145 incremental path: the same predicates, evaluated only for the given rows.
            members: format!(
                "SELECT u.id, {} FROM {from} WHERE u.id IN (SELECT value FROM json_each(:os_member_ids)) AND {population}",
                flags.join(", ")
            ),
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

fn run_sums(conn: &rusqlite::Connection, query: Option<&SumQuery>) -> Result<Vec<i64>, ViewError> {
    let Some(query) = query else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(&query.sql)?;
    bind_named(&mut stmt, &query.params, &[])?;
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
    counts_with(tabs, None, file, cfg, false, &os_model::CancelToken::new())
}

/// Like [`counts_in`] for the tabs at `only` (all without). With `parallel`, large crawls count
/// several tabs at once on additional read connections of the same data version (UI-145), a
/// quarter of the cores. Each tab keeps exactly its serial SQL and plan. Any version difference
/// falls back to the caller's snapshot.
pub(super) fn counts_with(
    tabs: &[TabDef],
    only: Option<&[usize]>,
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
    let selected: Vec<usize> = match only {
        Some(only) => only.to_vec(),
        None => (0..tabs.len()).collect(),
    };
    let plans = selected
        .iter()
        .map(|i| plan_tab(tabs, &tabs[*i], file, cfg, &cfg_json, &analysis))
        .collect::<Result<Vec<_>, _>>()?;
    let busy = plans.iter().filter(|p| p.query.is_some()).count();
    let workers = if parallel && busy > 1 {
        let max_id: i64 =
            conn.query_row("SELECT COALESCE(MAX(id), 0) FROM urls", [], |r| r.get(0))?;
        super::parallel_workers(max_id, 4).min(busy)
    } else {
        1
    };
    let parallel = if workers > 1 {
        let queries: Vec<_> = plans.iter().map(|p| p.query.as_ref()).collect();
        parallel_sums(file, &queries, version, workers, cancel)?
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
                run_sums(conn, plan.query.as_ref())
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

fn parallel_sums(
    file: &CrawlFile,
    jobs: &[Option<&SumQuery>],
    version: u64,
    workers: usize,
    cancel: &os_model::CancelToken,
) -> Result<Option<Vec<Vec<i64>>>, ViewError> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let stamp = super::filter_cache_stamp(file)?;
    let next = AtomicUsize::new(0);
    let path = file.path();
    type Part = Vec<(usize, Vec<i64>)>;
    let results: Vec<Result<Option<Part>, ViewError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (next, stamp) = (&next, &stamp);
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
                        let item = next.fetch_add(1, Ordering::Relaxed);
                        let Some(query) = jobs.get(item) else {
                            return Ok(Some(done));
                        };
                        done.push((item, run_sums(worker.conn(), *query)?));
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
    let mut sums: Vec<Vec<i64>> = vec![Vec::new(); jobs.len()];
    for result in results {
        let Some(parts) = result? else {
            return Ok(None);
        };
        for (item, counts) in parts {
            sums[item] = counts;
        }
    }
    Ok(Some(sums))
}

/// Filter memberships among a set of URL IDs (UI-145 incremental path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterMembers {
    /// Tab.
    pub tab: String,
    /// Filter id as in `FilterCount::filter`.
    pub filter: String,
    /// Members among the requested IDs, ascending.
    pub ids: Vec<i64>,
}

/// Evaluates every countable filter of `tabs` for exactly `ids`; per-page cost instead of a crawl scan.
pub(super) fn members_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    ids: &[i64],
) -> Result<Vec<FilterMembers>, ViewError> {
    let conn = file.conn();
    let _tx = conn
        .is_autocommit()
        .then(|| conn.unchecked_transaction())
        .transpose()?;
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let analysis = analysis_snapshot(file)?;
    let list = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
    let mut out = Vec::new();
    for tab in tabs {
        let plan = plan_tab(tabs, tab, file, cfg, &cfg_json, &analysis)?;
        let Some(query) = &plan.query else {
            continue;
        };
        let mut stmt = conn.prepare(&query.members)?;
        bind_named(
            &mut stmt,
            &query.params,
            &[(
                ":os_member_ids".into(),
                rusqlite::types::Value::Text(list.clone()),
            )],
        )?;
        let width = plan.slots.iter().flatten().count();
        let mut hits: Vec<Vec<i64>> = vec![Vec::new(); width];
        let mut rows = stmt.raw_query();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            for (slot, hit) in hits.iter_mut().enumerate() {
                if row.get::<_, i64>(slot + 1)? == 1 {
                    hit.push(id);
                }
            }
        }
        for (filter, slot) in tab.filters.iter().zip(&plan.slots) {
            if let Some(slot) = slot {
                let mut ids = std::mem::take(&mut hits[*slot]);
                ids.sort_unstable();
                out.push(FilterMembers {
                    tab: tab.id.to_string(),
                    filter: filter.id.to_string(),
                    ids,
                });
            }
        }
    }
    Ok(out)
}
