//! TECH-145: ten deterministic iterations over a disk-backed, resolved graph.
//! Only URL routing data and rank/degree vectors live in RAM; edges stay in SQLite's temp file.
use crate::{analysis::CancelToken, error::AnalyzeError};
use os_model::{CrawlConfig, StepState};
use os_store::CrawlFile;

#[derive(Clone, Copy, Debug, Default)]
pub enum Scaling {
    #[default]
    Logarithmic,
    Linear,
}

#[derive(Debug)]
struct Route {
    id: i64,
    next: Option<i64>,
    node: Option<usize>,
}

fn resolve(routes: &[Route], mut id: i64) -> Option<usize> {
    let mut seen = [0i64; 11];
    for depth in 0..=10 {
        if seen[..depth].contains(&id) {
            return None;
        }
        seen[depth] = id;
        let route = &routes[routes.binary_search_by_key(&id, |r| r.id).ok()?];
        if let Some(node) = route.node {
            return Some(node);
        }
        id = route.next?;
    }
    None
}

pub fn run(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<StepState, AnalyzeError> {
    run_with_scaling(file, cfg, cancel, Scaling::Logarithmic)
}

/// Internal linear option for independent comparisons; public default remains logarithmic.
pub fn run_with_scaling(
    file: &CrawlFile,
    _: &CrawlConfig,
    cancel: &CancelToken,
    scaling: Scaling,
) -> Result<StepState, AnalyzeError> {
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    let conn = file.conn();
    let previous_temp_store: i64 = conn.query_row("PRAGMA temp_store", [], |r| r.get(0))?;
    // Die Einstellung gilt verbindungsweit; auch bei SQL-Fehlern und Abbruch zurücksetzen.
    let result = (|| {
        conn.execute_batch("PRAGMA temp_store = FILE; DROP TABLE IF EXISTS temp.d08_routes; DROP TABLE IF EXISTS temp.d08_edges;
            CREATE TEMP TABLE d08_routes (id INTEGER PRIMARY KEY, node INTEGER, resolved INTEGER);")?;
        super::links::with_sql_cancel(conn, cancel, || calculate(file, cancel, scaling))
    })();
    let cleanup = conn.execute_batch(
        "DROP TABLE IF EXISTS temp.d08_routes; DROP TABLE IF EXISTS temp.d08_edges;",
    );
    let restore = conn.pragma_update(None, "temp_store", previous_temp_store);
    let state = result?;
    cleanup?;
    restore?;
    Ok(state)
}

fn calculate(
    file: &CrawlFile,
    cancel: &CancelToken,
    scaling: Scaling,
) -> Result<StepState, AnalyzeError> {
    let conn = file.conn();
    let mut nodes = Vec::new();
    let mut routes = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, is_internal, state, content_class, status_code, redirect_type,
            redirect_target_id, canonical_target_id FROM urls ORDER BY id",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let id: i64 = r.get(0)?;
            let internal: bool = r.get(1)?;
            let status: Option<i64> = r.get(4)?;
            let redirect: Option<String> = r.get(5)?;
            let canonical: Option<i64> = r.get(7)?;
            let next = if redirect.is_some() || status.is_some_and(|s| (300..400).contains(&s)) {
                r.get(6)?
            } else {
                canonical.filter(|c| *c != id)
            };
            let participates = internal
                && r.get::<_, String>(2)? == "done"
                && r.get::<_, String>(3)? == "HTML"
                && status.is_some_and(|s| s > 0)
                && redirect.is_none()
                && !status.is_some_and(|s| (300..400).contains(&s))
                && canonical.is_none_or(|c| c == id);
            let node = participates.then_some(nodes.len());
            if participates {
                nodes.push(id);
            }
            routes.push(Route { id, next, node });
        }
    }
    {
        let tx = conn.unchecked_transaction()?;
        {
            let mut insert = tx.prepare("INSERT INTO d08_routes VALUES (?1, ?2, ?3)")?;
            for route in &routes {
                if cancel.is_cancelled() {
                    return Ok(StepState::Pending);
                }
                insert.execute(rusqlite::params![
                    route.id,
                    route.node.map(|n| n as i64),
                    resolve(&routes, route.id).map(|n| n as i64)
                ])?;
            }
        }
        tx.commit()?;
    }
    drop(routes);
    if cancel.is_cancelled() {
        return Ok(StepState::Pending);
    }
    conn.execute_batch("CREATE TEMP TABLE d08_edges (source INTEGER, target INTEGER, follow INTEGER, PRIMARY KEY(source, target)) WITHOUT ROWID;
        INSERT INTO d08_edges SELECT s.node, t.resolved,
            MAX(l.follow <> 0 AND COALESCE(u.effective_follow, 1) <> 0
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% nofollow %'
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% ugc %'
                AND (' ' || COALESCE(l.rel, '') || ' ') NOT LIKE '% sponsored %')
        FROM links l JOIN d08_routes s ON s.id = l.source_id JOIN d08_routes t ON t.id = l.target_id
        JOIN urls u ON u.id = l.source_id
        WHERE s.node IS NOT NULL AND t.resolved IS NOT NULL AND s.node <> t.resolved
            AND l.link_type = 'Hyperlink' AND l.crawlable = 1
        GROUP BY s.node, t.resolved;")?;
    let n = nodes.len();
    let mut degree = vec![0u64; n];
    {
        let mut stmt = conn.prepare("SELECT source, COUNT(*) FROM d08_edges GROUP BY source")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            degree[row.get::<_, usize>(0)?] = row.get(1)?;
        }
    }
    let mut rank = vec![1.0 / n.max(1) as f64; n];
    let base = 0.15 / n.max(1) as f64;
    let mut next = vec![base; n];
    for iteration in 0..10 {
        if cancel.is_cancelled() {
            return Ok(StepState::Pending);
        }
        next.fill(base);
        let mut stmt = conn.prepare(
            "SELECT source, target FROM d08_edges WHERE follow = 1 ORDER BY source, target",
        )?;
        let mut rows = stmt.query([])?;
        let mut visited = 0u64;
        while let Some(row) = rows.next()? {
            if visited.is_multiple_of(8192) && cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let source: usize = row.get(0)?;
            let target: usize = row.get(1)?;
            next[target] += 0.85 * rank[source] / degree[source] as f64;
            visited += 1;
        }
        std::mem::swap(&mut rank, &mut next);
        cancel.set_progress((iteration + 1) as f32 / 10.0);
    }
    let transform = |r: f64| match scaling {
        Scaling::Logarithmic => r.ln(),
        Scaling::Linear => r,
    };
    let min = rank.iter().copied().fold(f64::INFINITY, f64::min);
    let max = rank.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let score_of = |raw: f64| {
        if min == max {
            100
        } else {
            (1.0 + 99.0 * (transform(raw) - transform(min)) / (transform(max) - transform(min)))
                .round()
                .clamp(1.0, 100.0) as i64
        }
    };
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let unchanged = {
        let mut stmt = tx.prepare("SELECT url_id,key,v_int,v_real FROM url_ext WHERE key IN ('linkscore.raw','linkscore.score') ORDER BY url_id,key")?;
        let mut rows = stmt.query([])?;
        let mut same = true;
        for (&id, &raw) in nodes.iter().zip(&rank) {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let Some(row) = rows.next()? else {
                same = false;
                break;
            };
            if row.get::<_, i64>(0)? != id
                || row.get::<_, String>(1)? != "linkscore.raw"
                || row.get::<_, Option<f64>>(3)? != Some(raw)
            {
                same = false;
                break;
            }
            let Some(row) = rows.next()? else {
                same = false;
                break;
            };
            if row.get::<_, i64>(0)? != id
                || row.get::<_, String>(1)? != "linkscore.score"
                || row.get::<_, Option<i64>>(2)? != Some(score_of(raw))
            {
                same = false;
                break;
            }
        }
        same && rows.next()?.is_none()
    };
    if unchanged {
        tx.commit()?;
        return Ok(StepState::Done);
    }
    let seq = os_store::queries::change_seq(conn)? + 1;
    // Vorhandene/neu veröffentlichte Scores melden sich über url_ext.change_seq.
    // Nur entfernte Knoten brauchen einen zusätzlichen Stempel auf urls.
    tx.execute("UPDATE urls SET change_seq = ?1 WHERE id IN (SELECT url_id FROM url_ext WHERE key IN ('linkscore.raw', 'linkscore.score')) AND id NOT IN (SELECT id FROM d08_routes WHERE node IS NOT NULL)", [seq])?;
    tx.execute_batch("DELETE FROM url_ext WHERE key IN ('linkscore.raw', 'linkscore.score')")?;
    {
        let mut insert = tx.prepare("INSERT INTO url_ext (url_id, key, v_int, v_real, change_seq) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for (id, raw) in nodes.iter().zip(rank) {
            if cancel.is_cancelled() {
                return Ok(StepState::Pending);
            }
            let score = score_of(raw);
            insert.execute(rusqlite::params![
                id,
                "linkscore.raw",
                Option::<i64>::None,
                raw,
                seq
            ])?;
            insert.execute(rusqlite::params![
                id,
                "linkscore.score",
                score,
                Option::<f64>::None,
                seq
            ])?;
        }
    }
    tx.execute(
        "INSERT OR REPLACE INTO crawl_meta (key, value) VALUES ('change_seq', ?1)",
        [seq.to_string()],
    )?;
    tx.commit()?;
    Ok(StepState::Done)
}
