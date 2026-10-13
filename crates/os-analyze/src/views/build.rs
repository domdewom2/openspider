//! Aufbau des SQL-Plans aus `TabDef`/`FilterDef` (M1.md 3.9): Population ∧ Filter ∧ CRAWL-089-Ausschluss ∧
//! Suche, Parameter aus `TabDef.params(cfg)`, Sortierung nach der einen Regel für Tabs, Exporte und MCP.

use os_model::{CrawlConfig, Value};

use super::{ColumnSelection, SearchSpec, ViewPlan, ViewQuery};
use crate::cursor;
use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, SortKey, TabDef};
use crate::error::{NameError, ViewError};
use crate::names;

/// Ein Sortierschlüssel in SQL.
#[derive(Debug, Clone)]
pub(super) struct SortTerm {
    /// SQL-Ausdruck des Rohwerts.
    pub raw: String,
    /// Mit `COLLATE OS_TEXT` vergleichen (Textspalten); sonst bytewise bzw. numerisch.
    pub collate: bool,
    /// Absteigend.
    pub desc: bool,
}

impl SortTerm {
    /// `1`, wenn der Wert leer ist (`NULL` oder `''`) – leere Werte stehen immer am Ende.
    pub fn empty_expr(&self) -> String {
        format!(
            "(CASE WHEN ({0}) IS NULL OR ({0}) = '' THEN 1 ELSE 0 END)",
            self.raw
        )
    }

    /// Wertausdruck, für leere Werte `NULL` (alle leeren Werte sind untereinander gleich).
    pub fn value_expr(&self) -> String {
        let collate = if self.collate { " COLLATE OS_TEXT" } else { "" };
        format!(
            "(CASE WHEN {} = 1 THEN NULL ELSE ({}) END){collate}",
            self.empty_expr(),
            self.raw
        )
    }
}

/// Ein aufgelöster Plan samt der Teile, die Ausführung und Cursor brauchen.
pub(super) struct Built<'t> {
    pub filter: &'t FilterDef,
    pub plan: ViewPlan,
    pub columns: Vec<&'t ColumnDef>,
    pub sort: Vec<SortTerm>,
    pub query_hash: String,
}

/// Parametername mit führendem `:`.
pub(super) fn param_name(name: &str) -> String {
    if name.starts_with([':', '@', '$']) {
        name.to_string()
    } else {
        format!(":{name}")
    }
}

fn unknown(input: String, suggestions: Vec<String>) -> ViewError {
    ViewError::Name(NameError::Unknown { input, suggestions })
}

pub(super) fn build<'t>(
    tabs: &'t [TabDef],
    q: &ViewQuery,
    cfg: &CrawlConfig,
) -> Result<Built<'t>, ViewError> {
    let tab = names::find_tab(tabs, &q.tab).ok_or_else(|| {
        unknown(
            q.tab.clone(),
            names::suggestions(tabs, &format!("{}:{}", q.tab, q.filter)),
        )
    })?;
    let filter = names::find_filter(tab, &q.filter).ok_or_else(|| {
        unknown(
            format!("{}:{}", q.tab, q.filter),
            names::suggestions(tabs, &format!("{}:{}", tab.id, q.filter)),
        )
    })?;

    let population_sql = format!("({})", tab.population);
    let filter_sql = filter_sql(filter, cfg);

    let mut params: Vec<(String, Value)> = (tab.params)(cfg)
        .into_iter()
        .map(|(name, value)| (param_name(name), value))
        .collect();

    let search_text = match &q.search {
        Some(SearchSpec::Simple(s)) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    };
    let search_sql = search_text.map(|s| {
        params.push((":q".to_string(), Value::Text(s.to_string())));
        "instr(lower(u.address), lower(:q)) > 0".to_string()
    });

    let columns = select_columns(tab, &q.columns)?;
    let sort = sort_terms(tab, &q.sort)?;
    let order_by = order_by(&sort);

    let sort_sig = sort
        .iter()
        .map(|t| format!("{}|{}|{}", t.raw, t.collate, t.desc))
        .collect::<Vec<_>>()
        .join(";");
    let query_hash = cursor::query_hash(&[
        tab.id,
        filter.id,
        &sort_sig,
        search_text.unwrap_or_default(),
    ]);

    Ok(Built {
        filter,
        plan: ViewPlan {
            population_sql,
            filter_sql,
            search_sql,
            params,
            order_by,
        },
        columns,
        sort,
        query_hash,
    })
}

/// Filterprädikat inkl. CRAWL-089-Ausschluss (`issues.ignore_non_indexable`, nur bei Filtern mit gesetztem
/// `ignore_non_indexable` und nie für „All“).
pub(super) fn filter_sql(filter: &FilterDef, cfg: &CrawlConfig) -> String {
    if cfg.issues.ignore_non_indexable && filter.ignore_non_indexable && filter.id != "All" {
        format!(
            "(({}) AND COALESCE(u.indexability, '') <> 'Non-Indexable')",
            filter.predicate
        )
    } else {
        format!("({})", filter.predicate)
    }
}

/// `WHERE`-Bedingung: Population ∧ Filter [∧ Suche].
pub(super) fn where_sql(plan: &ViewPlan) -> String {
    let mut sql = format!("{} AND {}", plan.population_sql, plan.filter_sql);
    if let Some(search) = &plan.search_sql {
        sql.push_str(" AND ");
        sql.push_str(search);
    }
    sql
}

fn find_column<'t>(tab: &'t TabDef, name: &str) -> Option<&'t ColumnDef> {
    let wanted = name.trim();
    tab.columns.iter().find(|c| c.key == wanted).or_else(|| {
        tab.columns
            .iter()
            .find(|c| c.key.eq_ignore_ascii_case(wanted))
    })
}

fn select_columns<'t>(
    tab: &'t TabDef,
    selection: &ColumnSelection,
) -> Result<Vec<&'t ColumnDef>, ViewError> {
    match selection {
        ColumnSelection::Default => Ok(tab.columns.iter().filter(|c| c.default_visible).collect()),
        ColumnSelection::All => {
            // Referenzspalten zuerst, OS-Erweiterungen dahinter (ColumnDef.extension).
            let mut cols: Vec<&ColumnDef> = tab.columns.iter().filter(|c| !c.extension).collect();
            cols.extend(tab.columns.iter().filter(|c| c.extension));
            Ok(cols)
        }
        ColumnSelection::Named(wanted) => wanted
            .iter()
            .map(|name| {
                find_column(tab, name).ok_or_else(|| {
                    unknown(
                        format!("{}: Spalte {name}", tab.id),
                        names::column_suggestions(tab.columns.iter().map(|c| c.key), name),
                    )
                })
            })
            .collect(),
    }
}

/// SQL-Ausdruck für den n-ten Wert (ab 1) einer Mehrfachspalte; `?1` der Listenabfrage wird zu `u.id`.
pub(super) fn multi_nth_expr(list_sql: &str, n: usize) -> String {
    format!(
        "(SELECT * FROM ({}) LIMIT 1 OFFSET {})",
        list_sql.replace("?1", "u.id"),
        n.saturating_sub(1)
    )
}

/// Zerlegt `"Basis 3"` in (`"Basis"`, 3).
fn split_numbered(key: &str) -> Option<(&str, usize)> {
    let (base, num) = key.trim().rsplit_once(' ')?;
    let n: usize = num.parse().ok()?;
    (n >= 1).then_some((base, n))
}

fn sort_term(tab: &TabDef, key: &SortKey) -> Result<SortTerm, ViewError> {
    let (raw, kind, is_address) = if let Some(col) = find_column(tab, &key.column) {
        let is_address = col.key.eq_ignore_ascii_case("Address");
        match &col.sql {
            ColumnSql::Expr(e) => ((*e).to_string(), col.kind, is_address),
            ColumnSql::Multi { list_sql, .. } => (multi_nth_expr(list_sql, 1), col.kind, false),
        }
    } else if let Some((col, n)) = split_numbered(&key.column).and_then(|(base, n)| {
        tab.columns
            .iter()
            .find(|c| matches!(&c.sql, ColumnSql::Multi { base: b, .. } if b.eq_ignore_ascii_case(base)))
            .map(|c| (c, n))
    }) {
        match &col.sql {
            ColumnSql::Multi { list_sql, .. } => (multi_nth_expr(list_sql, n), col.kind, false),
            ColumnSql::Expr(_) => unreachable!("nur Mehrfachspalten gefunden"),
        }
    } else {
        return Err(unknown(
            format!("{}: Sortierspalte {}", tab.id, key.column),
            names::column_suggestions(tab.columns.iter().map(|c| c.key), &key.column),
        ));
    };
    Ok(SortTerm {
        raw,
        collate: kind == ColumnKind::Text && !is_address,
        desc: key.descending,
    })
}

/// Wirksame Sortierschlüssel: die der Abfrage, sonst `default_sort` des Tabs.
fn sort_terms(tab: &TabDef, requested: &[SortKey]) -> Result<Vec<SortTerm>, ViewError> {
    let keys = if requested.is_empty() {
        &tab.default_sort
    } else {
        requested
    };
    keys.iter().map(|k| sort_term(tab, k)).collect()
}

/// `ORDER BY` nach der Sortierregel (3.9): Schlüssel → `u.address` bytewise → `u.id`; ohne Schlüssel nur
/// `u.id`. Leere Werte stehen bei auf- und absteigender Sortierung am Ende.
fn order_by(sort: &[SortTerm]) -> String {
    let mut terms: Vec<String> = Vec::new();
    for t in sort {
        terms.push(format!("{} ASC", t.empty_expr()));
        terms.push(format!(
            "{} {}",
            t.value_expr(),
            if t.desc { "DESC" } else { "ASC" }
        ));
    }
    if !sort.is_empty() {
        terms.push("u.address ASC".to_string());
    }
    terms.push("u.id ASC".to_string());
    format!("ORDER BY {}", terms.join(", "))
}
