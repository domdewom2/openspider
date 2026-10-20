//! Additiver Adapter für SQL-Views: Abgleich vollständig vor der ersten Ausgabe, ID-Menge in TEMP.
use super::{compile_ctx, search_budget, CompiledSearch, SearchCtx, SearchError};
use crate::views::{self, ViewQuery};
use crate::TabDef;
use os_model::{CancelToken, CrawlConfig, Value};
use os_store::CrawlFile;
use rusqlite::{types::Value as SqlValue, Statement, Transaction};
use std::time::Instant;

pub(super) fn bind(
    stmt: &mut Statement<'_>,
    params: &[(String, Value)],
) -> Result<(), rusqlite::Error> {
    for (name, value) in params {
        if let Some(i) = stmt.parameter_index(name)? {
            let value = match value {
                Value::Null => SqlValue::Null,
                Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
                Value::Int(n) => SqlValue::Integer(*n),
                Value::Float(n) => SqlValue::Real(*n),
                Value::Text(s) => SqlValue::Text(s.clone()),
            };
            stmt.raw_bind_parameter(i, value)?;
        }
    }
    Ok(())
}

/// Hält Snapshot und temporäre Treffer bis zum Ende der View-Abfrage; Fehler liefern keine Teilmenge.
pub struct PreparedSearch<'a> {
    file: &'a CrawlFile,
    table: String,
    _snapshot: Option<Transaction<'a>>,
}
impl PreparedSearch<'_> {
    /// SQL-Prädikat der vollständig vorbereiteten Unicode-Treffermenge, auch für Simple.
    pub fn sql(&self) -> String {
        format!("u.id IN (SELECT id FROM temp.{})", self.table)
    }
}
impl Drop for PreparedSearch<'_> {
    fn drop(&mut self) {
        let _ = self
            .file
            .conn()
            .execute_batch(&format!("DROP TABLE IF EXISTS temp.{}", self.table));
    }
}

impl CompiledSearch {
    /// Diese Wache während der SQL-Abfrage halten; ihr `sql()` nutzt auch bei Simple die Unicode-Treffermenge.
    pub fn prepare<'a>(
        &self,
        file: &'a CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
    ) -> Result<PreparedSearch<'a>, SearchError> {
        let deadline = self.bounded_deadline(file, deadline, cancel)?;
        self.prepare_where(file, deadline, cancel, "1", &[])
    }
    fn prepare_where<'a>(
        &self,
        file: &'a CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
        predicate: &str,
        params: &[(String, Value)],
    ) -> Result<PreparedSearch<'a>, SearchError> {
        super::eval::check(deadline, cancel)?;
        let snapshot = file
            .conn()
            .is_autocommit()
            .then(|| file.conn().unchecked_transaction())
            .transpose()?;
        let ids = self.matches_where(file, deadline, cancel, predicate, params)?;
        let table = self.table_name();
        file.conn().execute_batch(&format!(
            "CREATE TEMP TABLE {table}(id INTEGER PRIMARY KEY) WITHOUT ROWID"
        ))?;
        let guard = PreparedSearch {
            file,
            table,
            _snapshot: snapshot,
        };
        let _watchdog = super::eval::Watchdog::start(file, deadline, cancel);
        let mut insert = file
            .conn()
            .prepare(&format!("INSERT INTO temp.{}(id) VALUES (?1)", guard.table))?;
        for id in ids {
            super::eval::check(deadline, cancel)?;
            if let Err(error) = insert.execute([id]) {
                super::eval::check(deadline, cancel)?;
                return Err(SearchError::Sql(error));
            }
        }
        drop(insert);
        super::eval::check(deadline, cancel)?;
        Ok(guard)
    }
}

/// Kernseitiger Initialisierungsschritt für query/stream; die eigentliche SQL-View bleibt unverändert.
/// K05 kann für UI-Operationen `CompiledSearch::matches_bitmap` mit deren CancelToken aufrufen.
pub fn prepare_view<'a>(
    tabs: &[TabDef],
    file: &'a CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
) -> Result<Option<PreparedSearch<'a>>, SearchError> {
    let Some(spec) = &q.search else {
        return Ok(None);
    };
    let tab = tabs
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(&q.tab))
        .ok_or_else(|| SearchError::InvalidRule("unknown-tab".into()))?;
    let visible: Vec<_> = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .map(|c| c.key)
        .collect();
    let compiled = compile_ctx(
        spec,
        &SearchCtx {
            tab,
            columns: &tab.columns,
            visible: &visible,
        },
    )?;
    let compiled = compile_visible(compiled, spec, q, tab)?;
    let mut unsearched = q.clone();
    unsearched.search = None;
    let plan = views::plan_in(tabs, &unsearched, cfg)
        .map_err(|e| SearchError::InvalidRule(e.to_string()))?;
    let predicate = format!("({}) AND ({})", plan.population_sql, plan.filter_sql);
    let count: u64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))?;
    let deadline = Instant::now() + search_budget(count, compiled.has_regex());
    compiled
        .prepare_where(
            file,
            deadline,
            &CancelToken::new(),
            &predicate,
            &plan.params,
        )
        .map(Some)
}

/// Zusätzlicher Cursor-Bestandteil: Regelbaum, Schreibweise und sichtbare Suchspalten.
pub fn query_identity(q: &ViewQuery, tab: &TabDef) -> String {
    if q.search.is_none() {
        return String::new();
    }
    let visible: Vec<_> = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .map(|c| c.key)
        .collect();
    crate::cursor::query_hash(&[
        &format!("{:?}", q.search),
        &format!("{visible:?}"),
        &format!("{:?}", q.search_visible),
    ])
}

/// Führt Klartext durch dieselbe Unicode-Auswertung wie Regeln. Der öffentliche M1-SQL-Plan
/// von Simple bleibt für reine Plan-Konsumenten kompatibel (ASCII-lower-Fallback).
pub fn prepared_query(q: &ViewQuery) -> ViewQuery {
    let mut prepared = q.clone();
    if let Some(views::SearchSpec::Simple(text)) = &q.search {
        prepared.search = Some(views::SearchSpec::Rules {
            tree: super::RuleNode::Rule(super::Rule {
                column: "Address".into(),
                op: super::RuleOp::Contains,
                value: text.clone(),
            }),
            case_sensitive: false,
        });
    }
    prepared
}

/// Erweitert ausschließlich Such-Cursor; bestehende Cursor ohne Suche bleiben gleich.
pub fn extend_query_hash(hash: String, q: &ViewQuery, tab: &TabDef) -> String {
    if q.search.is_none() {
        hash
    } else {
        crate::cursor::query_hash(&[&hash, &query_identity(q, tab)])
    }
}

/// Zusätzlicher Initialisierungsschritt: Ausgabe/Export und sichtbare Suchspalten sind unabhängig.
pub fn compile_visible(
    compiled: CompiledSearch,
    spec: &views::SearchSpec,
    q: &ViewQuery,
    tab: &TabDef,
) -> Result<CompiledSearch, SearchError> {
    let Some(visible) = &q.search_visible else {
        return Ok(compiled);
    };
    let visible: Vec<_> = visible.iter().map(String::as_str).collect();
    compile_ctx(
        spec,
        &SearchCtx {
            tab,
            columns: &tab.columns,
            visible: &visible,
        },
    )
}

/// Additiv (K2-61, M2-K05): die Suche einer View-Sitzung als Bitmap-Einschränkung. Wertet `prepared_query`
/// (Unicode, auch für Klartext) über Population und Filter mit der Abbruchmarke des Aufbaus aus; die Sitzung
/// selbst führt dann kein Such-SQL aus (keine temporäre Tabelle auf der Pool-Verbindung nötig).
pub struct SearchFilter {
    query: ViewQuery,
    cfg: CrawlConfig,
    cancel: CancelToken,
}
impl SearchFilter {
    /// `None` ohne Suche in `q`.
    pub fn new(q: &ViewQuery, cfg: &CrawlConfig, cancel: CancelToken) -> Option<SearchFilter> {
        q.search.as_ref()?;
        Some(SearchFilter {
            query: prepared_query(q),
            cfg: cfg.clone(),
            cancel,
        })
    }
}
impl super::FilterSource for SearchFilter {
    fn key(&self) -> String {
        crate::cursor::query_hash(&[
            "search",
            &self.query.tab,
            &self.query.filter,
            &format!("{:?}", self.query.search),
            &format!("{:?}", self.query.search_visible),
        ])
    }
    fn bitmap(&self, file: &CrawlFile) -> Result<roaring::RoaringBitmap, crate::ViewError> {
        let q = &self.query;
        let spec = q.search.as_ref().expect("constructed with a search");
        let tabs = crate::tabs::configured_for_file(&self.cfg, file);
        let tab = tabs
            .iter()
            .find(|t| t.id.eq_ignore_ascii_case(&q.tab))
            .ok_or_else(|| SearchError::InvalidRule("unknown-tab".into()))?;
        let visible: Vec<_> = tab
            .columns
            .iter()
            .filter(|c| c.default_visible)
            .map(|c| c.key)
            .collect();
        let compiled = compile_ctx(
            spec,
            &SearchCtx {
                tab,
                columns: &tab.columns,
                visible: &visible,
            },
        )?;
        let compiled = compile_visible(compiled, spec, q, tab)?;
        let mut unsearched = q.clone();
        unsearched.search = None;
        let plan = views::plan_in(&tabs, &unsearched, &self.cfg)?;
        let predicate = format!("({}) AND ({})", plan.population_sql, plan.filter_sql);
        let far = Instant::now() + std::time::Duration::from_secs(24 * 3600);
        let deadline = compiled.bounded_deadline(file, far, &self.cancel)?;
        Ok(compiled.matches_where(file, deadline, &self.cancel, &predicate, &plan.params)?)
    }
}
