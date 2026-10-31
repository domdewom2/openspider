//! Standard-Linkformat der Bulk-Exporte (All Inlinks/Outlinks, Response-Codes-Inlinks, Detail-Exporte;
//! OUT-031/032/050, ONPAGE-192).
//!
//! Das Format hat 15 feste Spalten ([`LINK_COLUMNS`]). Die Zeilen kommen direkt aus den Tabellen `links`
//! (Quelle, Linktyp, Attribute) und `urls` (Quelle `s`, Ziel `t`); sie werden in einer einzigen Abfrage
//! gestreamt, nie gesammelt (konstanter Speicher). Die os-store-Abfragen `queries::inlinks`/`outlinks`
//! liefern nur einen Teil der Spalten (kein `Size (Bytes)`, `Target`, `Rel`, `Path Type`, `Link Path`,
//! `Link Position`, `Link Origin`) und blättern seitenweise; für die Exporte gilt dieselbe Auflösung
//! (`Destination` = Adresse der Ziel-URL, Rohwert nur ohne auflösbares Ziel, K-37; `Not Crawled` für Ziele
//! ohne Abruf, OUT-031) und dieselbe Sortierregel.
//!
//! * `Follow`: Text `True`/`False` (nicht der Wahrheitswert der CSV/XLSX-Schreiber, der `true` schriebe).
//! * `Status Code`: leer, wenn das Ziel nicht gecrawlt wurde oder keine Antwort kam (Code 0); `Status` ist
//!   dann `Not Crawled` bzw. der Grund (`DNS Lookup Failed` …).
//! * Keine Deduplizierung: zwei gleiche Links auf einer Seite sind zwei Zeilen.
//! * Sortierung: Inlinks nach `Destination`, `Source`, Dokumentreihenfolge (OUT-031); Outlinks nach `Source`,
//!   Dokumentreihenfolge (OUT-032).

use std::path::Path;

use os_analyze::defs::TabDef;
use os_analyze::views::{self, ViewQuery};
use os_analyze::ColumnKind;
use os_model::{CrawlConfig, FilterState, Value};
use os_store::{CrawlFile, StoreError};

use crate::catalog::bulk_links::{BulkLinkEntry, Selection};
use crate::error::ExportError;
use crate::output::open_writer;
use crate::pipeline::filter_warning;
use crate::types::{ExportFileInfo, Format, TableWriter};

/// Kopfzeile des Standard-Linkformats (OUT-031, Golden-Vergleich).
pub const LINK_COLUMNS: [&str; 15] = [
    "Type",
    "Source",
    "Destination",
    "Size (Bytes)",
    "Alt Text",
    "Anchor",
    "Status Code",
    "Status",
    "Follow",
    "Target",
    "Rel",
    "Path Type",
    "Link Path",
    "Link Position",
    "Link Origin",
];

/// Spaltenarten des Standard-Linkformats (für die Zellarten der Schreiber).
pub fn link_kinds() -> Vec<ColumnKind> {
    vec![
        ColumnKind::Text, // Type
        ColumnKind::Url,  // Source
        ColumnKind::Url,  // Destination
        ColumnKind::Int,  // Size (Bytes)
        ColumnKind::Text, // Alt Text
        ColumnKind::Text, // Anchor
        ColumnKind::Int,  // Status Code
        ColumnKind::Text, // Status
        ColumnKind::Text, // Follow ("True"/"False")
        ColumnKind::Text, // Target
        ColumnKind::Text, // Rel
        ColumnKind::Text, // Path Type
        ColumnKind::Text, // Link Path
        ColumnKind::Text, // Link Position
        ColumnKind::Text, // Link Origin
    ]
}

/// Kopfzeile als Zeichenketten.
pub fn link_header() -> Vec<String> {
    LINK_COLUMNS.iter().map(|c| (*c).to_string()).collect()
}

/// Reihenfolge der Zeilen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOrder {
    /// `Destination`, `Source`, Dokumentreihenfolge (OUT-031).
    Inlinks,
    /// Fehler nach Status, Ziel und Quelle (OUT-050).
    ErrorInlinks,
    /// `Source`, Dokumentreihenfolge (OUT-032).
    Outlinks,
}

impl LinkOrder {
    fn sql(self) -> &'static str {
        match self {
            LinkOrder::Inlinks => {
                "COALESCE(t.address, l.target_raw) ASC, s.address ASC, l.position ASC, l.id ASC"
            }
            LinkOrder::ErrorInlinks => {
                "CASE WHEN t.state = 'done' AND t.status_code > 0 THEN t.status_code END ASC, COALESCE(t.address, l.target_raw) ASC, s.address ASC, l.position ASC, l.id ASC"
            }
            LinkOrder::Outlinks => "s.address ASC, l.position ASC, l.id ASC",
        }
    }
}

/// Statustext des Ziels: ohne Abruf `Not Crawled`, ohne Ziel leer.
const STATUS_SQL: &str = "CASE WHEN t.id IS NULL THEN NULL \
     WHEN t.state <> 'done' OR t.status_code IS NULL THEN 'Not Crawled' ELSE t.status END";

/// Statuscode des Ziels; 0 (keine Antwort) und nicht gecrawlt sind leer.
const STATUS_CODE_SQL: &str =
    "CASE WHEN t.state = 'done' AND t.status_code > 0 THEN t.status_code END";

fn select_sql(where_sql: &str, order: LinkOrder) -> String {
    format!(
        "SELECT l.link_type, s.address, COALESCE(t.address, l.target_raw), t.size_bytes, l.alt_text, \
                l.anchor, {STATUS_CODE_SQL}, {STATUS_SQL}, \
                CASE WHEN l.follow <> 0 THEN 'True' ELSE 'False' END, \
                l.target_attr, l.rel, l.path_type, l.link_path, l.link_position, l.link_origin, l.uncrawlable_reason \
         FROM links l \
         JOIN urls s ON s.id = l.source_id \
         LEFT JOIN urls t ON t.id = l.target_id \
         WHERE {where_sql} ORDER BY {}",
        order.sql()
    )
}

/// SQL-Fehler in einen [`ExportError`] überführen (os-export bindet `rusqlite` nicht selbst ein).
pub(crate) fn sql<T, E>(r: Result<T, E>) -> Result<T, ExportError>
where
    StoreError: From<E>,
{
    r.map_err(|e| ExportError::Store(StoreError::from(e)))
}

/// Bindet die benannten Parameter, die in der Anweisung vorkommen (andere werden übergangen).
macro_rules! bind_named {
    ($stmt:expr, $params:expr) => {{
        for (name, value) in $params.iter() {
            let idx = $crate::links_export::sql($stmt.parameter_index(name))?;
            if let Some(idx) = idx {
                let bound = match value {
                    Value::Null => $stmt.raw_bind_parameter(idx, Option::<i64>::None),
                    Value::Bool(b) => $stmt.raw_bind_parameter(idx, i64::from(*b)),
                    Value::Int(i) => $stmt.raw_bind_parameter(idx, *i),
                    Value::Float(f) => $stmt.raw_bind_parameter(idx, *f),
                    Value::Text(s) => $stmt.raw_bind_parameter(idx, s.as_str()),
                };
                $crate::links_export::sql(bound)?;
            }
        }
    }};
}

/// Schreibt alle Zeilen der Abfrage in `writer` (ohne Kopfzeile, ohne `finish`); liefert die Zeilenzahl.
pub(crate) fn stream_links(
    file: &CrawlFile,
    where_sql: &str,
    params: &[(String, Value)],
    order: LinkOrder,
    kinds: &[ColumnKind],
    writer: &mut dyn TableWriter,
) -> Result<u64, ExportError> {
    stream_links_with(
        file,
        where_sql,
        params,
        order,
        kinds,
        writer,
        None,
        Layout::Standard,
    )
}

#[allow(clippy::too_many_arguments)]
fn stream_links_with(
    file: &CrawlFile,
    where_sql: &str,
    params: &[(String, Value)],
    order: LinkOrder,
    kinds: &[ColumnKind],
    writer: &mut dyn TableWriter,
    mut ctx: Option<&mut crate::types::ExportCtx<'_>>,
    layout: Layout,
) -> Result<u64, ExportError> {
    let conn = file.conn();
    let mut stmt = sql(conn.prepare_cached(&select_sql(where_sql, order)))?;
    bind_named!(stmt, params);
    let mut rows = stmt.raw_query();
    let mut count = 0u64;
    let mut values: Vec<Value> = Vec::with_capacity(LINK_COLUMNS.len());
    while let Some(row) = sql(rows.next())? {
        if let Some(ctx) = ctx.as_deref_mut() {
            ctx.checkpoint()?;
            (ctx.progress)(count);
        }
        values.clear();
        for &i in layout.indices() {
            let v = match i {
                3 | 6 => sql(row.get::<_, Option<i64>>(i))?.map_or(Value::Null, Value::Int),
                _ => sql(row.get::<_, Option<String>>(i))?.map_or(Value::Null, Value::Text),
            };
            let v = match (layout, i, v) {
                (Layout::Anchors, 5, Value::Text(s)) => {
                    Value::Text(os_store::queries::anchors::normalize(&s))
                }
                (Layout::Uncrawlable, 15, Value::Text(s)) => {
                    Value::Text(reason_text(&s).to_owned())
                }
                (_, _, value) => value,
            };
            values.push(v);
        }
        writer.row(kinds, &values)?;
        count += 1;
    }
    Ok(count)
}

/// Übernimmt einen benannten Parameter, falls noch nicht vorhanden (gleicher Name = gleicher Wert,
/// weil alle Filter denselben Tab und dieselbe Konfiguration verwenden).
fn merge_params(into: &mut Vec<(String, Value)>, from: Vec<(String, Value)>) {
    for (name, value) in from {
        if !into.iter().any(|(n, _)| *n == name) {
            into.push((name, value));
        }
    }
}

/// Bedingung „Ziel liegt in der Auswahl“ samt Parametern; `None` = die Auswahl ist leer (kein Filter in
/// M1, K-14). Der zweite Teil nennt, wenn ein Filter nicht auswertbar ist (Zustand ≠ `Ready`), den Grund.
struct TargetCondition {
    sql: String,
    params: Vec<(String, Value)>,
}

fn target_condition(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selection: &Selection,
    warnings: &mut Vec<String>,
) -> Result<Option<TargetCondition>, ExportError> {
    let (tab, filters, extra): (&str, &[String], Option<&str>) = match selection {
        Selection::All => {
            return Ok(Some(TargetCondition {
                sql: "1".to_string(),
                params: Vec::new(),
            }));
        }
        Selection::Empty => return Ok(None),
        Selection::LinkSpec(spec) => return link_spec_condition(tabs, file, cfg, spec, warnings),
        Selection::Filters { tab, filters } => (tab, filters, None),
        Selection::FilterWhere { tab, filter, extra } => {
            (tab, std::slice::from_ref(filter), Some(extra.as_str()))
        }
    };
    let mut alternatives: Vec<String> = Vec::new();
    let mut params: Vec<(String, Value)> = Vec::new();
    for filter in filters {
        let state = views::state_of_in(tabs, file, cfg, tab, filter)?;
        if state != FilterState::Ready {
            warnings.push(filter_warning(tab, filter, &state));
            continue;
        }
        let plan = views::plan_in(tabs, &ViewQuery::new(tab.to_string(), filter.clone()), cfg)?;
        let mut cond = format!("{} AND {}", plan.population_sql, plan.filter_sql);
        if let Some(extra) = extra {
            cond = format!("{cond} AND ({extra})");
        }
        alternatives.push(format!("({cond})"));
        merge_params(&mut params, plan.params);
    }
    if alternatives.is_empty() {
        return Ok(None);
    }
    Ok(Some(TargetCondition {
        sql: format!(
            "l.target_id IN (SELECT u.id FROM {} WHERE {})",
            views::URLS_FROM,
            alternatives.join(" OR ")
        ),
        params,
    }))
}

/// Führt einen Link-Bulk-Export des Katalogs aus (`Links:All Inlinks`, `Response Codes:…`).
///
/// Ohne auswertbare Auswahl (kein M1-Filter, Filter nicht bereit) entsteht eine Datei nur mit Kopfzeile;
/// Hinweise stehen in [`ExportFileInfo::warnings`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_link_entry_with_options(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    entry: &BulkLinkEntry,
    path: &Path,
    format: Format,
    sheet: &str,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let layout = Layout::for_entry(entry);
    let kinds = layout.kinds();
    let mut warnings: Vec<String> = Vec::new();
    let condition = target_condition(tabs, file, cfg, &entry.selection, &mut warnings)?;
    if entry.selection == Selection::Empty {
        warnings.push(format!(
            "{}: in dieser Version nicht befüllbar (setzt JavaScript-Rendering voraus); die Datei enthält nur die Kopfzeile",
            entry.name
        ));
    }
    let mut writer = open_writer(path, format, sheet, opts)?;
    writer.header(&Layout::for_entry(entry).header())?;
    if let Some(c) = &condition {
        stream_links_with(
            file,
            &c.sql,
            &c.params,
            entry.order,
            &kinds,
            &mut *writer,
            None,
            layout,
        )?;
    }
    let mut info = writer.finish()?;
    for w in &warnings {
        tracing::warn!("{w}");
    }
    info.warnings.extend(warnings);
    Ok(info)
}

/// Schreibt einen Linkexport in einen vorhandenen Writer (OUT-031/050).
pub fn export_link_entry_to(
    entry: &BulkLinkEntry,
    job: &crate::catalog::TableJob<'_>,
    writer: &mut dyn crate::types::TableWriter,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<u64, ExportError> {
    ctx.checkpoint()?;
    let mut warnings = Vec::new();
    let condition = target_condition(job.tabs, job.file, job.cfg, &entry.selection, &mut warnings)?;
    writer.header(&Layout::for_entry(entry).header())?;
    let count = match condition {
        Some(c) => stream_links_with(
            job.file,
            &c.sql,
            &c.params,
            entry.order,
            &Layout::for_entry(entry).kinds(),
            writer,
            Some(ctx),
            Layout::for_entry(entry),
        )?,
        None => 0,
    };
    ctx.checkpoint()?;
    (ctx.progress)(count);
    if count == 0 && ctx.skip_empty {
        ctx.skipped.push(entry.name.clone());
    }
    Ok(count)
}

#[derive(Clone, Copy)]
enum Layout {
    Standard,
    Anchors,
    Uncrawlable,
}
impl Layout {
    fn for_entry(entry: &BulkLinkEntry) -> Self {
        match entry.selection {
            Selection::LinkSpec("All Anchor Text") => Self::Anchors,
            Selection::LinkSpec("Pages With Uncrawlable Internal Outlinks") => Self::Uncrawlable,
            _ => Self::Standard,
        }
    }
    fn indices(self) -> &'static [usize] {
        match self {
            Self::Standard => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
            Self::Anchors => &[1, 2, 5, 4, 13, 8, 6],
            Self::Uncrawlable => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        }
    }
    fn header(self) -> Vec<String> {
        self.indices()
            .iter()
            .map(|&i| {
                if i == 15 {
                    "Link Crawlability"
                } else {
                    LINK_COLUMNS[i]
                }
                .to_owned()
            })
            .collect()
    }
    fn kinds(self) -> Vec<ColumnKind> {
        let kinds = link_kinds();
        self.indices()
            .iter()
            .map(|&i| if i == 15 { ColumnKind::Text } else { kinds[i] })
            .collect()
    }
}
fn reason_text(reason: &str) -> &str {
    match reason.parse::<os_model::UncrawlableReason>() {
        Ok(known) => known.label(),
        Err(_) => reason,
    }
}
fn link_spec_condition(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    spec: &str,
    warnings: &mut Vec<String>,
) -> Result<Option<TargetCondition>, ExportError> {
    use os_analyze::tabs::links::{LOCALHOST_LINK, NON_DESCRIPTIVE_LINK};
    let mut params = Vec::new();
    let predicate = match spec {
        "All Anchor Text" => "l.link_type = 'Hyperlink'".to_owned(),
        "External Links" => "s.is_internal = 1 AND t.is_internal = 0".to_owned(),
        "Internal Nofollow Outlinks" => "s.is_internal = 1 AND t.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND l.follow = 0".to_owned(),
        "Internal Outlinks With No Anchor Text" => "s.is_internal = 1 AND t.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND COALESCE(l.anchor, '') COLLATE OS_ANCHOR = '' AND COALESCE(l.alt_text, '') COLLATE OS_ANCHOR = ''".to_owned(),
        "Non-Descriptive Anchor Text In Internal Outlinks" => {
            params.push((":anchor_phrases".into(), Value::Text(serde_json::to_string(&cfg.links.non_descriptive_anchors).expect("string list"))));
            format!("s.is_internal = 1 AND ({NON_DESCRIPTIVE_LINK})")
        }
        "Outlinks To Localhost" => format!("s.is_internal = 1 AND ({LOCALHOST_LINK})"),
        "Pages With Uncrawlable Internal Outlinks" => {
            if !cfg.spider.store_uncrawlable_links {
                warnings.push("Uncrawlable links require spider.store_uncrawlable_links".into());
                return Ok(None);
            }
            "s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.crawlable = 0".to_owned()
        }
        "Follow & Nofollow Internal Inlinks To Page" | "Internal Nofollow Inlinks Only" | "Non-Indexable Page Inlinks Only" => {
            let selection = Selection::Filters { tab: "Links".into(), filters: vec![spec.into()] };
            let Some(mut condition) = target_condition(tabs, file, cfg, &selection, warnings)? else { return Ok(None) };
            condition.sql = format!("({}) AND s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.crawlable = 1 AND l.source_id <> l.target_id", condition.sql);
            return Ok(Some(condition));
        }
        _ => unreachable!("unknown Links specification"),
    };
    Ok(Some(TargetCondition {
        sql: predicate,
        params,
    }))
}
