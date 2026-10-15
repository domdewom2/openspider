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
use crate::pipeline::{filter_warning, writer_options};
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
    /// `Source`, Dokumentreihenfolge (OUT-032).
    Outlinks,
}

impl LinkOrder {
    fn sql(self) -> &'static str {
        match self {
            LinkOrder::Inlinks => {
                "COALESCE(t.address, l.target_raw) ASC, s.address ASC, l.position ASC, l.id ASC"
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
                l.target_attr, l.rel, l.path_type, l.link_path, l.link_position, l.link_origin \
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
    let conn = file.conn();
    let mut stmt = sql(conn.prepare_cached(&select_sql(where_sql, order)))?;
    bind_named!(stmt, params);
    let mut rows = stmt.raw_query();
    let mut count = 0u64;
    let mut values: Vec<Value> = Vec::with_capacity(LINK_COLUMNS.len());
    while let Some(row) = sql(rows.next())? {
        values.clear();
        for i in 0..LINK_COLUMNS.len() {
            let v = match i {
                3 | 6 => sql(row.get::<_, Option<i64>>(i))?.map_or(Value::Null, Value::Int),
                _ => sql(row.get::<_, Option<String>>(i))?.map_or(Value::Null, Value::Text),
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
            }))
        }
        Selection::Empty => return Ok(None),
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
pub(crate) fn export_link_entry(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    entry: &BulkLinkEntry,
    path: &Path,
    format: Format,
    sheet: &str,
) -> Result<ExportFileInfo, ExportError> {
    let kinds = link_kinds();
    let mut warnings: Vec<String> = Vec::new();
    let condition = target_condition(tabs, file, cfg, &entry.selection, &mut warnings)?;
    if entry.selection == Selection::Empty {
        warnings.push(format!(
            "{}: in dieser Version nicht befüllbar (setzt JavaScript-Rendering voraus); die Datei enthält nur die Kopfzeile",
            entry.name
        ));
    }
    let mut writer = open_writer(path, format, sheet, &writer_options(cfg))?;
    writer.header(&link_header())?;
    if let Some(c) = &condition {
        stream_links(file, &c.sql, &c.params, entry.order, &kinds, &mut *writer)?;
    }
    let mut info = writer.finish()?;
    for w in &warnings {
        tracing::warn!("{w}");
    }
    info.warnings.extend(warnings);
    Ok(info)
}
