//! Export-Pipeline (OUT-001): Datenquelle → Zeilen-Stream → `TableWriter` in konstantem Speicher; Bulk-Exporte
//! über den Katalog.
//!
//! * Tab-Exporte lesen ausschließlich über `views::stream_view` (Filter, Sortierung, Suche und CRAWL-089 sind
//!   dort geregelt, keine eigene `ORDER BY`-Logik). Die Zeilen gehen einzeln an den Schreiber; es wird nie
//!   eine Ergebnismenge gesammelt.
//! * Die Kopfzeile entsteht vor dem ersten Datensatz und fehlt nie, auch nicht bei null Zeilen oder wenn der
//!   Filter nicht auswertbar ist (`Crawl-Analyse erforderlich`); solche Fälle melden einen Hinweis in
//!   [`ExportFileInfo::warnings`].
//! * Bulk-Exporte des Katalogs: [`export_bulk`] (Links, Response-Codes-Inlinks und `Issues:All Issues`).
//! * Reports des Katalogs: [`export_report`] (Crawl Overview, Redirects).
//!
//! `*_in`-Varianten nehmen eine eigene Tab-Liste statt `tabs::all()` (Tests, abweichende Registry).

use std::path::Path;

use os_analyze::defs::{ColumnDef, ColumnKind, ColumnSql, TabDef};
use os_analyze::names;
use os_analyze::views::{self, PageStart, ViewQuery};
use os_analyze::ViewError;
use os_model::{CrawlConfig, FilterState, Value};
use os_store::{CrawlFile, StoreError};

use crate::catalog::{self, bulk_issues, bulk_links, ExportKind, ExportRef};
use crate::error::ExportError;
use crate::fs_paths;
use crate::links_export::export_link_entry;
use crate::naming;
use crate::types::{ExportFileInfo, Format, ReportResult, TableWriter, WriterOptions};

/// Schreiboptionen aus der Konfiguration (`export.utf8_bom`, `export.line_ending`).
pub fn writer_options(cfg: &CrawlConfig) -> WriterOptions {
    WriterOptions {
        utf8_bom: cfg.export.utf8_bom,
        line_ending: cfg.export.line_ending,
        ..WriterOptions::default()
    }
}

/// Hinweistext für einen Filter, der keine Zeilen liefern kann (ONPAGE-007, OUT-052).
pub(crate) fn filter_warning(tab: &str, filter: &str, state: &FilterState) -> String {
    let reason = match state {
        FilterState::Ready => return String::new(),
        FilterState::NeedsRecrawl => "Neu crawlen erforderlich".into(),
        FilterState::Incomplete { unchecked } => {
            format!("Unvollständig: {unchecked} ungeprüfte Seiten")
        }
        FilterState::NeedsCrawlAnalysis => "Crawl-Analyse ausführen".to_string(),
        FilterState::Stale => "Crawl-Analyse veraltet, erneut ausführen".to_string(),
        FilterState::NeedsConfig(key) => format!("Einstellung {key} einschalten"),
        FilterState::NeedsRendering => "JavaScript-Rendering erforderlich".to_string(),
    };
    format!("{tab}:{filter}: leerer Export – {reason}")
}

/// Spaltenart zu einem Spaltennamen der Ausgabe: Spalten des Tabs über ihren Schlüssel, Mehrfachspalten
/// (TECH-004) über `<Basis> <n>`; unbekannte Namen gelten als Text.
fn column_kind(tab: &TabDef, name: &str) -> ColumnKind {
    let by_key: Option<&ColumnDef> = tab.columns.iter().find(|c| c.key == name).or_else(|| {
        tab.columns
            .iter()
            .find(|c| c.key.eq_ignore_ascii_case(name))
    });
    if let Some(c) = by_key {
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

/// Lässt den Abbruch des Zeilen-Streams durch einen Schreibfehler als `ViewError` durch `stream_view`.
fn aborted() -> ViewError {
    ViewError::Store(StoreError::InvalidInput(
        "Export abgebrochen (Schreibfehler)".to_string(),
    ))
}

/// Streamt die Ansicht `q` in eine Datei (Kernstück von OUT-001/OUT-020).
///
/// `q.columns` bestimmt die Spalten (`Default` = sichtbare Standardspalten, `All` = alle, Erweiterungsspalten
/// hinter den Referenzspalten), `q.sort`/`q.search` Reihenfolge und Teilmenge; `q.limit` und `q.start` gelten
/// nicht – es werden alle Zeilen geschrieben. Gibt die Kopfzeile auch bei null Zeilen aus.
pub fn export_view(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    path: &Path,
    format: Format,
    sheet: &str,
) -> Result<ExportFileInfo, ExportError> {
    let cancel = os_model::CancelToken::new();
    let mut ctx = crate::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    export_view_with(tabs, file, cfg, q, path, format, sheet, true, &mut ctx)?
        .ok_or_else(|| ExportError::Write("Missing view output".into()))
}

/// Controlled single-view export. Returns `None` for an omitted empty table or
/// a sheet written into `ctx.workbook`; explicit columns override saved columns.
#[allow(clippy::too_many_arguments)]
pub fn export_view_with(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    path: &Path,
    format: Format,
    sheet: &str,
    preserve_upload_order: bool,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<Option<ExportFileInfo>, ExportError> {
    let mut warnings = Vec::new();
    let info = snapshot(file, || {
        crate::output::write_table(path, format, sheet, &writer_options(cfg), ctx, |writer| {
            warnings = write_view_to(tabs, file, cfg, q, preserve_upload_order, writer)?;
            Ok(())
        })
    })?;
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    Ok(info.map(|mut info| {
        info.warnings.extend(warnings);
        info
    }))
}

/// A read transaction shared by header discovery and all selected exports.
/// Nested callers keep the caller's existing transaction intact.
pub fn snapshot<T>(
    file: &CrawlFile,
    run: impl FnOnce() -> Result<T, ExportError>,
) -> Result<T, ExportError> {
    let _transaction = if file.conn().is_autocommit() {
        Some(crate::links_export::sql(
            file.conn().unchecked_transaction(),
        )?)
    } else {
        None
    };
    // BEGIN DEFERRED acquires its snapshot at the first read, before any output is opened.
    file.meta()?;
    run()
}

/// Custom writer entry point, retaining write failures unchanged.
pub fn write_view(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    open: impl FnOnce() -> Result<Box<dyn TableWriter>, ExportError>,
) -> Result<ExportFileInfo, ExportError> {
    snapshot(file, || {
        // Resolve names before opening the output.
        views::plan_in(tabs, q, cfg)?;
        let mut writer = open()?;
        let warnings = write_view_to(tabs, file, cfg, q, true, writer.as_mut())?;
        let mut info = writer.finish()?;
        info.warnings.extend(warnings);
        Ok(info)
    })
}

/// Streams a view into an existing table. `false` opts out of upload order.
/// Explicit columns override `ui.columns`; Address is always first.
pub fn write_view_to(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    preserve_upload_order: bool,
    writer: &mut dyn TableWriter,
) -> Result<Vec<String>, ExportError> {
    use os_analyze::views::ColumnSelection;
    let tab = names::find_tab(tabs, &q.tab).ok_or_else(|| {
        ExportError::Name(os_analyze::NameError::Unknown {
            input: q.tab.clone(),
            suggestions: names::suggestions(tabs, &q.tab),
        })
    })?;
    let mut q = q.clone();
    let mut warnings = Vec::new();
    if q.columns == ColumnSelection::Default {
        if let Some((_, columns)) = cfg
            .ui
            .columns
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(tab.id))
        {
            q.columns = ColumnSelection::Named(columns.clone());
        }
    }
    if let ColumnSelection::Named(columns) = &mut q.columns {
        let mut valid = Vec::new();
        if views::column_name_resolves(tab, "Address") {
            valid.push("Address".to_string());
        }
        for column in columns.iter() {
            if !views::column_name_resolves(tab, column) {
                warnings.push(format!("{}: unknown column '{column}' skipped", tab.id));
            } else if !valid.iter().any(|v| v.eq_ignore_ascii_case(column)) {
                valid.push(column.clone());
            }
        }
        *columns = valid;
    }
    let mut probe = q.clone();
    probe.limit = 1;
    probe.start = PageStart::Offset(0);
    let header = views::query_view_in(tabs, file, cfg, &probe)?.columns;
    let kinds: Vec<ColumnKind> = header.iter().map(|n| column_kind(tab, n)).collect();
    let state = views::state_of_in(tabs, file, cfg, tab.id, &q.filter)?;
    let list_order = preserve_upload_order
        && cfg.mode == os_model::CrawlMode::List
        && tab.id == "Internal"
        && q.filter.eq_ignore_ascii_case("All");
    // Sitemap list sources can have only a source marker, without uploaded URL rows.
    let list_order = list_order && crate::links_export::sql(file.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM list_input WHERE outcome NOT IN ('ignored_line','sitemap'))",
        [], |r| r.get::<_, bool>(0),
    ))?;
    if list_order {
        let mut list_header = vec!["Original Url".to_string()];
        list_header.extend(header.clone());
        // Invalid/excluded inputs need an explicit status even in a reduced column selection.
        let status_index = list_header.iter().position(|h| h == "Status");
        if status_index.is_none() {
            list_header.push("Status".into());
        }
        let mut list_kinds = vec![ColumnKind::Url];
        list_kinds.extend(kinds.clone());
        if status_index.is_none() {
            list_kinds.push(ColumnKind::Text);
        }
        writer.header(&list_header)?;
        let expressions: Vec<String> = header
            .iter()
            .map(|name| list_column_expr(tab, name))
            .collect::<Result<_, _>>()?;
        let plan = views::plan_in(tabs, &q, cfg)?;
        let condition = format!(
            "({}) AND ({}) AND ({})",
            plan.population_sql,
            plan.filter_sql,
            plan.search_sql.as_deref().unwrap_or("1")
        );
        let query = upload_order_sql(&expressions, &condition);
        let mut stmt = crate::links_export::sql(file.conn().prepare(&query))?;
        for (name, value) in &plan.params {
            if let Some(index) = crate::links_export::sql(stmt.parameter_index(name))? {
                crate::links_export::sql(match value {
                    Value::Null => stmt.raw_bind_parameter(index, Option::<i64>::None),
                    Value::Bool(v) => stmt.raw_bind_parameter(index, i64::from(*v)),
                    Value::Int(v) => stmt.raw_bind_parameter(index, *v),
                    Value::Float(v) => stmt.raw_bind_parameter(index, *v),
                    Value::Text(v) => stmt.raw_bind_parameter(index, v.as_str()),
                })?;
            }
        }
        let mut rows = stmt.raw_query();
        while let Some(row) = crate::links_export::sql(rows.next())? {
            let original: String = crate::links_export::sql(row.get(0))?;
            let outcome: String = crate::links_export::sql(row.get(1))?;
            let mut values = vec![Value::Text(original)];
            for (i, kind) in kinds.iter().enumerate() {
                let index = i + 2;
                let value = match kind {
                    ColumnKind::Bool => crate::links_export::sql(row.get::<_, Option<i64>>(index))?
                        .map_or(Value::Null, |v| Value::Bool(v != 0)),
                    ColumnKind::Int => crate::links_export::sql(row.get::<_, Option<i64>>(index))?
                        .map_or(Value::Null, Value::Int),
                    ColumnKind::Float { .. } | ColumnKind::Seconds3 => {
                        let value = crate::links_export::sql(row.get::<_, Option<f64>>(index))?;
                        let decimals = match kind {
                            ColumnKind::Float { decimals } => *decimals,
                            _ => 3,
                        };
                        let power = 10f64.powi(i32::from(decimals));
                        value.map_or(Value::Null, |v| {
                            Value::Float(
                                ((if *kind == ColumnKind::Seconds3 {
                                    v / 1000.0
                                } else {
                                    v
                                }) * power)
                                    .round()
                                    / power,
                            )
                        })
                    }
                    _ => {
                        let text = row
                            .get::<_, Option<String>>(index)
                            .or_else(|_| {
                                row.get::<_, Option<i64>>(index)
                                    .map(|v| v.map(|v| v.to_string()))
                            })
                            .or_else(|_| {
                                row.get::<_, Option<f64>>(index)
                                    .map(|v| v.map(|v| v.to_string()))
                            });
                        crate::links_export::sql(text)?.map_or(Value::Null, Value::Text)
                    }
                };
                values.push(value);
            }
            if status_index.is_none() {
                values.push(Value::Null);
            }
            if matches!(outcome.as_str(), "invalid" | "excluded" | "not_included") {
                if let Some(address) = list_header.iter().position(|h| h == "Address") {
                    values[address] = Value::Null;
                }
                let index = status_index.unwrap_or(list_header.len() - 1);
                values[index] = Value::Text(
                    if outcome == "invalid" {
                        "Invalid"
                    } else {
                        "Excluded"
                    }
                    .into(),
                );
            }
            writer.row(&list_kinds, &values)?;
        }
    } else {
        writer.header(&header)?;
        let mut failure = None;
        let width = header.len();
        let mut padded = Vec::new();
        let streamed = views::stream_view_in(tabs, file, cfg, &q, &mut |values: &[Value]| {
            let row = if values.len() == width {
                values
            } else {
                padded.clear();
                padded.extend(values.iter().take(width).cloned());
                padded.resize(width, Value::Null);
                &padded
            };
            writer.row(&kinds, row).map_err(|e| {
                failure = Some(e);
                aborted()
            })
        });
        if let Some(e) = failure {
            return Err(e);
        }
        streamed?;
    }
    if state != FilterState::Ready {
        warnings.push(filter_warning(tab.id, &q.filter, &state));
    }
    Ok(warnings)
}

/// Führt einen Bulk-Export aus dem Katalog aus (z. B. „All Inlinks“); kann mehrere Dateien liefern.
///
/// Dateiname nach OUT-008 aus dem kanonischen Namen (ohne Datum), Ablage in `out_dir`. `Issues:All Issues`
/// schreibt den Ordner `issues/` mit einer Datei je Issue (OUT-063); die Hinweise dieses Exports stehen in
/// den `warnings` der letzten Datei (`issues_overview`).
pub fn export_bulk(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    export_bulk_in(
        &os_analyze::tabs::configured_for_file(cfg, file),
        file,
        cfg,
        name,
        out_dir,
        format,
    )
}

/// Wie [`export_bulk`], gegen eine eigene Tab-Liste.
pub fn export_bulk_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    if file.conn().is_autocommit() {
        os_store::link_index::ensure_if_idle(file)?;
    }
    snapshot(file, || {
        export_bulk_in_snapshot(tabs, file, cfg, name, out_dir, format)
    })
}

fn export_bulk_in_snapshot(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    // Auflösung über den gemeinsamen Katalog: Vorschläge bei Tippfehlern stammen aus allen Bulk-Namen.
    let lookup = catalog::bulk_generic::canonical_name(name, cfg).unwrap_or(name);
    let canonical = match catalog::resolve_with(ExportKind::Bulk, lookup, cfg)? {
        ExportRef::Bulk(canonical) => canonical,
        other => unreachable!("Bulk-Auflösung lieferte {other:?}"),
    };
    if let Some(kind) = catalog::bulk_web::page_export(&canonical) {
        return catalog::bulk_web::export_pages(file, cfg, kind, out_dir, None);
    }
    if !cfg.export.inlinks_for_all_filters && catalog::bulk_generic::is_generic_name(&canonical) {
        return Err(ExportError::Unsupported(format!(
            "{canonical}: enable export.inlinks_for_all_filters to export generic inlinks"
        )));
    }
    if canonical == bulk_issues::BULK_NAME {
        let mut result = bulk_issues::export_in(tabs, file, cfg, out_dir, format)?;
        // Hinweise des Gesamtexports (z. B. fehlende Beschreibungstexte) gehen an die letzte Datei, damit
        // der Aufrufer sie über `ExportFileInfo::warnings` findet; Voraussetzungen stehen dort schon.
        if let Some(last) = result.files.last_mut() {
            for w in result.warnings {
                if !last.warnings.contains(&w) {
                    tracing::warn!("{w}");
                    last.warnings.push(w);
                }
            }
        }
        return Ok(result.files);
    }
    if let Some(entry) = catalog::dynamic_entries(cfg)
        .into_iter()
        .find(|e| e.name == canonical)
    {
        fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
            path: out_dir.into(),
            source,
        })?;
        let path = out_dir.join(naming::file_name(
            &entry.file_stem,
            None,
            format.extension(),
        ));
        let job = catalog::TableJob {
            tabs,
            file,
            cfg,
            entry_name: &canonical,
        };
        let mut writer =
            crate::output::open_writer(&path, format, &entry.name, &writer_options(cfg))?;
        writer.header(&entry.header)?;
        (entry.run)(&job, writer.as_mut())?;
        return Ok(vec![writer.finish()?]);
    }
    // M2: Tabellen-Einträge der Gruppen `catalog/bulk_<gruppe>.rs` (Stub: nur Kopfzeile, K-14).
    if let Some(entry) = catalog::find_table(&catalog::bulk_tables(), &canonical) {
        fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
            path: out_dir.to_path_buf(),
            source,
        })?;
        let path = out_dir.join(entry.file_name(format.extension()));
        let job = catalog::TableJob {
            tabs,
            file,
            cfg,
            entry_name: &canonical,
        };
        return Ok(vec![catalog::export_table(entry, &job, &path, format)?]);
    }
    let entry = bulk_links::resolve(&canonical)?;
    fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;
    let path = out_dir.join(naming::file_name(&entry.name, None, format.extension()));
    let info = export_link_entry(tabs, file, cfg, entry, &path, format, &entry.name)?;
    Ok(vec![info])
}

/// Führt einen Report aus dem Katalog aus (z. B. `Crawl Overview`, `Redirects:Redirect Chains`;
/// `--save-report`). Dateiname nach OUT-008 aus dem Dateinamen-Stamm des Reports, Ablage in `out_dir`;
/// fehlende Voraussetzungen stehen in [`ReportResult::warnings`] (OUT-112).
pub fn export_report(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    export_report_in(
        &os_analyze::tabs::configured_for_file(cfg, file),
        file,
        cfg,
        name,
        out_dir,
        format,
    )
}

/// Wie [`export_report`], gegen eine eigene Tab-Liste.
pub fn export_report_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    if file.conn().is_autocommit() {
        os_store::link_index::ensure_if_idle(file)?;
    }
    snapshot(file, || {
        export_report_in_snapshot(tabs, file, cfg, name, out_dir, format)
    })
}

fn export_report_in_snapshot(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;
    crate::reports::save_report_in(tabs, file, cfg, name, out_dir, format)
}

/// Options shared by CLI, tasks and the multi-export command.
pub struct BatchOptions<'a> {
    pub format: Format,
    pub directory: &'a Path,
    pub crawl_name: &'a str,
    pub single_workbook: bool,
}

/// Executes selected tables in order against one consistent read snapshot.
pub fn export_batch(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selections: &[crate::types::ExportSelection],
    options: &BatchOptions<'_>,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<crate::types::BatchResult, ExportError> {
    snapshot(file, || {
        let tabs = os_analyze::tabs::configured_for_file(cfg, file);
        export_batch_in(&tabs, file, cfg, selections, options, ctx)
    })
}

fn export_batch_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selections: &[crate::types::ExportSelection],
    options: &BatchOptions<'_>,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<crate::types::BatchResult, ExportError> {
    snapshot(file, || {
        use crate::types::WorkbookTarget;
        let mut result = crate::types::BatchResult::default();
        let consolidate = options.single_workbook && options.format == Format::Xlsx;
        if options.single_workbook && options.format == Format::Csv {
            result
                .warnings
                .push("CSV cannot contain multiple worksheets; single-workbook ignored".into());
        }
        let destination = options.directory.join(naming::file_name(
            &format!("{} export", options.crawl_name),
            None,
            "xlsx",
        ));
        let pending = if consolidate {
            Some(crate::output::PendingFile::new(&destination)?)
        } else {
            None
        };
        let mut workbook = if let Some(pending) = &pending {
            Some({
                let mut writer =
                    crate::xlsx::XlsxWriter::create(&pending.path, "Export", &writer_options(cfg))?;
                writer.set_cancel(ctx.cancel.clone());
                writer
            })
        } else {
            None
        };
        let mut tables_written = 0;
        let done = std::cell::Cell::new(0u64);
        for selection in selections {
            ctx.checkpoint()?;
            let kind = match selection.kind.to_ascii_lowercase().as_str() {
                "tab" | "view" => ExportKind::Tab,
                "bulk" => ExportKind::Bulk,
                "report" | "reports" => ExportKind::Report,
                _ => {
                    result.warnings.push(format!(
                        "{}: unknown export kind '{}'",
                        selection.name, selection.kind
                    ));
                    result.skipped.push(selection.name.clone());
                    continue;
                }
            };
            let lookup = if kind == ExportKind::Bulk {
                catalog::bulk_generic::canonical_name(&selection.name, cfg)
                    .unwrap_or(&selection.name)
            } else {
                &selection.name
            };
            let resolved = match catalog::resolve_with(kind, lookup, cfg) {
                Ok(r) => r,
                Err(e) => {
                    result.warnings.push(e.to_string());
                    result.skipped.push(selection.name.clone());
                    continue;
                }
            };
            let (name, stem, requires) = match &resolved {
                ExportRef::Tab { tab, filter } => (
                    format!("{tab}:{filter}"),
                    format!("{tab}:{filter}"),
                    &[][..],
                ),
                ExportRef::Bulk(name) => {
                    let tables = catalog::bulk_tables();
                    match catalog::find_table(&tables, name) {
                        Some(e) => (name.clone(), e.file_stem.to_string(), e.requires),
                        None => (name.clone(), name.clone(), &[][..]),
                    }
                }
                ExportRef::Report(name) => {
                    if let Ok(e) = catalog::reports::resolve(name) {
                        (name.clone(), e.file_stem.to_string(), e.requires)
                    } else {
                        let tables = crate::reports::report_tables();
                        let e = catalog::find_table(&tables, name).expect("resolved report");
                        (name.clone(), e.file_stem.to_string(), e.requires)
                    }
                }
            };
            let filter_state = match &resolved {
                ExportRef::Tab { tab, filter } => Some((tab.as_str(), filter.as_str())),
                ExportRef::Bulk(name) if catalog::bulk_generic::is_generic_name(name) => {
                    name.rsplit_once(':').and_then(|(tab, filter)| {
                        filter.strip_suffix(" Inlinks").map(|filter| (tab, filter))
                    })
                }
                _ => None,
            };
            if let Some((tab, filter)) = filter_state {
                let state = match views::state_of_in(tabs, file, cfg, tab, filter) {
                    Ok(state) => state,
                    Err(ViewError::Name(e)) => {
                        result.warnings.push(e.to_string());
                        result.skipped.push(selection.name.clone());
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                };
                if state != FilterState::Ready {
                    result.warnings.push(filter_warning(tab, filter, &state));
                    result.skipped.push(name);
                    continue;
                }
            }
            let generic_disabled = kind == ExportKind::Bulk
                && !cfg.export.inlinks_for_all_filters
                && catalog::bulk_generic::is_generic_name(&name);
            let available = crate::reports::prereq::availability(&name, requires, file, cfg);
            if generic_disabled || !available.enabled {
                result.skipped.push(name.clone());
                result.warnings.push(format!(
                    "{name}: {}",
                    available
                        .reason
                        .unwrap_or_else(|| "generic inlink exports disabled".into())
                ));
                continue;
            }
            if kind == ExportKind::Bulk {
                if let Some(page_kind) = catalog::bulk_web::page_export(&name) {
                    let files = catalog::bulk_web::export_pages(
                        file,
                        cfg,
                        page_kind,
                        options.directory,
                        Some(ctx),
                    )?;
                    let rows = files.last().map_or(0, |info| info.rows);
                    if rows == 0 && ctx.skip_empty {
                        result.skipped.push(name);
                    }
                    done.set(done.get() + rows);
                    (ctx.progress)(done.get());
                    for info in files {
                        result.warnings.extend(info.warnings.clone());
                        result.files.push(info);
                    }
                    continue;
                }
            }
            let path =
                options
                    .directory
                    .join(naming::file_name(&stem, None, options.format.extension()));
            let base = done.get();
            let progress = |count| {
                done.set(base + count);
                (ctx.progress)(base + count);
            };
            let mut table_ctx = crate::types::ExportCtx {
                cancel: ctx.cancel,
                progress: &progress,
                skip_empty: ctx.skip_empty,
                workbook: if matches!(
                    name.as_str(),
                    "Web:Screenshots" | "Web:All Page Source" | "Web:All Page Text"
                ) {
                    None
                } else {
                    match workbook.as_mut() {
                        Some(w) => Some(w as &mut dyn WorkbookTarget),
                        None => ctx
                            .workbook
                            .as_deref_mut()
                            .map(|w| w as &mut dyn WorkbookTarget),
                    }
                },
                skipped: Vec::new(),
            };
            let uses_workbook = table_ctx.workbook.is_some();
            let job = catalog::TableJob {
                tabs,
                file,
                cfg,
                entry_name: &name,
            };
            let mut inner_ctx = crate::types::ExportCtx {
                cancel: ctx.cancel,
                progress: &|_| {},
                skip_empty: false,
                workbook: None,
                skipped: Vec::new(),
            };
            let mut warnings = Vec::new();
            let info = crate::output::write_table(
                &path,
                options.format,
                &name.replace(':', " "),
                &writer_options(cfg),
                &mut table_ctx,
                |writer| {
                    match &resolved {
                        ExportRef::Tab { tab, filter } => {
                            warnings.extend(write_view_to(
                                tabs,
                                file,
                                cfg,
                                &ViewQuery::new(tab, filter),
                                true,
                                writer,
                            )?);
                        }
                        ExportRef::Bulk(name) => {
                            if name == bulk_issues::BULK_NAME {
                                bulk_issues::export_to(&job, writer, &mut inner_ctx)?;
                            } else if let Some(e) = catalog::dynamic_entries(cfg)
                                .into_iter()
                                .find(|e| e.name == *name)
                            {
                                writer.header(&e.header)?;
                                (e.run)(&job, writer)?;
                            } else if let Some(e) =
                                catalog::find_table(&catalog::bulk_tables(), name)
                            {
                                writer.header(&match e.header_fn {
                                    Some(f) => f(&job)?,
                                    None => e.header.iter().map(|s| (*s).to_owned()).collect(),
                                })?;
                                (e.run)(&job, writer)?;
                            } else {
                                crate::links_export::export_link_entry_to(
                                    bulk_links::resolve(name)?,
                                    &job,
                                    writer,
                                    &mut inner_ctx,
                                )?;
                            }
                        }
                        ExportRef::Report(_) => {
                            crate::reports::save_report_to(&job, writer, &mut inner_ctx)?;
                        }
                    }
                    Ok(())
                },
            )?;
            let empty = !table_ctx.skipped.is_empty();
            if empty {
                result.skipped.push(name);
            } else if uses_workbook {
                tables_written += 1;
            }
            if let Some(info) = info {
                result.warnings.extend(info.warnings.clone());
                result.files.push(info);
            }
            result.warnings.extend(warnings);
        }
        if let (Some(workbook), Some(pending)) = (workbook, pending) {
            if tables_written > 0 {
                ctx.checkpoint()?;
                let info = Box::new(workbook).finish()?;
                ctx.checkpoint()?;
                let info = pending.publish(info)?;
                result.warnings.extend(info.warnings.clone());
                result.files.push(info);
            }
        }
        ctx.skipped.extend(result.skipped.clone());
        Ok(result)
    })
}

/// Keep the two primary-key joins separate: a parenthesized right-hand join
/// causes SQLite to materialize all URLs before it can emit an uploaded row.
fn upload_order_sql(expressions: &[String], condition: &str) -> String {
    let columns = expressions
        .iter()
        .map(|expr| format!("CASE WHEN ({condition}) THEN ({expr}) END"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT i.original, i.outcome, {columns} FROM list_input i \
        LEFT JOIN urls u ON u.id = i.url_id \
        LEFT JOIN url_counters c ON c.url_id = u.id \
        WHERE i.outcome NOT IN ('ignored_line', 'sitemap') ORDER BY i.position"
    )
}

/// SQL for an already resolved output column in upload order. Multi-value names
/// come from the view header, so the list export uses exactly the same expansion.
fn list_column_expr(tab: &TabDef, name: &str) -> Result<String, ExportError> {
    let nth = |sql: &str, n: usize| {
        format!(
            "(SELECT * FROM ({}) LIMIT 1 OFFSET {})",
            sql.replace("?1", "u.id"),
            n - 1
        )
    };
    for column in &tab.columns {
        match &column.sql {
            ColumnSql::Expr(expr) if column.key.eq_ignore_ascii_case(name) => {
                return Ok((*expr).into())
            }
            ColumnSql::Multi { base, list_sql } | ColumnSql::MultiCrawlWide { base, list_sql } => {
                if let Some(n) = name
                    .strip_prefix(&format!("{base} "))
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| *n > 0)
                {
                    return Ok(nth(list_sql, n));
                }
            }
            ColumnSql::MultiPaired { base, parts } => {
                if let Some(rest) = name.strip_prefix(&format!("{base} ")) {
                    let (number, suffix) = rest.split_once(' ').unwrap_or((rest, ""));
                    if let (Some(n), Some(part)) = (
                        number.parse::<usize>().ok().filter(|n| *n > 0),
                        parts.iter().find(|p| p.suffix == suffix),
                    ) {
                        return Ok(nth(part.list_sql, n));
                    }
                }
            }
            _ => {}
        }
    }
    Err(ExportError::Write(format!(
        "Unresolved list column: {name}"
    )))
}

/// Row total for selections whose size is available without running their exporter.
/// `None` means a report/dynamic source cannot provide a total in advance.
pub fn batch_row_count(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selections: &[crate::types::ExportSelection],
) -> Result<Option<u64>, ExportError> {
    let tabs = os_analyze::tabs::configured_for_file(cfg, file);
    let mut total = 0u64;
    for selection in selections {
        match selection.kind.to_ascii_lowercase().as_str() {
            "tab" | "view" => {
                if let Ok(ExportRef::Tab { tab, filter }) =
                    catalog::resolve_with(ExportKind::Tab, &selection.name, cfg)
                {
                    let uploaded = if cfg.mode == os_model::CrawlMode::List
                        && tab == "Internal"
                        && filter == "All"
                    {
                        crate::links_export::sql(file.conn().query_row(
                            "SELECT COUNT(*) FROM list_input WHERE outcome NOT IN ('ignored_line','sitemap')", [], |r| r.get::<_, u64>(0)))?
                    } else {
                        0
                    };
                    total += if uploaded > 0 {
                        uploaded
                    } else {
                        let mut q = ViewQuery::new(tab, filter);
                        q.limit = 1;
                        views::query_view_in(&tabs, file, cfg, &q)?.total
                    };
                }
            }
            "bulk" => {
                if let Ok(entry) = bulk_links::resolve(&selection.name) {
                    if entry.selection == bulk_links::Selection::All {
                        total += crate::links_export::sql(file.conn().query_row(
                            "SELECT COUNT(*) FROM links l JOIN urls s ON s.id = l.source_id",
                            [],
                            |r| r.get::<_, u64>(0),
                        ))?;
                    } else {
                        return Ok(None);
                    }
                } else if catalog::bulk_generic::canonical_name(&selection.name, cfg).is_some()
                    || catalog::resolve_with(ExportKind::Bulk, &selection.name, cfg).is_ok()
                {
                    return Ok(None);
                }
            }
            "report" | "reports" => return Ok(None),
            _ => {}
        }
    }
    Ok(Some(total))
}

/// Prepares a registry for exports using an existing segment assignment (M2-K14).
/// The scope is an internal query provider, never raw SQL from an IPC argument.
pub fn scoped_tabs(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    scope: &dyn crate::types::SegmentScope,
) -> Vec<TabDef> {
    let mut tabs = os_analyze::tabs::configured_for_file(cfg, file);
    let condition = scope.population_sql();
    for tab in &mut tabs {
        tab.population =
            os_analyze::defs::intern(&format!("({}) AND ({condition})", tab.population));
    }
    tabs
}

#[cfg(test)]
#[path = "../tests/links_support.rs"]
mod query_plan_fixture;

#[cfg(test)]
mod query_plan_tests {
    use super::*;

    // spec: OUT-075
    #[test]
    fn unresolved_view_names_are_skipped_after_catalogue_resolution() {
        let dir = tempfile::tempdir().unwrap();
        let file = query_plan_fixture::new_file(dir.path(), "fixture.ospider");
        query_plan_fixture::exec(
            &file,
            include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
        );
        let cfg = CrawlConfig::default();
        let mut tabs = os_analyze::tabs::configured_for_file(&cfg, &file);
        // Simulate a saved catalogue entry absent from this crawl's view registry.
        tabs.retain(|tab| tab.id != "Page Titles");
        let selections = [
            crate::types::ExportSelection {
                kind: "tab".into(),
                name: "Page Titles:All".into(),
            },
            crate::types::ExportSelection {
                kind: "bulk".into(),
                name: "Page Titles:Missing Inlinks".into(),
            },
            crate::types::ExportSelection {
                kind: "tab".into(),
                name: "Internal:All".into(),
            },
        ];
        let cancel = os_model::CancelToken::new();
        let mut ctx = crate::types::ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: false,
            workbook: None,
            skipped: Vec::new(),
        };
        let result = export_batch_in(
            &tabs,
            &file,
            &cfg,
            &selections,
            &BatchOptions {
                format: Format::Csv,
                directory: dir.path(),
                crawl_name: "fixture",
                single_workbook: false,
            },
            &mut ctx,
        )
        .unwrap();
        assert_eq!(
            result.skipped,
            ["Page Titles:All", "Page Titles:Missing Inlinks"]
        );
        assert_eq!(result.warnings.len(), 2);
        assert!(result
            .warnings
            .iter()
            .all(|warning| warning.contains("Page Titles")));
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].rows, 3);
        assert_eq!(ctx.skipped, result.skipped);
        assert!(!dir.path().join("page_titles_all.csv").exists());
        assert!(!dir.path().join("page_titles_missing_inlinks.csv").exists());
    }

    // spec: PERF-008 [M2]
    #[test]
    fn upload_order_streams_without_materializing_the_crawl() {
        let dir = tempfile::tempdir().unwrap();
        let file = query_plan_fixture::new_file(dir.path(), "plan.ospider");
        let query = upload_order_sql(
            &["u.address".into(), "c.inlinks".into()],
            "u.is_internal = 1",
        );
        let mut stmt = file
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {query}"))
            .unwrap();
        let plan = stmt
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            !plan
                .iter()
                .any(|line| line.contains("MATERIALIZE") || line.starts_with("SCAN u")),
            "{plan:?}"
        );
        for alias in ["u", "c"] {
            assert!(
                plan.iter()
                    .any(|line| line.starts_with(&format!("SEARCH {alias} "))
                        && line.contains("PRIMARY KEY")),
                "{plan:?}"
            );
        }
    }
}
