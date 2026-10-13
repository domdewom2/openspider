//! Ausführung eines [`ViewPlan`]: Seite abfragen (Offset oder Keyset-Cursor) und Zeilen streamen.
//!
//! Zellwerte entstehen nach `ColumnKind`: `Bool` → `Value::Bool`, `Float { decimals }` gerundet,
//! `Seconds3` rechnet Millisekunden in Sekunden (3 Stellen) um, `Text`/`Url`/`Hash`/`Timestamp` bleiben
//! Text. Mehrfachspalten (TECH-004) erhalten so viele nummerierte Spalten wie das Maximum über die
//! gesamte Ansicht (mindestens eine); fehlende Werte sind leere Texte, nie `null`. Verschränkte Mehrfachspalten
//! (`ColumnSql::MultiPaired`) liefern je Index alle Teilspalten hintereinander; wird nur eine nummerierte
//! Teilspalte gewählt (`Meta Robots 1`), entsteht genau diese eine Spalte.

use os_model::{UrlId, Value};
use os_store::{queries, CrawlFile};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, Statement};
use serde_json::Value as Json;

use super::build::{
    multi_nth_expr, numbered_name, series_parts, where_sql, Built, Pick, SelectedColumn, SortTerm,
};
use super::{PageStart, ViewPage, ViewQuery};
use crate::cursor::Cursor;
use crate::defs::{ColumnKind, ColumnSql, PairedPart};
use crate::error::ViewError;

/// Größte erlaubte Seitengröße.
pub(super) const MAX_LIMIT: u32 = 500;

pub(super) fn to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Int(i) => SqlValue::Integer(*i),
        Value::Float(f) => SqlValue::Real(*f),
        Value::Text(s) => SqlValue::Text(s.clone()),
    }
}

/// Bindet die benannten Parameter, die in der Anweisung vorkommen (andere werden übergangen).
pub(super) fn bind_named(
    stmt: &mut Statement<'_>,
    params: &[(String, Value)],
    extra: &[(String, SqlValue)],
) -> Result<(), rusqlite::Error> {
    for (name, value) in params {
        if let Some(idx) = stmt.parameter_index(name)? {
            stmt.raw_bind_parameter(idx, to_sql(value))?;
        }
    }
    for (name, value) in extra {
        if let Some(idx) = stmt.parameter_index(name)? {
            stmt.raw_bind_parameter(idx, value)?;
        }
    }
    Ok(())
}

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

fn raw_to_json(v: ValueRef<'_>) -> Json {
    match v {
        ValueRef::Integer(i) => Json::from(i),
        ValueRef::Real(r) => serde_json::Number::from_f64(r).map_or(Json::Null, Json::Number),
        ValueRef::Text(t) => Json::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Null | ValueRef::Blob(_) => Json::Null,
    }
}

fn json_to_sql(v: &Json) -> SqlValue {
    match v {
        Json::Number(n) => n.as_i64().map_or_else(
            || SqlValue::Real(n.as_f64().unwrap_or(0.0)),
            SqlValue::Integer,
        ),
        Json::String(s) => SqlValue::Text(s.clone()),
        Json::Bool(b) => SqlValue::Integer(i64::from(*b)),
        _ => SqlValue::Null,
    }
}

fn json_is_empty(v: &Json) -> bool {
    matches!(v, Json::Null) || matches!(v, Json::String(s) if s.is_empty())
}

/// Bedingung „Zeile liegt hinter dem Cursor“ im vollständigen Sortiertupel (Schlüssel → Address → UrlId).
fn after_condition(
    sort: &[SortTerm],
    cur: &Cursor,
) -> Result<(String, Vec<(String, SqlValue)>), ViewError> {
    if cur.last_key.len() != sort.len() {
        return Err(ViewError::ForeignCursor);
    }
    let mut atoms: Vec<(String, String)> = Vec::new(); // (größer, gleich)
    let mut params: Vec<(String, SqlValue)> = Vec::new();
    for (i, (term, last)) in sort.iter().zip(&cur.last_key).enumerate() {
        let empty = json_is_empty(last);
        let e = term.empty_expr();
        let pe = format!(":ce{i}");
        params.push((pe.clone(), SqlValue::Integer(i64::from(empty))));
        atoms.push((format!("{e} > {pe}"), format!("{e} = {pe}")));
        if !empty {
            let v = term.value_expr();
            let pv = format!(":cv{i}");
            params.push((pv.clone(), json_to_sql(last)));
            let op = if term.desc { "<" } else { ">" };
            atoms.push((format!("{v} {op} {pv}"), format!("{v} = {pv}")));
        }
    }
    if !sort.is_empty() {
        params.push((":ca".to_string(), SqlValue::Text(cur.last_address.clone())));
        atoms.push(("u.address > :ca".to_string(), "u.address = :ca".to_string()));
    }
    params.push((":ci".to_string(), SqlValue::Integer(cur.last_id)));
    atoms.push(("u.id > :ci".to_string(), "u.id = :ci".to_string()));

    let mut disjuncts = Vec::new();
    for k in 0..atoms.len() {
        let mut parts: Vec<&str> = atoms[..k].iter().map(|(_, eq)| eq.as_str()).collect();
        parts.push(atoms[k].0.as_str());
        disjuncts.push(format!("({})", parts.join(" AND ")));
    }
    Ok((format!("({})", disjuncts.join(" OR ")), params))
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

/// Anzahl nummerierter Spalten einer Mehrfachspalte: Maximum über die Ansicht und alle Teillisten,
/// mindestens 1.
fn multi_count(
    conn: &Connection,
    built: &Built<'_>,
    parts: &[PairedPart],
) -> Result<usize, ViewError> {
    let mut max_count: i64 = 0;
    for part in parts {
        let correlated = part.list_sql.replace("?1", "u.id");
        let sql = format!(
            "SELECT COALESCE(MAX(c), 0) FROM (SELECT (SELECT COUNT(*) FROM ({correlated})) AS c \
             FROM urls u WHERE {})",
            where_sql(&built.plan)
        );
        let mut stmt = conn.prepare(&sql)?;
        bind_named(&mut stmt, &built.plan.params, &[])?;
        let mut rows = stmt.raw_query();
        let max: i64 = match rows.next()? {
            Some(r) => r.get(0)?,
            None => 0,
        };
        max_count = max_count.max(max);
    }
    Ok((max_count.max(1)) as usize)
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
    nk: usize,
}

fn prepare_columns(
    conn: &Connection,
    built: &Built<'_>,
    count_multi: bool,
) -> Result<Prepared, ViewError> {
    let nk = built.sort.len();
    let mut exprs: Vec<String> = vec!["u.id".to_string()];
    for (i, t) in built.sort.iter().enumerate() {
        exprs.push(format!("({}) AS __k{i}", t.raw));
    }
    exprs.push("u.address AS __a".to_string());
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
                    multi_count(conn, built, &parts)?
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
        nk,
    })
}

/// Werte einer Teilliste einer Mehrfachspalte für eine URL.
fn multi_values(
    stmt: &mut Statement<'_>,
    kind: ColumnKind,
    id: i64,
) -> Result<Vec<Value>, ViewError> {
    let mut rows = stmt.query([id])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        out.push(cell(kind, r.get_ref(0)?));
    }
    Ok(out)
}

/// Eine Zeile samt Zeilenschlüssel und Rohwerten der Sortierschlüssel.
struct RawRow {
    id: i64,
    keys: Vec<Json>,
    address: String,
    cells: Vec<Value>,
}

fn read_row(
    row: &rusqlite::Row<'_>,
    prepared: &Prepared,
    multi_stmts: &mut [Vec<Statement<'_>>],
) -> Result<RawRow, ViewError> {
    let id: i64 = row.get(0)?;
    let mut keys = Vec::with_capacity(prepared.nk);
    for i in 0..prepared.nk {
        keys.push(raw_to_json(row.get_ref(1 + i)?));
    }
    let address: String = row.get(1 + prepared.nk)?;
    let mut cells = Vec::new();
    for (plan, stmts) in prepared.plans.iter().zip(multi_stmts.iter_mut()) {
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
                let mut values: Vec<Vec<Value>> = Vec::with_capacity(parts.len());
                for (part, stmt) in parts.iter().zip(stmts.iter_mut()) {
                    values.push(multi_values(stmt, part.kind, id)?);
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
    Ok(RawRow {
        id,
        keys,
        address,
        cells,
    })
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

fn crawl_id(file: &CrawlFile) -> Result<String, ViewError> {
    Ok(file.get_meta("crawl_id")?.unwrap_or_default())
}

/// Eine Seite abfragen. `ready = false`: der Filter hat einen Zustand statt Zeilen (ONPAGE-007).
pub(super) fn query_page(
    file: &CrawlFile,
    built: &Built<'_>,
    q: &ViewQuery,
    ready: bool,
) -> Result<ViewPage, ViewError> {
    if q.limit == 0 || q.limit > MAX_LIMIT {
        return Err(ViewError::InvalidLimit(q.limit));
    }
    let conn = file.conn();
    // Lesetransaktion: Zähler, Seite und Version stammen aus demselben Stand.
    let tx = conn.unchecked_transaction()?;
    let version = queries::change_seq(conn)?;

    let cursor = match &q.start {
        PageStart::Cursor(c) => {
            let cur = Cursor::decode(c)?;
            cur.check(&crawl_id(file)?, &built.query_hash)?;
            Some(cur)
        }
        PageStart::Offset(_) => None,
    };

    if !ready {
        let prepared = prepare_columns(conn, built, false)?;
        let columns = header(&built.columns, &prepared.plans);
        drop(tx);
        return Ok(ViewPage {
            columns,
            rows: Vec::new(),
            row_keys: Vec::new(),
            total: 0,
            next_cursor: None,
            version,
        });
    }

    let where_base = where_sql(&built.plan);
    let total: i64 = {
        let mut stmt = conn.prepare(&format!("SELECT COUNT(*) FROM urls u WHERE {where_base}"))?;
        bind_named(&mut stmt, &built.plan.params, &[])?;
        let mut rows = stmt.raw_query();
        match rows.next()? {
            Some(r) => r.get(0)?,
            None => 0,
        }
    };

    let prepared = prepare_columns(conn, built, true)?;
    let columns = header(&built.columns, &prepared.plans);

    let mut where_page = where_base;
    let mut extra: Vec<(String, SqlValue)> = Vec::new();
    if let Some(cur) = &cursor {
        let (cond, params) = after_condition(&built.sort, cur)?;
        where_page = format!("{where_page} AND {cond}");
        extra = params;
    }
    let offset = match q.start {
        PageStart::Offset(n) => n.min(i64::MAX as u64),
        PageStart::Cursor(_) => 0,
    };
    let sql = format!(
        "SELECT {} FROM urls u WHERE {where_page} {} LIMIT {} OFFSET {offset}",
        prepared.select_list,
        built.plan.order_by,
        u64::from(q.limit) + 1
    );
    let mut stmt = conn.prepare(&sql)?;
    bind_named(&mut stmt, &built.plan.params, &extra)?;
    let mut multi = multi_statements(conn, &prepared)?;

    let mut rows_out: Vec<Vec<Value>> = Vec::new();
    let mut keys_out: Vec<UrlId> = Vec::new();
    let mut last: Option<(Vec<Json>, String, i64)> = None;
    let mut has_more = false;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        if rows_out.len() as u32 == q.limit {
            has_more = true;
            break;
        }
        let r = read_row(row, &prepared, &mut multi)?;
        keys_out.push(UrlId(r.id));
        last = Some((r.keys, r.address, r.id));
        rows_out.push(r.cells);
    }
    drop(rows);
    drop(stmt);
    drop(multi);

    let next_cursor = match (has_more, last) {
        (true, Some((last_key, last_address, last_id))) => Some(
            Cursor {
                v: crate::cursor::CURSOR_VERSION,
                crawl_id: crawl_id(file)?,
                query_hash: built.query_hash.clone(),
                last_key,
                last_address,
                last_id,
            }
            .encode(),
        ),
        _ => None,
    };
    drop(tx);
    Ok(ViewPage {
        columns,
        rows: rows_out,
        row_keys: keys_out,
        total: total as u64,
        next_cursor,
        version,
    })
}

/// Alle Zeilen der Ansicht nacheinander an `sink` geben; liefert die Spaltennamen.
pub(super) fn stream_rows(
    file: &CrawlFile,
    built: &Built<'_>,
    ready: bool,
    sink: &mut dyn FnMut(&[Value]) -> Result<(), ViewError>,
) -> Result<Vec<String>, ViewError> {
    let conn = file.conn();
    let tx = conn.unchecked_transaction()?;
    let prepared = prepare_columns(conn, built, ready)?;
    let columns = header(&built.columns, &prepared.plans);
    if !ready {
        drop(tx);
        return Ok(columns);
    }
    let sql = format!(
        "SELECT {} FROM urls u WHERE {} {}",
        prepared.select_list,
        where_sql(&built.plan),
        built.plan.order_by
    );
    let mut stmt = conn.prepare(&sql)?;
    bind_named(&mut stmt, &built.plan.params, &[])?;
    let mut multi = multi_statements(conn, &prepared)?;
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        let r = read_row(row, &prepared, &mut multi)?;
        sink(&r.cells)?;
    }
    drop(rows);
    drop(stmt);
    drop(multi);
    drop(tx);
    Ok(columns)
}
