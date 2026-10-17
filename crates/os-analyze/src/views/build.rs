//! Aufbau des SQL-Plans aus `TabDef`/`FilterDef` (M1.md 3.9): Population ∧ Filter ∧ CRAWL-089-Ausschluss ∧
//! Suche, Parameter aus `TabDef.params(cfg)`, Sortierung nach der einen Regel für Tabs, Exporte und MCP.

use os_model::{CrawlConfig, Value};

use super::{ColumnSelection, SearchSpec, ViewPlan, ViewQuery};
use crate::cursor;
use crate::defs::{ColumnDef, ColumnKind, ColumnSql, FilterDef, PairedPart, SortKey, TabDef};
use crate::error::{NameError, ViewError};
use crate::{names, tabs};

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

/// Welcher Teil einer Spalte ausgewählt ist.
#[derive(Debug, Clone)]
pub(super) enum Pick {
    /// Die ganze Spalte (Mehrfachspalten: alle Indizes).
    Whole,
    /// Genau eine nummerierte Teilspalte einer Mehrfachspalte, z. B. `Meta Robots 1` oder
    /// `HTML hreflang 1 URL`.
    Numbered {
        /// Ausgabename in kanonischer Schreibweise.
        name: String,
        /// Index (ab 1).
        n: usize,
        /// Teilspalte bei [`ColumnSql::MultiPaired`] (sonst 0).
        part: usize,
    },
}

/// Eine ausgewählte Spalte der Ansicht.
pub(super) struct SelectedColumn<'t> {
    pub def: &'t ColumnDef,
    pub pick: Pick,
}

/// Ein aufgelöster Plan samt der Teile, die Ausführung und Cursor brauchen.
pub(super) struct Built<'t> {
    pub tab: &'t TabDef,
    pub filter: &'t FilterDef,
    pub plan: ViewPlan,
    pub columns: Vec<SelectedColumn<'t>>,
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

    let population_sql = match tab.hidden_population {
        Some(hidden) => format!("(({}) OR ({}))", tab.population, hidden),
        None => format!("({})", tab.population),
    };
    let mut filter_sql = filter_sql(filter, cfg);
    for requirement in &filter.requires {
        if let crate::Requirement::Coverage { marker, .. } = requirement {
            filter_sql = format!("({filter_sql}) AND EXISTS (SELECT 1 FROM url_ext checked WHERE checked.url_id = u.id AND checked.key = '{}' AND checked.v_int = 1)", marker.replace(char::from(39), "''"));
        }
    }

    let mut params: Vec<(String, Value)> = (tab.params)(cfg)
        .into_iter()
        .map(|(name, value)| (param_name(name), value))
        .collect();

    let search_text = match &q.search {
        Some(SearchSpec::Simple(s)) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    };
    let search_sql = if let Some(spec) = &q.search {
        let visible: Vec<&str> = tab
            .columns
            .iter()
            .filter(|c| c.default_visible)
            .map(|c| c.key)
            .collect();
        let compiled = crate::search::compile_ctx(
            spec,
            &crate::search::SearchCtx {
                tab,
                columns: &tab.columns,
                visible: &visible,
            },
        )?;
        params.extend(compiled.params);
        Some(compiled.sql)
    } else {
        None
    };

    let defaults = super::ColumnSelection::Named(
        filter
            .default_columns
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    );
    let columns = select_columns(
        tab,
        if matches!(q.columns, super::ColumnSelection::Default)
            && !filter.default_columns.is_empty()
        {
            &defaults
        } else {
            &q.columns
        },
    )?;
    let sort = sort_terms(tab, filter, &q.sort)?;
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
        tab,
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

/// Ergebnis der Namensauflösung einer Spalte.
enum Resolved<'t> {
    /// Eine Spalte über ihren Schlüssel.
    Whole(&'t ColumnDef),
    /// Eine nummerierte Teilspalte einer Mehrfachspalte.
    Numbered {
        def: &'t ColumnDef,
        n: usize,
        part: usize,
    },
}

fn find_column<'t>(tab: &'t TabDef, name: &str) -> Option<&'t ColumnDef> {
    let wanted = name.trim();
    tab.columns.iter().find(|c| c.key == wanted).or_else(|| {
        tab.columns
            .iter()
            .find(|c| c.key.eq_ignore_ascii_case(wanted))
    })
}

/// Liest `"<base> <n>"` bzw. `"<base> <n> <suffix>"` (Groß-/Kleinschreibung gleichgültig); `suffix` leer =
/// ohne Zusatz. `n` ist eine Zahl ab 1.
fn parse_numbered(name: &str, base: &str, suffix: &str) -> Option<usize> {
    let head = name.get(..base.len())?;
    if !head.eq_ignore_ascii_case(base) {
        return None;
    }
    let rest = name[base.len()..].strip_prefix(' ')?;
    let (num, tail) = match rest.split_once(' ') {
        Some((num, tail)) => (num, Some(tail)),
        None => (rest, None),
    };
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: usize = num.parse().ok().filter(|n| *n >= 1)?;
    match (tail, suffix.is_empty()) {
        (None, true) => Some(n),
        (Some(t), false) if t.eq_ignore_ascii_case(suffix) => Some(n),
        _ => None,
    }
}

/// Löst einen Spaltennamen auf: erst der Schlüssel einer Spalte, dann der nummerierte Name einer
/// Mehrfachspalte (`Meta Robots 1`, `Canonical Link Element 2`, `HTML hreflang 1 URL`).
fn resolve_column<'t>(tab: &'t TabDef, name: &str) -> Option<Resolved<'t>> {
    if let Some(def) = find_column(tab, name) {
        return Some(Resolved::Whole(def));
    }
    let wanted = name.trim();
    for def in &tab.columns {
        match &def.sql {
            ColumnSql::Expr(_) => {}
            ColumnSql::Multi { base, .. } | ColumnSql::MultiCrawlWide { base, .. } => {
                if let Some(n) = parse_numbered(wanted, base, "") {
                    return Some(Resolved::Numbered { def, n, part: 0 });
                }
            }
            ColumnSql::MultiPaired { base, parts } => {
                for (part, p) in parts.iter().enumerate() {
                    if let Some(n) = parse_numbered(wanted, base, p.suffix) {
                        return Some(Resolved::Numbered { def, n, part });
                    }
                }
            }
        }
    }
    None
}

/// Öffentliche Namensregel für Spalten eines Tabs: `true`, wenn `name` eine Spalte des Tabs benennt, also
/// ihren Schlüssel (Groß-/Kleinschreibung gleichgültig) oder eine nummerierte Teilspalte einer
/// Mehrfachspalte (`Meta Robots 2`, `HTML hreflang 1`, `HTML hreflang 1 URL`). Dieselbe Auflösung wie beim
/// Aufbau der Ansicht (`resolve_column`): Basis und Suffix ohne Beachtung der Schreibweise, `n` nur Ziffern
/// und mindestens 1. Aufrufer wie das MCP-Werkzeug `query_urls` prüfen Namen damit vorab, ohne die Regel
/// nachzubauen.
pub fn column_name_resolves(tab: &TabDef, name: &str) -> bool {
    resolve_column(tab, name).is_some()
}

/// Ausgabename der nummerierten Teilspalte in kanonischer Schreibweise.
pub(super) fn numbered_name(base: &str, n: usize, suffix: &str) -> String {
    if suffix.is_empty() {
        format!("{base} {n}")
    } else {
        format!("{base} {n} {suffix}")
    }
}

/// Grundname und Teilspalten einer Mehrfachspalte; `None` bei einfachen Spalten. Eine einfache
/// Mehrfachspalte hat eine Teilspalte ohne Suffix.
pub(super) fn series_parts(def: &ColumnDef) -> Option<(&'static str, Vec<PairedPart>)> {
    match &def.sql {
        ColumnSql::Expr(_) => None,
        ColumnSql::Multi { base, list_sql } | ColumnSql::MultiCrawlWide { base, list_sql } => {
            Some((
                *base,
                vec![PairedPart {
                    suffix: "",
                    list_sql,
                    kind: def.kind,
                }],
            ))
        }
        ColumnSql::MultiPaired { base, parts } => Some((*base, parts.to_vec())),
    }
}

fn select_columns<'t>(
    tab: &'t TabDef,
    selection: &ColumnSelection,
) -> Result<Vec<SelectedColumn<'t>>, ViewError> {
    let whole = |def: &'t ColumnDef| SelectedColumn {
        def,
        pick: Pick::Whole,
    };
    match selection {
        ColumnSelection::Default => Ok(tab
            .columns
            .iter()
            .filter(|c| c.default_visible)
            .map(whole)
            .collect()),
        ColumnSelection::All => {
            // Referenzspalten zuerst, OS-Erweiterungen dahinter (ColumnDef.extension).
            let mut cols: Vec<SelectedColumn<'t>> = tab
                .columns
                .iter()
                .filter(|c| !c.extension)
                .map(whole)
                .collect();
            cols.extend(tab.columns.iter().filter(|c| c.extension).map(whole));
            Ok(cols)
        }
        ColumnSelection::Named(wanted) => wanted
            .iter()
            .map(|name| match resolve_column(tab, name) {
                Some(Resolved::Whole(def)) => Ok(whole(def)),
                Some(Resolved::Numbered { def, n, part }) => {
                    let (base, parts) = series_parts(def)
                        .ok_or_else(|| unknown(format!("{}: Spalte {name}", tab.id), Vec::new()))?;
                    let suffix = parts.get(part).map_or("", |p| p.suffix);
                    Ok(SelectedColumn {
                        def,
                        pick: Pick::Numbered {
                            name: numbered_name(base, n, suffix),
                            n,
                            part,
                        },
                    })
                }
                None => Err(unknown(
                    format!("{}: Spalte {name}", tab.id),
                    names::column_suggestions(tab.columns.iter().map(|c| c.key), name),
                )),
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

fn sort_term(tab: &TabDef, key: &SortKey) -> Result<SortTerm, ViewError> {
    let not_found = || {
        unknown(
            format!("{}: Sortierspalte {}", tab.id, key.column),
            names::column_suggestions(tab.columns.iter().map(|c| c.key), &key.column),
        )
    };
    let (raw, kind, is_address) = match resolve_column(tab, &key.column).ok_or_else(not_found)? {
        Resolved::Whole(def) => match &def.sql {
            ColumnSql::Expr(e) => (
                (*e).to_string(),
                def.kind,
                def.key.eq_ignore_ascii_case("Address"),
            ),
            // Ganze Mehrfachspalte: nach dem ersten Wert der ersten Teilspalte.
            _ => {
                let (_, parts) = series_parts(def).ok_or_else(not_found)?;
                let first = parts.first().ok_or_else(not_found)?;
                (multi_nth_expr(first.list_sql, 1), first.kind, false)
            }
        },
        Resolved::Numbered { def, n, part } => {
            let (_, parts) = series_parts(def).ok_or_else(not_found)?;
            let p = parts.get(part).ok_or_else(not_found)?;
            (multi_nth_expr(p.list_sql, n), p.kind, false)
        }
    };
    Ok(SortTerm {
        raw,
        collate: kind == ColumnKind::Text && !is_address,
        desc: key.descending,
    })
}

/// Wirksame Sortierschlüssel: die der Abfrage, sonst die Standardsortierung des Filters
/// ([`tabs::filter_default_sort`]), sonst `default_sort` des Tabs, sonst Entdeckungsreihenfolge.
fn sort_terms(
    tab: &TabDef,
    filter: &FilterDef,
    requested: &[SortKey],
) -> Result<Vec<SortTerm>, ViewError> {
    let registry;
    let keys: &[SortKey] = if !requested.is_empty() {
        requested
    } else if let Some(keys) = tabs::filter_default_sort(tab.id, filter.id).filter(|keys| {
        keys.iter()
            .all(|k| resolve_column(tab, &k.column).is_some())
    }) {
        registry = keys;
        &registry
    } else {
        &tab.default_sort
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs;

    fn tab(id: &str) -> TabDef {
        tabs::all()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("Tab {id} fehlt"))
    }

    // spec: AGENT-011
    #[test]
    fn numbered_names_resolve_for_multi_and_paired_columns() {
        let hreflang = tab("Hreflang");
        for name in [
            "HTML hreflang 1",
            "html HREFLANG 12",
            "HTML hreflang 1 URL",
            "html hreflang 3 url",
            "  HTML hreflang 2 URL  ",
        ] {
            assert!(column_name_resolves(&hreflang, name), "{name}");
        }
        let internal = tab("Internal");
        for name in [
            "Title 1",
            "title 2",
            "H1-2",
            "h1-2",
            "Meta Robots 3",
            "meta ROBOTS 1",
        ] {
            assert!(column_name_resolves(&internal, name), "{name}");
        }
        assert!(column_name_resolves(&internal, "Address"));
        assert!(column_name_resolves(&internal, "address"));
    }

    // spec: AGENT-011
    #[test]
    fn invalid_numbered_names_do_not_resolve() {
        let hreflang = tab("Hreflang");
        for name in [
            "HTML hreflang 0",
            "HTML hreflang 0 URL",
            "HTML hreflang x",
            "HTML hreflang +1 URL",
            "HTML hreflang URL",
            "HTML hreflang 1 URLs",
            "HTML hreflang 1 Foo",
            "HTML hreflang 1  URL",
            "",
        ] {
            assert!(!column_name_resolves(&hreflang, name), "{name:?}");
        }
        let internal = tab("Internal");
        for name in ["Title 0", "Title x", "Meta Robots 0", "Meta Robots 1 URL"] {
            assert!(!column_name_resolves(&internal, name), "{name}");
        }
    }

    // spec: AGENT-011
    #[test]
    fn parse_numbered_reads_base_number_and_suffix() {
        assert_eq!(parse_numbered("Base 3", "base", ""), Some(3));
        assert_eq!(parse_numbered("BASE 3 url", "Base", "URL"), Some(3));
        assert_eq!(parse_numbered("Base 3 URL", "Base", ""), None);
        assert_eq!(parse_numbered("Base 3", "Base", "URL"), None);
        assert_eq!(parse_numbered("Base 0", "Base", ""), None);
        assert_eq!(parse_numbered("Baseline 1", "Base", ""), None);
    }
}
