//! Begrenzter Rust-Abgleich; Zellwerte werden chargenweise gelesen, niemals crawlweit gesammelt.
use std::sync::Arc;
use std::time::{Duration, Instant};

use fancy_regex::{Regex, RegexBuilder};
use os_model::{CancelToken, Value};
use os_store::CrawlFile;
use roaring::RoaringBitmap;
use rusqlite::types::ValueRef;

use super::{folding::fold, CompiledSearch, Rule, RuleNode, RuleOp};
use crate::defs::{meta_for_kind, ColumnDef, ColumnKind, ColumnSql, SearchOps};
use crate::error::ViewError;
use crate::views::{SearchSpec, URLS_FROM};

/// Kontext der Suche. `visible` enthält die tatsächlich sichtbaren Spalten.
pub struct SearchCtx<'a> {
    pub tab: &'a crate::TabDef,
    pub columns: &'a [ColumnDef],
    pub visible: &'a [&'a str],
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
    #[error("Suche abgebrochen")]
    Cancelled,
    #[error("Ausdruck zu aufwendig: Zeitlimit der Suche erreicht")]
    Deadline,
    #[error("Ausdruck zu aufwendig: Regex-Schrittlimit erreicht")]
    StepLimit,
    #[error("ungültige Suchregel: {0}")]
    InvalidRule(String),
    #[error("ungültige Regex: {0}")]
    InvalidRegex(String),
    #[error(transparent)]
    Store(#[from] os_store::StoreError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}

const BACKTRACK_LIMIT: usize = 1_000_000;
const BATCH_SIZE: usize = 8192;

/// Harte Grenze K2-38: mindestens 2 s, bei großen Ansichten das UI-141-Budget.
pub fn search_budget(rows: u64, regex: bool) -> Duration {
    let (small, large) = if regex { (3.0, 10.0) } else { (1.0, 3.0) };
    let fraction = ((rows as f64 - 1_000_000.0) / 4_000_000.0).clamp(0.0, 1.0);
    let seconds = if rows < 1_000_000 {
        small * rows as f64 / 1_000_000.0
    } else {
        small + fraction * (large - small)
    };
    Duration::from_secs_f64(seconds.max(2.0))
}

fn regex(pattern: &str, case_sensitive: bool) -> Result<Regex, SearchError> {
    if pattern.len() > 65_536 {
        return Err(SearchError::InvalidRegex("pattern-too-long".into()));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .map_err(|e| SearchError::InvalidRegex(e.to_string()))
}

/// Gleiche Engine und Grenzen wie in der Auswertung; kein probeweiser Abgleich.
pub fn validate_regex(pattern: &str) -> Result<(), SearchError> {
    regex(pattern, false).map(|_| ())
}

#[derive(Debug, Clone)]
struct Atom {
    sql: String,
    list: bool,
    kind: ColumnKind,
    op: RuleOp,
    value: String,
    number: Option<f64>,
    regex: Option<Arc<Regex>>,
}
impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        self.sql == other.sql
            && self.list == other.list
            && self.kind == other.kind
            && self.op == other.op
            && self.value == other.value
            && self.number == other.number
            && self.regex.as_ref().map(|r| r.as_str()) == other.regex.as_ref().map(|r| r.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    And(Vec<Node>),
    Or(Vec<Node>),
    Not(Box<Node>),
    Atom(usize),
}
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Program {
    node: Node,
    atoms: Vec<Atom>,
    case_sensitive: bool,
}

fn invalid(message: impl Into<String>) -> SearchError {
    SearchError::InvalidRule(message.into())
}
fn is_number(kind: ColumnKind) -> bool {
    matches!(
        kind,
        ColumnKind::Int | ColumnKind::Float { .. } | ColumnKind::Seconds3
    )
}
fn negative(op: RuleOp) -> bool {
    matches!(
        op,
        RuleOp::NotContains | RuleOp::NotEquals | RuleOp::NotRegex
    )
}

fn numbered(name: &str, base: &str, suffix: &str) -> Option<usize> {
    if !name.get(..base.len())?.eq_ignore_ascii_case(base) {
        return None;
    }
    let rest = name[base.len()..].strip_prefix(' ')?;
    let (number, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    if !tail.eq_ignore_ascii_case(suffix)
        || number.is_empty()
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    number.parse::<usize>().ok().filter(|n| *n > 0)
}

fn column(
    ctx: &SearchCtx<'_>,
    name: &str,
) -> Result<(String, bool, ColumnKind, SearchOps), SearchError> {
    for def in ctx.columns {
        let whole = def.key.eq_ignore_ascii_case(name.trim());
        let ops = if def.search_ops.is_none() {
            meta_for_kind(def.kind).2
        } else {
            def.search_ops
        };
        let parts: Vec<(&str, &str, &str, ColumnKind)> = match &def.sql {
            ColumnSql::Expr(sql) if whole => return Ok((sql.to_string(), false, def.kind, ops)),
            ColumnSql::Expr(_) => continue,
            ColumnSql::Multi { base, list_sql } | ColumnSql::MultiCrawlWide { base, list_sql } => {
                vec![(base, "", list_sql, def.kind)]
            }
            ColumnSql::MultiPaired { base, parts } => parts
                .iter()
                .map(|p| (*base, p.suffix, p.list_sql, p.kind))
                .collect(),
        };
        for (base, suffix, sql, kind) in parts {
            let n = numbered(name.trim(), base, suffix);
            if !whole && n.is_none() {
                continue;
            }
            let correlated = sql.replace("?1", "u.id");
            // Der Alias `value` stammt aus den Listenverträgen von ColumnSql.
            let expr = if let Some(n) = n.filter(|_| !whole) {
                format!(
                    "(WITH os_search_values(value) AS ({correlated}) SELECT value FROM os_search_values LIMIT 1 OFFSET {})",
                    n - 1
                )
            } else {
                format!("(WITH os_search_values(value) AS ({correlated}) SELECT json_group_array(value) FROM os_search_values)")
            };
            return Ok((expr, whole, kind, ops));
        }
    }
    Err(invalid(format!("unknown-column: {name}")))
}

fn atom(rule: &Rule, ctx: &SearchCtx<'_>, sensitive: bool) -> Result<Atom, SearchError> {
    let (sql, list, kind, ops) = column(ctx, &rule.column)?;
    let required = match rule.op {
        RuleOp::Contains | RuleOp::NotContains => SearchOps::CONTAINS,
        RuleOp::Equals | RuleOp::NotEquals => {
            if kind == ColumnKind::Bool {
                SearchOps::BOOL
            } else {
                SearchOps::EQUALS
            }
        }
        RuleOp::StartsWith | RuleOp::EndsWith => SearchOps::AFFIX,
        RuleOp::Regex | RuleOp::NotRegex => SearchOps::REGEX,
        RuleOp::Greater | RuleOp::Less | RuleOp::GreaterOrEqual | RuleOp::LessOrEqual => {
            SearchOps::COMPARE
        }
        RuleOp::IsEmpty | RuleOp::IsNotEmpty => SearchOps::EMPTY,
    };
    if !ops.has(required) {
        return Err(invalid(format!("operator-not-supported: {}", rule.column)));
    }
    let unary = matches!(rule.op, RuleOp::IsEmpty | RuleOp::IsNotEmpty);
    let mut value = rule.value.clone();
    let number = if !unary && is_number(kind) {
        let number: f64 = value
            .trim()
            .replace(',', ".")
            .parse()
            .map_err(|_| invalid("expected-number"))?;
        if !number.is_finite() {
            return Err(invalid("expected-finite-number"));
        }
        Some(number)
    } else {
        None
    };
    if kind == ColumnKind::Bool && !unary {
        value = match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => "1".into(),
            "false" | "0" => "0".into(),
            _ => return Err(invalid("expected-boolean")),
        };
    }
    let compiled_regex = matches!(rule.op, RuleOp::Regex | RuleOp::NotRegex)
        .then(|| regex(&value, sensitive).map(Arc::new))
        .transpose()?;
    if !sensitive && compiled_regex.is_none() {
        value = fold(&value);
    }
    Ok(Atom {
        sql,
        list,
        kind,
        op: rule.op,
        value,
        number,
        regex: compiled_regex,
    })
}

fn build(
    tree: &RuleNode,
    ctx: &SearchCtx<'_>,
    p: &mut Program,
    depth: usize,
) -> Result<Node, SearchError> {
    if depth >= 64 || p.atoms.len() >= 4096 {
        return Err(invalid("query-too-complex"));
    }
    match tree {
        RuleNode::And(children) | RuleNode::Or(children) => {
            if children.is_empty() || children.len() > 4096 {
                return Err(invalid("invalid-group-size"));
            }
            let nodes = children
                .iter()
                .map(|n| build(n, ctx, p, depth + 1))
                .collect::<Result<_, _>>()?;
            Ok(if matches!(tree, RuleNode::And(_)) {
                Node::And(nodes)
            } else {
                Node::Or(nodes)
            })
        }
        RuleNode::Not(child) => Ok(Node::Not(Box::new(build(child, ctx, p, depth + 1)?))),
        RuleNode::Rule(rule) if rule.column.is_empty() || rule.column == "*" => {
            if ctx.visible.is_empty() {
                return Err(invalid("no-visible-columns"));
            }
            // Positive All-visible-Regeln: irgendeine Spalte; negative: keine positive Spalte.
            let mut nodes = Vec::new();
            for name in ctx.visible {
                let rule = Rule {
                    column: name.to_string(),
                    ..rule.clone()
                };
                match atom(&rule, ctx, p.case_sensitive) {
                    Ok(a) => {
                        let i = p.atoms.len();
                        p.atoms.push(a);
                        nodes.push(Node::Atom(i));
                    }
                    Err(SearchError::InvalidRule(e))
                        if e.starts_with("operator-not-supported:") => {}
                    Err(e) => return Err(e),
                }
            }
            if nodes.is_empty() {
                return Err(invalid("operator-not-supported: visible-columns"));
            }
            Ok(if negative(rule.op) || rule.op == RuleOp::IsEmpty {
                Node::And(nodes)
            } else {
                Node::Or(nodes)
            })
        }
        RuleNode::Rule(rule) => {
            let a = atom(rule, ctx, p.case_sensitive)?;
            let i = p.atoms.len();
            p.atoms.push(a);
            Ok(Node::Atom(i))
        }
    }
}

/// Kompiliert typgeprüft gegen die Spalten des aktiven Tabs. SQL nutzt eine vorbereitete temporäre ID-Menge;
/// [`CompiledSearch::prepare`] bzw. die View-Aufrufstellen bereiten sie vor.
pub fn compile_ctx(spec: &SearchSpec, ctx: &SearchCtx<'_>) -> Result<CompiledSearch, SearchError> {
    let (tree, case_sensitive) = match spec {
        SearchSpec::Simple(text) => (
            RuleNode::Rule(Rule {
                column: "Address".into(),
                op: RuleOp::Contains,
                value: text.clone(),
            }),
            false,
        ),
        SearchSpec::Rules {
            tree,
            case_sensitive,
        } => (tree.clone(), *case_sensitive),
    };
    let mut program = Program {
        node: Node::And(Vec::new()),
        atoms: Vec::new(),
        case_sensitive,
    };
    program.node = build(&tree, ctx, &mut program, 0)?;
    let identity = crate::cursor::query_hash(&[&format!("{program:?}")]);
    // Der historische öffentliche SQL-Plan für Simple bleibt kompatibel. Ausführende Views
    // promovieren Simple über prepared_query zu Rules und nutzen die Unicode-Bitmap.
    let (sql, params) = if let SearchSpec::Simple(text) = spec {
        (
            "instr(lower(u.address), lower(:q)) > 0".into(),
            vec![(":q".into(), Value::Text(text.clone()))],
        )
    } else {
        (
            format!("u.id IN (SELECT id FROM temp.os_search_{identity})"),
            Vec::new(),
        )
    };
    Ok(CompiledSearch {
        sql,
        params,
        program,
    })
}

/// Ohne Tab-Kontext gelten die konfigurationsfreien Standardspalten von Internal.
pub fn compile(tree: &RuleNode, case_sensitive: bool) -> Result<CompiledSearch, ViewError> {
    let tabs = crate::tabs::all();
    let tab = tabs
        .iter()
        .find(|t| t.id == "Internal")
        .ok_or_else(|| invalid("missing-internal-tab"))?;
    let visible: Vec<_> = tab
        .columns
        .iter()
        .filter(|c| c.default_visible)
        .map(|c| c.key)
        .collect();
    Ok(compile_ctx(
        &SearchSpec::Rules {
            tree: tree.clone(),
            case_sensitive,
        },
        &SearchCtx {
            tab,
            columns: &tab.columns,
            visible: &visible,
        },
    )?)
}

pub(super) fn check(deadline: Instant, cancel: &CancelToken) -> Result<(), SearchError> {
    if cancel.is_cancelled() {
        return Err(SearchError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(SearchError::Deadline);
    }
    Ok(())
}

fn text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
impl Atom {
    fn scalar(&self, value: &serde_json::Value, sensitive: bool) -> Result<bool, SearchError> {
        let empty = value.is_null() || value.as_str() == Some("");
        if self.op == RuleOp::IsEmpty {
            return Ok(empty);
        }
        if self.op == RuleOp::IsNotEmpty {
            return Ok(!empty);
        }
        let positive = if let Some(number) = self.number {
            // Eine fehlende Zahl ist keine 0.
            value.as_f64().is_some_and(|mut n| {
                if self.kind == ColumnKind::Seconds3 {
                    n = (n / 1000.0 * 1000.0).round() / 1000.0;
                }
                match self.op {
                    RuleOp::Equals | RuleOp::NotEquals => n == number,
                    RuleOp::Greater => n > number,
                    RuleOp::Less => n < number,
                    RuleOp::GreaterOrEqual => n >= number,
                    RuleOp::LessOrEqual => n <= number,
                    _ => false,
                }
            })
        } else if let Some(regex) = &self.regex {
            regex.is_match(&text(value)).map_err(|e| match e {
                fancy_regex::Error::RuntimeError(
                    fancy_regex::RuntimeError::BacktrackLimitExceeded,
                ) => SearchError::StepLimit,
                _ => SearchError::InvalidRegex(e.to_string()),
            })?
        } else {
            let mut s = text(value);
            if !sensitive {
                s = fold(&s);
            }
            match self.op {
                RuleOp::Contains | RuleOp::NotContains => s.contains(&self.value),
                RuleOp::Equals | RuleOp::NotEquals => s == self.value,
                RuleOp::StartsWith => s.starts_with(&self.value),
                RuleOp::EndsWith => s.ends_with(&self.value),
                RuleOp::Greater => s > self.value,
                RuleOp::Less => s < self.value,
                RuleOp::GreaterOrEqual => s >= self.value,
                RuleOp::LessOrEqual => s <= self.value,
                _ => false,
            }
        };
        Ok(if negative(self.op) {
            !positive
        } else {
            positive
        })
    }
    fn matches(&self, value: &serde_json::Value, sensitive: bool) -> Result<bool, SearchError> {
        if !self.list {
            return self.scalar(value, sensitive);
        }
        let values = value
            .as_array()
            .ok_or_else(|| invalid("invalid-list-cell"))?;
        if values.is_empty() {
            return self.scalar(&serde_json::Value::Null, sensitive);
        }
        let all = negative(self.op) || self.op == RuleOp::IsEmpty;
        for value in values {
            let matched = self.scalar(value, sensitive)?;
            if all && !matched {
                return Ok(false);
            }
            if !all && matched {
                return Ok(true);
            }
        }
        Ok(all)
    }
}
impl Program {
    fn matches(
        &self,
        node: &Node,
        values: &[serde_json::Value],
        deadline: Instant,
        cancel: &CancelToken,
    ) -> Result<bool, SearchError> {
        check(deadline, cancel)?;
        match node {
            Node::And(nodes) => {
                for n in nodes {
                    if !self.matches(n, values, deadline, cancel)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Node::Or(nodes) => {
                for n in nodes {
                    if self.matches(n, values, deadline, cancel)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Node::Not(n) => Ok(!self.matches(n, values, deadline, cancel)?),
            Node::Atom(i) => self.atoms[*i].matches(&values[*i], self.case_sensitive),
        }
    }
}

/// Interruptiert auch einen laufenden SQLite-Schritt; kein Warten auf eine Ergebniszeile nötig.
pub(super) struct Watchdog {
    done: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Watchdog {
    pub(super) fn start(file: &CrawlFile, deadline: Instant, cancel: &CancelToken) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};
        let done = Arc::new(AtomicBool::new(false));
        let finished = done.clone();
        let interrupt = file.conn().get_interrupt_handle();
        let cancel = cancel.clone();
        let thread = std::thread::spawn(move || {
            while !finished.load(Ordering::Acquire) {
                if cancel.is_cancelled() || Instant::now() >= deadline {
                    interrupt.interrupt();
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(5));
            }
        });
        Self {
            done,
            thread: Some(thread),
        }
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        self.done.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

impl CompiledSearch {
    pub(super) fn table_name(&self) -> String {
        format!(
            "os_search_{}",
            crate::cursor::query_hash(&[&format!("{:?}", self.program)])
        )
    }
    pub(super) fn has_regex(&self) -> bool {
        self.program.atoms.iter().any(|a| a.regex.is_some())
    }

    pub fn matches_bitmap(
        &self,
        file: &CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
    ) -> Result<RoaringBitmap, SearchError> {
        let deadline = self.bounded_deadline(file, deadline, cancel)?;
        self.matches_where(file, deadline, cancel, "1", &[])
    }
    pub(super) fn bounded_deadline(
        &self,
        file: &CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
    ) -> Result<Instant, SearchError> {
        check(deadline, cancel)?;
        let start = Instant::now();
        let _watchdog = Watchdog::start(file, deadline, cancel);
        let count = file
            .conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get::<_, u64>(0));
        check(deadline, cancel)?;
        Ok(deadline.min(start + search_budget(count?, self.has_regex())))
    }
    pub(super) fn matches_where(
        &self,
        file: &CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
        predicate: &str,
        params: &[(String, Value)],
    ) -> Result<RoaringBitmap, SearchError> {
        check(deadline, cancel)?;
        let _watchdog = Watchdog::start(file, deadline, cancel);
        let result = self.scan(file, deadline, cancel, predicate, params);
        check(deadline, cancel)?;
        result
    }
    fn scan(
        &self,
        file: &CrawlFile,
        deadline: Instant,
        cancel: &CancelToken,
        predicate: &str,
        params: &[(String, Value)],
    ) -> Result<RoaringBitmap, SearchError> {
        let select = self
            .program
            .atoms
            .iter()
            .map(|a| format!(", ({})", a.sql))
            .collect::<String>();
        let mut stmt = file.conn().prepare(&format!(
            "SELECT u.id{select} FROM {URLS_FROM} WHERE {predicate}"
        ))?;
        super::view::bind(&mut stmt, params)?;
        let mut rows = stmt.raw_query();
        let mut result = RoaringBitmap::new();
        // Auch breite Regelbäume halten höchstens BATCH_SIZE Zellen pro Charge.
        let batch_size = (BATCH_SIZE / self.program.atoms.len().max(1)).max(1);
        let workers = std::thread::available_parallelism()
            .map_or(1, usize::from)
            .min(4);
        loop {
            check(deadline, cancel)?;
            let mut batch = Vec::with_capacity(batch_size);
            while batch.len() < batch_size {
                let Some(row) = rows.next()? else {
                    break;
                };
                let id = u32::try_from(row.get::<_, i64>(0)?)
                    .map_err(|_| invalid("url-id-outside-bitmap-range"))?;
                let mut values = Vec::with_capacity(self.program.atoms.len());
                for (i, atom) in self.program.atoms.iter().enumerate() {
                    let value = match row.get_ref(i + 1)? {
                        ValueRef::Null => serde_json::Value::Null,
                        ValueRef::Integer(n) => serde_json::Value::from(n),
                        ValueRef::Real(n) => serde_json::Value::from(n),
                        ValueRef::Text(t) if atom.list => {
                            serde_json::from_slice(t).map_err(|_| invalid("invalid-list-cell"))?
                        }
                        ValueRef::Text(t) | ValueRef::Blob(t) => {
                            serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
                        }
                    };
                    values.push(value);
                }
                batch.push((id, values));
            }
            if batch.is_empty() {
                break;
            }
            // Parallel nur auf eigenen Zellwerten: keine SQLite-Verbindung auf Worker-Threads.
            let chunks = batch.chunks(batch.len().div_ceil(workers));
            std::thread::scope(|scope| -> Result<(), SearchError> {
                let jobs: Vec<_> = chunks
                    .map(|chunk| {
                        scope.spawn(move || {
                            let mut ids = RoaringBitmap::new();
                            for (id, values) in chunk {
                                if self.program.matches(
                                    &self.program.node,
                                    values,
                                    deadline,
                                    cancel,
                                )? {
                                    ids.insert(*id);
                                }
                            }
                            Ok::<_, SearchError>(ids)
                        })
                    })
                    .collect();
                for job in jobs {
                    result |= job
                        .join()
                        .map_err(|_| invalid("search-worker-panicked"))??;
                }
                Ok(())
            })?;
        }
        Ok(result)
    }
}
