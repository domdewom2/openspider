//! Exporte im Vergleichsmodus (OUT-113, OUT-140 ff.): Crawl Overview mit Vergleichsspalten (OUT-147), Tab-Exporte mit
//! `Comparison Status` und `Previous Address` (OUT-151), Inlinks/Outlinks-Vergleich (OUT-149) und Reports im
//! Vergleichs- und Segmentkontext (OUT-113).
//!
//! Alle Zahlen stammen aus derselben Klassifikation wie das Overview-Panel ([`os_analyze::compare::Comparison`]);
//! Bulk-Exporte und Reports außer dem Crawl Overview beziehen sich auf den Current-Crawl (OUT-113, OUT-151).
//!
//! **OS-Festlegungen:**
//! * Der Kopfblock des Crawl Overview (Name und Startzeit beider Crawls) steht als Abschnitt `Comparison` in denselben
//!   zehn Spalten vor den Abschnitten des Reports; Change-Detection-Elemente (M2-C02) übergibt der Aufrufer als eigenen
//!   Abschnitt ([`OverviewRow::section`]), sie folgen nach dem letzten Abschnitt.
//! * Ein Tab-Export ohne Unterfilter enthält die Zeilen des Filters im Current-Crawl mit `Added`, `New` oder
//!   `Unchanged`; `Removed` und `Missing` erscheinen nur mit dem jeweiligen Unterfilter. Removed-Zeilen zeigen die
//!   Current-Zeile des Partners, Missing-Zeilen die Previous-Zeile (Spalten nach Namen zugeordnet).
//! * `Previous Address` steht nur bei aktivem URL-Mapping im Export; bei `New` bleibt sie leer.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use os_analyze::compare::links::CHANGE_COLUMNS;
use os_analyze::compare::{
    ChangeFilter, CompareClass, CompareError, Comparison, LinkDirection, LinkSource, RoaringBitmap,
    Side,
};
use os_analyze::defs::{ColumnSql, TabDef};
use os_analyze::views::{self, ColumnSelection, PageStart, ViewQuery};
use os_analyze::ColumnKind;
use os_model::{CancelToken, CrawlConfig, UrlId, Value};
use os_store::{CrawlFile, StoreError};

use crate::catalog::reports::{self as report_catalog, ReportKind};
use crate::catalog::TableJob;
use crate::error::ExportError;
use crate::links_export::{link_header, link_kinds};
use crate::output::open_writer;
use crate::pipeline::writer_options;
use crate::reports::overview::{self, Cell};
use crate::types::{ExportCtx, ExportFileInfo, Format, ReportResult, SegmentScope, TableWriter};

/// Kopfzeile des Crawl Overview im Vergleichsmodus (OUT-147).
pub const OVERVIEW_HEADER: [&str; 10] = [
    "Section", "Item", "Current", "Previous", "Change", "Change %", "Added", "New", "Removed",
    "Missing",
];

/// Zusatzspalte der Tab-Exporte (OUT-151).
pub const STATUS_COLUMN: &str = "Comparison Status";
/// Zusatzspalte bei aktivem URL-Mapping (OUT-151, TECH-306).
pub const PREVIOUS_ADDRESS_COLUMN: &str = "Previous Address";
/// Abschnitt des Kopfblocks.
pub const HEADER_SECTION: &str = "Comparison";

/// Fehler des Vergleichs als Exportfehler.
pub fn compare_error(e: CompareError) -> ExportError {
    match e {
        CompareError::Store(e) => ExportError::Store(e),
        CompareError::Sql(e) => ExportError::Store(StoreError::from(e)),
        CompareError::View(e) => ExportError::View(e),
        CompareError::Cancelled => ExportError::Write("Export abgebrochen".into()),
        e => ExportError::Write(e.to_string()),
    }
}

fn cmp<T>(r: Result<T, CompareError>) -> Result<T, ExportError> {
    r.map_err(compare_error)
}

// ---------------------------------------------------------------------------------------------
// OUT-147 Crawl Overview

/// Eine Zeile des Crawl Overview im Vergleichsmodus.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRow {
    /// Abschnitt.
    pub section: String,
    /// Eintrag.
    pub item: String,
    /// Wert im Current-Crawl.
    pub current: Cell,
    /// Wert im Previous-Crawl.
    pub previous: Cell,
    /// Unterfilter `Added`, `New`, `Removed`, `Missing` (nur Filterzeilen mit Zahlen in beiden Crawls).
    pub classes: Option<[u64; 4]>,
}

fn number(c: &Cell) -> Option<f64> {
    match c {
        Cell::Int(n) => Some(*n as f64),
        Cell::Float(f) => Some(*f),
        _ => None,
    }
}

impl OverviewRow {
    /// `Current − Previous` für Zahlen.
    pub fn change(&self) -> Option<f64> {
        Some(number(&self.current)? - number(&self.previous)?)
    }

    /// `Change` in Prozent von `Previous`, zwei Nachkommastellen; leer bei `Previous` = 0.
    pub fn change_pct(&self) -> Option<f64> {
        let previous = number(&self.previous)?;
        let change = self.change()?;
        (previous != 0.0).then(|| ((change * 100.0 / previous) * 100.0).round() / 100.0)
    }

    /// Werte und Zellarten in der Reihenfolge von [`OVERVIEW_HEADER`].
    pub fn to_values(&self) -> (Vec<ColumnKind>, Vec<Value>) {
        let cell = |c: &Cell| match c {
            Cell::Empty => (ColumnKind::Int, Value::Null),
            Cell::Int(n) => (
                ColumnKind::Int,
                Value::Int(i64::try_from(*n).unwrap_or(i64::MAX)),
            ),
            Cell::Text(t) => (ColumnKind::Text, Value::Text(t.clone())),
            Cell::Float(f) => (ColumnKind::Float { decimals: 2 }, Value::Float(*f)),
        };
        let (ck, cv) = cell(&self.current);
        let (pk, pv) = cell(&self.previous);
        let (change_kind, change) = match (self.change(), &self.current, &self.previous) {
            (Some(d), Cell::Int(_), Cell::Int(_)) => (ColumnKind::Int, Value::Int(d as i64)),
            (Some(d), _, _) => (ColumnKind::Float { decimals: 2 }, Value::Float(d)),
            (None, _, _) => (ColumnKind::Int, Value::Null),
        };
        let mut kinds = vec![ColumnKind::Text, ColumnKind::Text, ck, pk, change_kind];
        kinds.push(ColumnKind::Float { decimals: 2 });
        kinds.extend([ColumnKind::Int; 4]);
        let mut values = vec![
            Value::Text(self.section.clone()),
            Value::Text(self.item.clone()),
            cv,
            pv,
            change,
            self.change_pct().map_or(Value::Null, Value::Float),
        ];
        for i in 0..4 {
            values.push(self.classes.map_or(Value::Null, |k| {
                Value::Int(i64::try_from(k[i]).unwrap_or(i64::MAX))
            }));
        }
        (kinds, values)
    }
}

/// Berechnet den Crawl Overview im Vergleichsmodus: Kopfblock, die Abschnitte des Reports (OUT-080) mit den Werten
/// beider Crawls und – für die Filterzeilen der Tabs – den Unterfiltern aus der Klassifikation. Prüft das Token je
/// Filter (Pause/Abbruch, CRAWL-129).
pub fn crawl_overview(
    comparison: &mut Comparison,
    cancel: &CancelToken,
) -> Result<Vec<OverviewRow>, ExportError> {
    let filters = cmp(comparison.overview(cancel))?;
    let cur = comparison.current();
    let prev = comparison.previous();
    let current = overview::build(&cur.file, &cur.config, &tabs_of(&cur.file, &cur.config))?;
    let previous = overview::build(&prev.file, &prev.config, &tabs_of(&prev.file, &prev.config))?;
    let text = |s: &str| Cell::Text(s.to_string());
    let stamp =
        |t: Option<os_model::Timestamp>| t.map_or(Cell::Empty, |t| Cell::Text(t.to_export()));
    let mut rows = vec![
        OverviewRow {
            section: HEADER_SECTION.into(),
            item: "Name".into(),
            current: text(&cur.meta.name),
            previous: text(&prev.meta.name),
            classes: None,
        },
        OverviewRow {
            section: HEADER_SECTION.into(),
            item: "Crawl Start".into(),
            current: stamp(cur.meta.started_at),
            previous: stamp(prev.meta.started_at),
            classes: None,
        },
    ];
    let by_label: HashMap<(&str, &str), &os_analyze::compare::FilterComparison> = filters
        .iter()
        .map(|f| ((f.tab.as_str(), f.label.as_str()), f))
        .collect();
    let previous_rows: HashMap<(&str, &str), &Cell> = previous
        .rows
        .iter()
        .map(|r| ((r.section.as_str(), r.item.as_str()), &r.urls))
        .collect();
    for r in &current.rows {
        cancel
            .checkpoint()
            .map_err(|_| ExportError::Write("Export abgebrochen".into()))?;
        let row = match by_label.get(&(r.section.as_str(), r.item.as_str())) {
            Some(f) => OverviewRow {
                section: r.section.clone(),
                item: r.item.clone(),
                current: f.current.map_or(Cell::Empty, Cell::Int),
                previous: f.previous.map_or(Cell::Empty, Cell::Int),
                classes: f.classes,
            },
            None => OverviewRow {
                section: r.section.clone(),
                item: r.item.clone(),
                current: r.urls.clone(),
                previous: previous_rows
                    .get(&(r.section.as_str(), r.item.as_str()))
                    .map_or(Cell::Empty, |c| (*c).clone()),
                classes: None,
            },
        };
        rows.push(row);
    }
    Ok(rows)
}

fn tabs_of(file: &CrawlFile, cfg: &CrawlConfig) -> Vec<TabDef> {
    os_analyze::tabs::configured_for_file(cfg, file)
}

/// Schreibt den Crawl Overview im Vergleichsmodus; `extra` sind weitere Abschnitte (Change-Detection-Elemente, M2-C02),
/// die hinter dem Report folgen.
pub fn write_crawl_overview(
    comparison: &mut Comparison,
    extra: &[OverviewRow],
    writer: &mut dyn TableWriter,
    ctx: &mut ExportCtx<'_>,
) -> Result<u64, ExportError> {
    ctx.checkpoint()?;
    let rows = crawl_overview(comparison, ctx.cancel)?;
    writer.header(&OVERVIEW_HEADER.map(String::from))?;
    let mut n = 0;
    for row in rows.iter().chain(extra) {
        ctx.checkpoint()?;
        let (kinds, values) = row.to_values();
        writer.row(&kinds, &values)?;
        n += 1;
        (ctx.progress)(n);
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------
// OUT-151 Tab-Exporte

/// Auswahl eines Tab-Exports im Vergleichsmodus.
#[derive(Debug, Clone, PartialEq)]
pub struct TabSelection {
    /// Tab.
    pub tab: String,
    /// Filter.
    pub filter: String,
    /// Unterfilter; `None` = alle Zeilen des Filters im Current-Crawl.
    pub class: Option<CompareClass>,
    /// Spalten.
    pub columns: ColumnSelection,
}

fn column_kind(tab: Option<&TabDef>, name: &str) -> ColumnKind {
    let Some(tab) = tab else {
        return ColumnKind::Text;
    };
    if let Some(c) = tab
        .columns
        .iter()
        .find(|c| c.key.eq_ignore_ascii_case(name))
    {
        return c.kind;
    }
    if let Some((base, num)) = name.trim().rsplit_once(' ') {
        if num.parse::<usize>().is_ok() {
            if let Some(c) = tab.columns.iter().find(
                |c| matches!(&c.sql, ColumnSql::Multi { base: b, .. } if b.eq_ignore_ascii_case(base)),
            ) {
                return c.kind;
            }
        }
    }
    ColumnKind::Text
}

/// Adressen zu Kennungen einer Datei.
fn addresses(
    file: &CrawlFile,
    ids: impl IntoIterator<Item = u32>,
) -> Result<HashMap<u32, String>, ExportError> {
    let mut stmt = crate::links_export::sql(
        file.conn()
            .prepare("SELECT address FROM urls WHERE id = ?1"),
    )?;
    let mut out = HashMap::new();
    for id in ids {
        let address: String =
            crate::links_export::sql(stmt.query_row([i64::from(id)], |r| r.get(0)))?;
        out.insert(id, address);
    }
    Ok(out)
}

/// Zeile zu einer Kennung: Spaltennamen der Seite, Kennung, Werte.
type IdRow = (Vec<String>, UrlId, Vec<Value>);

/// Zeilen zu ausdrücklichen Kennungen (Removed, Missing) in Adressreihenfolge, Spalten des Tabs; je Abschnitt von 500
/// Kennungen eine Abfrage.
fn rows_by_ids(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    columns: &ColumnSelection,
    ids: &RoaringBitmap,
    ctx: &ExportCtx<'_>,
) -> Result<Vec<IdRow>, ExportError> {
    let tabs = tabs_of(file, cfg);
    let names = addresses(file, ids.iter())?;
    let mut sorted: Vec<(String, u32)> = names.into_iter().map(|(id, a)| (a, id)).collect();
    sorted.sort();
    let mut out = Vec::with_capacity(sorted.len());
    for chunk in sorted.chunks(500) {
        ctx.checkpoint()?;
        let mut q = ViewQuery::new(tab, "All");
        q.columns = columns.clone();
        q.sort = vec![os_analyze::SortKey {
            column: "Address".into(),
            descending: false,
        }];
        q.limit = chunk.len().max(1) as u32;
        q.start = PageStart::Offset(0);
        let chunk_ids: Vec<UrlId> = chunk.iter().map(|(_, id)| UrlId(i64::from(*id))).collect();
        let page = views::query_url_ids_in(&tabs, file, cfg, &q, &chunk_ids, true)?;
        for (key, row) in page.row_keys.iter().zip(page.rows) {
            out.push((page.columns.clone(), *key, row));
        }
    }
    Ok(out)
}

/// Schreibt einen Tab-Export im Vergleichsmodus (OUT-151).
pub fn write_tab(
    comparison: &mut Comparison,
    selection: &TabSelection,
    writer: &mut dyn TableWriter,
    ctx: &mut ExportCtx<'_>,
) -> Result<u64, ExportError> {
    ctx.checkpoint()?;
    let (tab, filter) = cmp(comparison.resolve(&selection.tab, &selection.filter))?;
    let classes = cmp(comparison.classes(&tab, &filter))?;
    let mapped = comparison.has_mapping();
    let cur = comparison.current();
    let prev = comparison.previous();
    let cur_tabs = tabs_of(&cur.file, &cur.config);
    let tab_def = cur_tabs.iter().find(|t| t.id == tab);

    let mut probe = ViewQuery::new(&tab, &filter);
    probe.columns = selection.columns.clone();
    let columns = views::column_names_in(&cur_tabs, &cur.file, &cur.config, &probe)?;
    let mut header = columns.clone();
    header.push(STATUS_COLUMN.into());
    if mapped {
        header.push(PREVIOUS_ADDRESS_COLUMN.into());
    }
    let mut kinds: Vec<ColumnKind> = columns.iter().map(|c| column_kind(tab_def, c)).collect();
    kinds.push(ColumnKind::Text);
    if mapped {
        kinds.push(ColumnKind::Url);
    }
    writer.header(&header)?;

    let pairing = comparison.pairing();
    let mut count = 0u64;
    let mut emit = |writer: &mut dyn TableWriter,
                    names: &[String],
                    values: &[Value],
                    status: Option<&str>,
                    previous_address: Option<String>|
     -> Result<(), ExportError> {
        let mut out: Vec<Value> = columns
            .iter()
            .map(|c| {
                names
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(c))
                    .and_then(|i| values.get(i).cloned())
                    .unwrap_or(Value::Null)
            })
            .collect();
        out.push(status.map_or(Value::Null, |s| Value::Text(s.into())));
        if mapped {
            out.push(previous_address.map_or(Value::Null, Value::Text));
        }
        writer.row(&kinds, &out)?;
        count += 1;
        (ctx.progress)(count);
        Ok(())
    };
    let previous_of = |id: i64| -> Result<Option<String>, ExportError> {
        if !mapped {
            return Ok(None);
        }
        let Some(p) = u32::try_from(id)
            .ok()
            .and_then(|id| pairing.previous_of(id))
        else {
            return Ok(None);
        };
        Ok(addresses(&prev.file, [p])?.remove(&p))
    };

    match selection.class {
        None | Some(CompareClass::Added) | Some(CompareClass::New) => {
            let mut q = ViewQuery::new(&tab, &filter);
            q.columns = selection.columns.clone();
            q.limit = 500;
            q.start = PageStart::Offset(0);
            loop {
                ctx.checkpoint()?;
                let page = views::query_view_in(&cur_tabs, &cur.file, &cur.config, &q)?;
                for (key, row) in page.row_keys.iter().zip(&page.rows) {
                    let id = u32::try_from(key.0).unwrap_or(u32::MAX);
                    let status = classes.as_ref().map(|k| {
                        if k.added.contains(id) {
                            "Added"
                        } else if k.new.contains(id) {
                            "New"
                        } else {
                            "Unchanged"
                        }
                    });
                    let keep = match selection.class {
                        None => true,
                        Some(c) => status == Some(c.as_str()),
                    };
                    if keep {
                        emit(
                            &mut *writer,
                            &page.columns,
                            row,
                            status,
                            previous_of(key.0)?,
                        )?;
                    }
                }
                match page.next_cursor {
                    Some(cursor) if !page.rows.is_empty() => q.start = PageStart::Cursor(cursor),
                    _ => break,
                }
            }
        }
        Some(class @ (CompareClass::Removed | CompareClass::Missing)) => {
            let Some(k) = classes.as_ref() else {
                return Ok(0);
            };
            let ids = k.ids(class, pairing);
            let side = comparison.side(class.side());
            let rows = rows_by_ids(
                &side.file,
                &side.config,
                &tab,
                &selection.columns,
                &ids,
                ctx,
            )?;
            for (names, key, values) in rows {
                ctx.checkpoint()?;
                let previous_address = match class {
                    CompareClass::Missing if mapped => {
                        let id = u32::try_from(key.0).unwrap_or(u32::MAX);
                        addresses(&prev.file, [id])?.remove(&id)
                    }
                    _ => previous_of(key.0)?,
                };
                emit(
                    &mut *writer,
                    &names,
                    &values,
                    Some(class.as_str()),
                    previous_address,
                )?;
            }
        }
    }
    Ok(count)
}

/// Exportiert einen Tab im Vergleichsmodus in eine Datei.
pub fn export_tab(
    comparison: &mut Comparison,
    selection: &TabSelection,
    path: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let cancel = CancelToken::new();
    let mut ctx = plain_ctx(&cancel);
    let sheet = format!("{} {}", selection.tab, selection.filter);
    let mut writer = open_writer(
        path,
        format,
        &sheet,
        &writer_options(&comparison.current().config),
    )?;
    write_tab(comparison, selection, writer.as_mut(), &mut ctx)?;
    writer.finish()
}

fn plain_ctx(cancel: &CancelToken) -> ExportCtx<'_> {
    ExportCtx {
        cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// OUT-149 Inlinks/Outlinks-Vergleich

/// Auswahl der Linkansicht im Vergleichsmodus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkSelection {
    /// Crawl der im oberen Fenster markierten URL.
    pub side: Side,
    /// Kennung der URL in diesem Crawl.
    pub url_id: i64,
    /// Inlinks oder Outlinks.
    pub direction: LinkDirection,
    /// Quelle.
    pub source: LinkSource,
    /// Änderung.
    pub change: ChangeFilter,
}

/// Schreibt den Detail-Export der Linkansicht: Standard-Linkformat plus `Link Change`, `Crawl`, `Previous Anchor`.
pub fn write_links(
    comparison: &Comparison,
    selection: &LinkSelection,
    writer: &mut dyn TableWriter,
    ctx: &mut ExportCtx<'_>,
) -> Result<u64, ExportError> {
    ctx.checkpoint()?;
    let links = cmp(comparison.links(selection.side, selection.url_id, selection.direction))?;
    let mut header = link_header();
    header.extend(CHANGE_COLUMNS.map(String::from));
    let mut kinds = link_kinds();
    kinds.extend([ColumnKind::Text; 3]);
    writer.header(&header)?;
    let mut n = 0;
    for row in links.view(selection.source, selection.side, selection.change) {
        ctx.checkpoint()?;
        writer.row(&kinds, &row.export_values())?;
        n += 1;
        (ctx.progress)(n);
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------
// OUT-113 Reports

/// Writer, der nur Zeilen durchlässt, deren Spalte `Address` im Segment liegt (Start-URL der Kettenreports).
struct SegmentWriter<'a> {
    inner: &'a mut dyn TableWriter,
    allowed: HashSet<String>,
    column: Option<usize>,
}

impl TableWriter for SegmentWriter<'_> {
    fn header(&mut self, columns: &[String]) -> Result<(), ExportError> {
        self.column = columns.iter().position(|c| c == "Address");
        if self.column.is_none() {
            tracing::warn!("Report ohne Spalte Address: Segment schränkt die Zeilen nicht ein");
        }
        self.inner.header(columns)
    }
    fn row(&mut self, kinds: &[ColumnKind], values: &[Value]) -> Result<(), ExportError> {
        let keep = match self.column.and_then(|i| values.get(i)) {
            Some(Value::Text(address)) => self.allowed.contains(address),
            Some(_) => false,
            None => true,
        };
        if keep {
            self.inner.row(kinds, values)?;
        }
        Ok(())
    }
    fn finish(self: Box<Self>) -> Result<ExportFileInfo, ExportError> {
        Err(ExportError::Write(
            "SegmentWriter wird nicht abgeschlossen".into(),
        ))
    }
}

fn segment_addresses(
    file: &CrawlFile,
    scope: &dyn SegmentScope,
) -> Result<HashSet<String>, ExportError> {
    let sql = format!(
        "SELECT u.address FROM urls u WHERE {}",
        scope.population_sql()
    );
    let mut stmt = crate::links_export::sql(file.conn().prepare(&sql))?;
    let rows = crate::links_export::sql(stmt.query_map([], |r| r.get::<_, String>(0)))?;
    rows.map(crate::links_export::sql).collect()
}

/// Schreibt einen Report im Segmentkontext (OUT-113, OUT-011): zeilenbasierte Reports mit Spalte `Address`
/// (Start-URL, z. B. Redirect Chains) nur mit Zeilen im Segment; der Crawl Overview zählt die Filter des Segments.
pub fn write_report_in_segment(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: &dyn SegmentScope,
    writer: &mut dyn TableWriter,
    ctx: &mut ExportCtx<'_>,
) -> Result<u64, ExportError> {
    let tabs = crate::pipeline::scoped_tabs(file, cfg, scope);
    let job = TableJob {
        entry_name: name,
        tabs: &tabs,
        file,
        cfg,
    };
    if report_catalog::resolve(name).is_ok_and(|d| d.kind == ReportKind::CrawlOverview) {
        return crate::reports::save_report_to(&job, writer, ctx);
    }
    let mut filtered = SegmentWriter {
        inner: writer,
        allowed: segment_addresses(file, scope)?,
        column: None,
    };
    crate::reports::save_report_to(&job, &mut filtered, ctx)
}

/// Speichert einen Report im Vergleichsmodus (OUT-113): Crawl Overview mit Vergleichsspalten (OUT-147), alle übrigen
/// Reports für den Current-Crawl.
pub fn save_report(
    comparison: &mut Comparison,
    name: &str,
    out_dir: &Path,
    format: Format,
    cancel: &CancelToken,
) -> Result<ReportResult, ExportError> {
    let cfg = comparison.current().config.clone();
    if let Ok(def) = report_catalog::resolve(name) {
        if def.kind == ReportKind::CrawlOverview {
            let path = out_dir.join(def.file_name(format.extension()));
            let mut writer = open_writer(&path, format, def.file_stem, &writer_options(&cfg))?;
            let mut ctx = plain_ctx(cancel);
            write_crawl_overview(comparison, &[], writer.as_mut(), &mut ctx)?;
            let info = writer.finish()?;
            return Ok(ReportResult {
                warnings: info.warnings.clone(),
                files: vec![info],
            });
        }
    }
    let current = comparison.current();
    crate::reports::save_report(&current.file, &current.config, name, out_dir, format)
}

/// Alle Reportnamen des Menüs (Katalog und Tabellen-Einträge), die im Vergleichsmodus bedienbar sind.
pub fn report_names() -> Vec<String> {
    let mut names = report_catalog::names();
    names.extend(
        crate::reports::report_tables()
            .iter()
            .map(|e| e.name.to_string()),
    );
    names
}
