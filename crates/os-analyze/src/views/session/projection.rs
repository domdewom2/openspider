//! Window projection by primary key; bounded crawl-wide width cache independent of view filters.
use super::super::build::{
    multi_nth_expr, numbered_name, series_parts, Built, Pick, SelectedColumn,
};
use super::super::URLS_FROM;
use crate::defs::{ColumnKind, ColumnSql, PairedPart};
use crate::error::ViewError;
use os_model::{UrlId, Value};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, Statement};
fn round_to(x: f64, decimals: u8) -> f64 {
    let factor = 10f64.powi(i32::from(decimals));
    (x * factor).round() / factor
}

/// Zellwert nach Spaltenart.
fn cell(kind: ColumnKind, v: ValueRef<'_>) -> Value {
    match (kind, v) {
        (_, ValueRef::Null) => Value::Null,
        (ColumnKind::Bool, ValueRef::Integer(i)) => Value::Bool(i != 0),
        (ColumnKind::Bool, ValueRef::Real(r)) => Value::Bool(r != 0.0),
        (ColumnKind::Int, ValueRef::Integer(i)) => Value::Int(i),
        (ColumnKind::Int, ValueRef::Real(r)) => Value::Int(r as i64),
        (ColumnKind::Float { decimals }, ValueRef::Integer(i)) => {
            Value::Float(round_to(i as f64, decimals))
        }
        (ColumnKind::Float { decimals }, ValueRef::Real(r)) => Value::Float(round_to(r, decimals)),
        (ColumnKind::Seconds3, ValueRef::Integer(ms)) => {
            Value::Float(round_to(ms as f64 / 1000.0, 3))
        }
        (ColumnKind::Seconds3, ValueRef::Real(ms)) => Value::Float(round_to(ms / 1000.0, 3)),
        (_, ValueRef::Integer(i)) if kind_is_text(kind) => Value::Text(i.to_string()),
        (_, ValueRef::Real(r)) if kind_is_text(kind) => Value::Text(r.to_string()),
        (_, ValueRef::Integer(i)) => Value::Int(i),
        (_, ValueRef::Real(r)) => Value::Float(r),
        (_, ValueRef::Text(t)) => Value::Text(String::from_utf8_lossy(t).into_owned()),
        (_, ValueRef::Blob(b)) => Value::Text(String::from_utf8_lossy(b).into_owned()),
    }
}

fn kind_is_text(kind: ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::Text | ColumnKind::Url | ColumnKind::Hash | ColumnKind::Timestamp
    )
}

/// Eine ausgewählte Spalte, bereit zur Ausführung.
enum ColExec {
    /// Ein SQL-Ausdruck in der SELECT-Liste (auch eine nummerierte Teilspalte: n-ter Wert als Ausdruck).
    Expr {
        idx: usize,
        kind: ColumnKind,
        /// Fehlende Werte als leerer Text statt `null` (nummerierte Teilspalten).
        empty_text: bool,
    },
    /// Mehrfachspalte: `count` Indizes, je Index alle Teilspalten in Reihenfolge.
    Series {
        base: &'static str,
        parts: Vec<PairedPart>,
        count: usize,
    },
}

/// Spaltennamen der Ausgabe (Mehrfachspalten nummeriert).
fn header(cols: &[SelectedColumn<'_>], plans: &[ColExec]) -> Vec<String> {
    let mut out = Vec::new();
    for (col, plan) in cols.iter().zip(plans) {
        match (&col.pick, plan) {
            (Pick::Numbered { name, .. }, _) => out.push(name.clone()),
            (Pick::Whole, ColExec::Series { base, parts, count }) => {
                for n in 1..=*count {
                    out.extend(parts.iter().map(|p| numbered_name(base, n, p.suffix)));
                }
            }
            (Pick::Whole, ColExec::Expr { .. }) => out.push(col.def.key.to_string()),
        }
    }
    out
}

/// Anzahl nummerierter Spalten einer Mehrfachspalte: Maximum über den Crawl und alle Teillisten,
/// mindestens 1.
fn multi_count(conn: &Connection, parts: &[PairedPart]) -> Result<usize, ViewError> {
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock},
    };
    static WIDTHS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    let seq = os_store::queries::change_seq(conn)?;
    let crawl: String = conn.query_row(
        "SELECT value FROM crawl_meta WHERE key='crawl_id'",
        [],
        |r| r.get(0),
    )?;
    let reset: String = conn.query_row(
        "SELECT COALESCE((SELECT value FROM crawl_meta WHERE key='reset_seq'),'0')",
        [],
        |r| r.get(0),
    )?;
    let analysis: String = conn.query_row("SELECT COALESCE(group_concat(step || ':' || state || ':' || COALESCE(finished_at,'') || ':' || COALESCE(config_hash,'')), '') FROM (SELECT * FROM analysis_steps ORDER BY step)", [], |r| r.get(0))?;
    let mut max_count = 1;
    for part in parts {
        let key = format!(
            "{}:{crawl}:{seq}:{reset}:{analysis}:{}",
            conn.path().unwrap_or_default(),
            part.list_sql
        );
        let cache = WIDTHS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(count) = cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .copied()
        {
            max_count = max_count.max(count);
            continue;
        }
        // The common series can aggregate their child table once, without probing every URL.
        let grouped = match part.list_sql {
            crate::tabs::internal::META_ROBOTS_LIST => Some("SELECT COUNT(*) AS n FROM directives WHERE source='meta_robots' AND applies=1 GROUP BY url_id"),
            crate::tabs::internal::X_ROBOTS_LIST => Some("SELECT COUNT(*) AS n FROM directives WHERE source='x_robots_tag' AND applies=1 GROUP BY url_id"),
            crate::tabs::internal::CANONICAL_LIST => Some("SELECT COUNT(*) AS n FROM canonicals WHERE source='html' GROUP BY url_id"),
            _ => None,
        };
        let sql = if let Some(grouped) = grouped {
            format!("SELECT COALESCE(MAX(n),0) FROM ({grouped})")
        } else {
            let correlated = part.list_sql.replace("?1", "u.id");
            format!("SELECT COALESCE(MAX((SELECT COUNT(*) FROM ({correlated}))), 0) FROM urls u")
        };
        let count: usize = conn.query_row(&sql, [], |r| r.get(0))?;
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.insert(key, count);
        max_count = max_count.max(count);
    }
    Ok(max_count)
}

fn no_series() -> ViewError {
    ViewError::Store(os_store::StoreError::InvalidInput(
        "Mehrfachspalte ohne Teillisten".to_string(),
    ))
}

/// Gemeinsamer Teil: Spaltenplan, SELECT-Liste.
struct Prepared {
    plans: Vec<ColExec>,
    select_list: String,
}

fn prepare_columns(
    conn: &Connection,
    built: &Built<'_>,
    count_multi: bool,
) -> Result<Prepared, ViewError> {
    let mut exprs: Vec<String> = vec!["u.id".to_string()];
    let mut plans = Vec::new();
    for col in &built.columns {
        let def = col.def;
        match (&col.pick, &def.sql) {
            (Pick::Whole, ColumnSql::Expr(e)) => {
                let idx = exprs.len();
                exprs.push(format!("({e}) AS __c{idx}"));
                plans.push(ColExec::Expr {
                    idx,
                    kind: def.kind,
                    empty_text: false,
                });
            }
            (Pick::Whole, _) => {
                let (base, parts) = series_parts(def).ok_or_else(no_series)?;
                let count = if count_multi {
                    multi_count(conn, &parts)?
                } else {
                    1
                };
                plans.push(ColExec::Series { base, parts, count });
            }
            (Pick::Numbered { n, part, .. }, _) => {
                let (_, parts) = series_parts(def).ok_or_else(no_series)?;
                let p = parts.get(*part).ok_or_else(no_series)?;
                let idx = exprs.len();
                exprs.push(format!("{} AS __c{idx}", multi_nth_expr(p.list_sql, *n)));
                plans.push(ColExec::Expr {
                    idx,
                    kind: p.kind,
                    empty_text: true,
                });
            }
        }
    }
    Ok(Prepared {
        plans,
        select_list: exprs.join(", "),
    })
}

pub(super) struct Projection {
    prepared: Prepared,
    /// Benannte Parameter der SELECT-Liste (z. B. `:readability_enabled`, `:near_ready`), bereits
    /// mit dem Dateizustand aufgelöst; ohne sie scheitert die Zeilenabfrage (A01-01).
    params: Vec<(String, Value)>,
    pub columns: Vec<String>,
}
impl Projection {
    pub fn build(
        conn: &Connection,
        built: &Built<'_>,
        params: Vec<(String, Value)>,
        ready: bool,
    ) -> Result<Self, ViewError> {
        let prepared = prepare_columns(conn, built, ready)?;
        let columns = header(&built.columns, &prepared.plans);
        Ok(Self {
            prepared,
            params,
            columns,
        })
    }
    pub fn rows(
        &self,
        conn: &Connection,
        ids: &[u32],
    ) -> Result<(Vec<UrlId>, Vec<Vec<Value>>), ViewError> {
        if ids.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let requested = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let order = ids
            .iter()
            .enumerate()
            .map(|(pos, id)| format!("WHEN {id} THEN {pos}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM {URLS_FROM} WHERE u.id IN ({requested}) ORDER BY CASE u.id {order} END",
            self.prepared.select_list
        ))?;
        let mut multi = multi_statements(conn, &self.prepared)?;
        let mut result = Vec::with_capacity(ids.len());
        let mut keys = Vec::with_capacity(ids.len());
        super::super::exec::bind_named(&mut stmt, &self.params, &[])?;
        let mut rows = stmt.raw_query();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let mut cells = Vec::new();
            for (plan, stmts) in self.prepared.plans.iter().zip(multi.iter_mut()) {
                match plan {
                    ColExec::Expr {
                        idx,
                        kind,
                        empty_text,
                    } => {
                        let v = cell(*kind, row.get_ref(*idx)?);
                        cells.push(match (v, empty_text) {
                            (Value::Null, true) => Value::Text(String::new()),
                            (v, _) => v,
                        });
                    }
                    ColExec::Series { parts, count, .. } => {
                        let mut values = Vec::new();
                        for (part, stmt) in parts.iter().zip(stmts.iter_mut()) {
                            let mut rows = stmt.query([id])?;
                            let mut list = Vec::new();
                            while let Some(r) = rows.next()? {
                                list.push(cell(part.kind, r.get_ref(0)?));
                            }
                            values.push(list);
                        }
                        for n in 0..*count {
                            for list in &values {
                                cells.push(
                                    list.get(n)
                                        .cloned()
                                        .unwrap_or_else(|| Value::Text(String::new())),
                                );
                            }
                        }
                    }
                }
            }
            keys.push(UrlId(id));
            result.push(cells);
        }
        Ok((keys, result))
    }
}
fn multi_statements<'a>(
    conn: &'a Connection,
    prepared: &Prepared,
) -> Result<Vec<Vec<Statement<'a>>>, ViewError> {
    prepared
        .plans
        .iter()
        .map(|p| match p {
            ColExec::Series { parts, .. } => parts
                .iter()
                .map(|part| Ok(conn.prepare(part.list_sql)?))
                .collect(),
            ColExec::Expr { .. } => Ok(Vec::new()),
        })
        .collect()
}
