//! OUT-011: exports consume the saved assignment, never re-evaluate its rules.
use crate::{
    catalog::{bulk_links, ExportKind},
    types::{ExportFileInfo, Format, ReportResult, SegmentScope},
    ExportError,
};
use os_analyze::{
    segments::{self, SegmentFilter},
    views::ViewQuery,
    TabDef,
};
use os_model::CrawlConfig;
use os_store::CrawlFile;
use std::path::Path;

impl SegmentScope for SegmentFilter {
    fn population_sql(&self) -> String {
        SegmentFilter::population_sql(self)
    }
}

/// An absent filter is the unfiltered export; `all` adds each configured segment.
pub fn selections(
    file: &CrawlFile,
    name: Option<&str>,
) -> Result<Vec<Option<SegmentFilter>>, ExportError> {
    let cfg = segments::saved_config(file).map_err(|e| ExportError::Write(e.to_string()))?;
    match name {
        None => Ok(vec![None]),
        Some("all") => Ok(std::iter::once(None)
            .chain(
                cfg.segments
                    .list
                    .iter()
                    .map(|s| Some(SegmentFilter::new(&s.name))),
            )
            .collect()),
        Some(name) if cfg.segments.list.iter().any(|s| s.name == name) => {
            Ok(vec![Some(SegmentFilter::new(name))])
        }
        Some(name) => Err(ExportError::Write(format!("unknown segment: {name}"))),
    }
}
fn tabs(file: &CrawlFile, cfg: &CrawlConfig, scope: Option<&SegmentFilter>) -> Vec<TabDef> {
    match scope {
        Some(scope) => crate::pipeline::scoped_tabs(file, cfg, scope),
        None => os_analyze::tabs::configured_for_file(cfg, file),
    }
}
fn suffix(scope: Option<&SegmentFilter>) -> Option<&str> {
    scope
        .and_then(|s| s.name.as_deref())
        .or_else(|| scope.map(|_| "unassigned"))
}

/// Reserve names for every saved segment before opening a writer. Single-segment
/// exports use the same plan as `all`, including on case-insensitive filesystems.
pub(crate) fn file_name(
    file: &CrawlFile,
    canonical: &str,
    scope: Option<&SegmentFilter>,
    extension: &str,
) -> Result<String, ExportError> {
    let mut registry = crate::fs_paths::NameRegistry::new();
    let unfiltered = crate::naming::file_name(canonical, None, extension);
    registry.claim(&unfiltered, "unfiltered");
    let Some(scope) = scope else {
        return Ok(unfiltered);
    };
    let cfg = segments::saved_config(file).map_err(|e| ExportError::Write(e.to_string()))?;
    for segment in &cfg.segments.list {
        let requested = crate::naming::file_name(canonical, Some(&segment.name), extension);
        let resolved = registry.claim(&requested, &format!("segment:{}", segment.name));
        if scope.name.as_deref() == Some(segment.name.as_str()) {
            return Ok(resolved);
        }
    }
    Ok(registry.claim(
        &crate::naming::file_name(canonical, suffix(Some(scope)), extension),
        &format!("scope:{:?}", scope.name),
    ))
}

pub fn export_view(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    query: &ViewQuery,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    export_view_with_options(
        file,
        cfg,
        query,
        scope,
        directory,
        format,
        &crate::pipeline::writer_options(cfg),
    )
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_view_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    query: &ViewQuery,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let registry = tabs(file, cfg, scope);
    let name = crate::tab_export::canonical_name(&registry, cfg, &query.tab, &query.filter)?;
    let path = directory.join(file_name(file, &name, scope, format.extension())?);
    crate::pipeline::export_view_with_options(
        &registry, file, cfg, query, &path, format, &query.tab, opts,
    )
}

/// Inlinks restrict targets. All Outlinks restrict sources; filter-specific inlinks
/// continue to use the tab's exact population and predicate.
pub fn export_bulk(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    export_bulk_with_options(
        file,
        cfg,
        name,
        scope,
        directory,
        format,
        &crate::pipeline::writer_options(cfg),
    )
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_bulk_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    crate::pipeline::snapshot(file, || {
        export_bulk_snapshot_with_options(file, cfg, name, scope, directory, format, opts)
    })
}
#[allow(clippy::too_many_arguments)]
fn export_bulk_snapshot_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    let tabs = tabs(file, cfg, scope);
    let crate::catalog::ExportRef::Bulk(canonical) =
        crate::catalog::resolve_with(ExportKind::Bulk, name, cfg)?
    else {
        unreachable!("bulk resolution")
    };
    if let Ok(entry) = bulk_links::resolve(&canonical) {
        let mut entry = entry.clone();
        if let Some(scope) = scope {
            if entry.selection == bulk_links::Selection::All {
                let mut tab = os_analyze::defs::placeholder_tab(
                    "Segment Targets",
                    vec![os_analyze::FilterDef::placeholder(
                        "All",
                        os_analyze::FilterKind::Live,
                        &[],
                    )],
                );
                tab.population = os_analyze::defs::intern(&scope.population_sql());
                let mut tabs = tabs;
                if canonical == "Links:All Outlinks" {
                    let path = directory.join(file_name(
                        file,
                        &canonical,
                        Some(scope),
                        format.extension(),
                    )?);
                    let mut writer = crate::output::open_writer(&path, format, &canonical, opts)?;
                    writer.header(&crate::links_export::link_header())?;
                    let predicate = format!(
                        "l.source_id IN (SELECT u.id FROM urls u WHERE {})",
                        scope.population_sql()
                    );
                    crate::links_export::stream_links(
                        file,
                        &predicate,
                        &[],
                        entry.order,
                        &crate::links_export::link_kinds(),
                        writer.as_mut(),
                    )?;
                    return Ok(vec![writer.finish()?]);
                } else {
                    entry.selection = bulk_links::Selection::Filters {
                        tab: "Segment Targets".into(),
                        filters: vec!["All".into()],
                    };
                }
                tabs.push(tab);
                let path = directory.join(file_name(
                    file,
                    &canonical,
                    Some(scope),
                    format.extension(),
                )?);
                return Ok(vec![crate::links_export::export_link_entry_with_options(
                    &tabs, file, cfg, &entry, &path, format, &canonical, opts,
                )?]);
            }
        }
    }
    if scope.is_none() {
        return crate::pipeline::export_bulk_in_with_options(
            &tabs, file, cfg, &canonical, directory, format, opts,
        );
    }
    if let Some(scope) = scope {
        let job = crate::catalog::TableJob {
            entry_name: &canonical,
            tabs: &tabs,
            file,
            cfg,
        };
        if let Some(entry) = crate::catalog::find_table(&crate::catalog::bulk_tables(), &canonical)
        {
            if crate::catalog::bulk_generic::canonical_name(&canonical, cfg).is_some()
                && !cfg.export.inlinks_for_all_filters
            {
                return Err(ExportError::Unsupported(
                    "enable export.inlinks_for_all_filters".into(),
                ));
            }
            let header = match entry.header_fn {
                Some(make) => make(&job)?,
                None => entry.header.iter().map(|s| s.to_string()).collect(),
            };
            let mut info = write_scoped_table_with_options(
                &job,
                scope,
                directory,
                format,
                entry.file_stem,
                &header,
                |writer| (entry.run)(&job, writer),
                opts,
            )?;
            info.warnings
                .extend(crate::reports::prereq::requirement_warnings(
                    entry.name,
                    entry.requires,
                    cfg,
                    &[],
                ));
            return Ok(vec![info]);
        }
        if let Some(entry) = crate::catalog::dynamic_entries(cfg)
            .into_iter()
            .find(|e| e.name == canonical)
        {
            let info = write_scoped_table_with_options(
                &job,
                scope,
                directory,
                format,
                &entry.file_stem,
                &entry.header,
                |writer| (entry.run)(&job, writer),
                opts,
            )?;
            return Ok(vec![info]);
        }
    }
    let staging = Staging::new(directory)?;
    let generated = crate::pipeline::export_bulk_in_with_options(
        &tabs, file, cfg, &canonical, &staging.0, format, opts,
    )?;
    publish(file, generated, scope, directory, &staging.0)
}
struct Staging(std::path::PathBuf);
impl Staging {
    fn new(directory: &Path) -> Result<Self, ExportError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let path = directory.join(format!(
            ".segment-export-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).map_err(|source| ExportError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self(path))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn publish(
    file: &CrawlFile,
    mut files: Vec<ExportFileInfo>,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    staging: &Path,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    for info in &mut files {
        let stem = info
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ExportError::Write("invalid export filename".into()))?;
        let ext = info.path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let relative = info
            .path
            .strip_prefix(staging)
            .map_err(|e| ExportError::Write(e.to_string()))?;
        let dest = directory
            .join(relative)
            .with_file_name(file_name(file, stem, scope, ext)?);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ExportError::Io {
                path: parent.into(),
                source,
            })?;
        }
        std::fs::rename(&info.path, &dest).map_err(|source| ExportError::Io {
            path: dest.clone(),
            source,
        })?;
        info.path = dest;
    }
    Ok(files)
}

/// Reports with URL rows retain their complete graph context while filtering the
/// primary URL. Aggregate reports run on a connection-local scoped population.
pub fn export_report(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    export_report_with_options(
        file,
        cfg,
        name,
        scope,
        directory,
        format,
        &crate::pipeline::writer_options(cfg),
    )
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_report_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<ReportResult, ExportError> {
    crate::pipeline::snapshot(file, || {
        export_report_snapshot_with_options(file, cfg, name, scope, directory, format, opts)
    })
}
#[allow(clippy::too_many_arguments)]
fn export_report_snapshot_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    scope: Option<&SegmentFilter>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<ReportResult, ExportError> {
    let Some(scope) = scope else {
        return crate::pipeline::export_report_with_options(
            file, cfg, name, directory, format, opts,
        );
    };
    let crate::catalog::ExportRef::Report(canonical) =
        crate::catalog::resolve_with(ExportKind::Report, name, cfg)?
    else {
        unreachable!("report resolution")
    };
    let tables = crate::reports::report_tables();
    let row_headers = if let Ok(def) = crate::catalog::reports::resolve(&canonical) {
        if def.kind == crate::catalog::reports::ReportKind::CrawlOverview {
            Vec::new()
        } else {
            crate::catalog::CHAIN_HEADER
                .iter()
                .map(|s| s.to_string())
                .collect()
        }
    } else {
        let entry = crate::catalog::find_table(&tables, &canonical)
            .ok_or_else(|| ExportError::Unsupported(canonical.clone()))?;
        let registry = tabs(file, cfg, Some(scope));
        let job = crate::catalog::TableJob {
            entry_name: &canonical,
            tabs: &registry,
            file,
            cfg,
        };
        match entry.header_fn {
            Some(header) => header(&job)?,
            None => entry.header.iter().map(|s| s.to_string()).collect(),
        }
    };
    let aggregate = !row_headers
        .iter()
        .any(|c| matches!(c.as_str(), "Address" | "URL" | "Source" | "Source URL"));
    if aggregate {
        struct Population<'a>(&'a CrawlFile);
        impl Drop for Population<'_> {
            fn drop(&mut self) {
                let _ = self.0.conn().execute_batch("DROP VIEW IF EXISTS temp.urls");
            }
        }
        file.conn()
            .execute_batch(&format!(
                "CREATE TEMP VIEW urls AS SELECT u.* FROM main.urls u WHERE {}",
                scope.population_sql()
            ))
            .map_err(|e| ExportError::Write(e.to_string()))?;
        let _guard = Population(file);
        let staging = Staging::new(directory)?;
        let mut report = crate::pipeline::export_report_in_with_options(
            &tabs(file, cfg, Some(scope)),
            file,
            cfg,
            &canonical,
            &staging.0,
            format,
            opts,
        )?;
        report.files = publish(file, report.files, Some(scope), directory, &staging.0)?;
        return Ok(report);
    }
    let stem = if let Ok(def) = crate::catalog::reports::resolve(&canonical) {
        def.file_stem
    } else {
        crate::catalog::find_table(&crate::reports::report_tables(), &canonical)
            .ok_or_else(|| ExportError::Unsupported(canonical.clone()))?
            .file_stem
    };
    let path = directory.join(file_name(file, stem, Some(scope), format.extension())?);
    let scoped = tabs(file, cfg, Some(scope));
    let job = crate::catalog::TableJob {
        entry_name: &canonical,
        tabs: &scoped,
        file,
        cfg,
    };
    let cancel = os_model::CancelToken::new();
    let mut ctx = crate::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    let writer = crate::output::open_writer(&path, format, &canonical, opts)?;
    let _addresses = AddressScope::new(file, scope)?;
    let mut writer = RowScope {
        writer,
        file,
        index: None,
        rows: 0,
        prefer_target: false,
    };
    crate::reports::save_report_to(&job, &mut writer, &mut ctx)?;
    let rows = writer.rows;
    let mut info = writer.writer.finish()?;
    info.rows = rows;
    Ok(ReportResult {
        files: vec![info],
        warnings: Vec::new(),
    })
}
struct RowScope<'a> {
    writer: Box<dyn crate::types::TableWriter>,
    file: &'a CrawlFile,
    index: Option<usize>,
    rows: u64,
    prefer_target: bool,
}
impl crate::types::TableWriter for RowScope<'_> {
    fn header(&mut self, columns: &[String]) -> Result<(), ExportError> {
        self.index = if self.prefer_target {
            columns.iter().position(|c| {
                matches!(c.as_str(), "Destination" | "Destination URL" | "Target URL")
            })
        } else {
            None
        }
        .or_else(|| {
            columns
                .iter()
                .position(|c| matches!(c.as_str(), "Address" | "URL" | "Source" | "Source URL"))
        });
        self.writer.header(columns)
    }
    fn row(
        &mut self,
        kinds: &[os_analyze::ColumnKind],
        values: &[os_model::Value],
    ) -> Result<(), ExportError> {
        if let Some(i) = self.index {
            let Some(os_model::Value::Text(address)) = values.get(i) else {
                return Ok(());
            };
            let selected:bool=self.file.conn().query_row("SELECT EXISTS(SELECT 1 FROM temp.os_segment_report_addresses WHERE address=?1)",[address],|r|r.get(0)).map_err(|e|ExportError::Write(e.to_string()))?;
            if !selected {
                return Ok(());
            }
        }
        self.writer.row(kinds, values)?;
        self.rows += 1;
        Ok(())
    }
    fn finish(self: Box<Self>) -> Result<ExportFileInfo, ExportError> {
        self.writer.finish()
    }
}

/// Shared entry point for CLI/scheduling: `all` includes the unfiltered export.
pub fn export_all(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selection: &crate::types::ExportSelection,
    segment: Option<&str>,
    directory: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    export_all_with_options(
        file,
        cfg,
        selection,
        segment,
        directory,
        format,
        &crate::pipeline::writer_options(cfg),
    )
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_all_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    selection: &crate::types::ExportSelection,
    segment: Option<&str>,
    directory: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    crate::pipeline::snapshot(file, || {
        let mut files = Vec::new();
        for scope in selections(file, segment)? {
            match selection.kind.as_str() {
                "tab" | "view" => {
                    let (tab, filter) = os_analyze::names::resolve_in(
                        &os_analyze::tabs::configured_for_file(cfg, file),
                        &selection.name,
                    )?;
                    files.push(export_view_with_options(
                        file,
                        cfg,
                        &ViewQuery::new(tab, filter),
                        scope.as_ref(),
                        directory,
                        format,
                        opts,
                    )?);
                }
                "bulk" => files.extend(export_bulk_with_options(
                    file,
                    cfg,
                    &selection.name,
                    scope.as_ref(),
                    directory,
                    format,
                    opts,
                )?),
                "report" | "reports" => files.extend(
                    export_report_with_options(
                        file,
                        cfg,
                        &selection.name,
                        scope.as_ref(),
                        directory,
                        format,
                        opts,
                    )?
                    .files,
                ),
                other => return Err(ExportError::Unsupported(other.into())),
            }
        }
        Ok(files)
    })
}

struct AddressScope<'a>(&'a CrawlFile);
impl<'a> AddressScope<'a> {
    fn new(file: &'a CrawlFile, scope: &SegmentFilter) -> Result<Self, ExportError> {
        file.conn().execute_batch("CREATE TEMP TABLE os_segment_report_addresses(address TEXT PRIMARY KEY) WITHOUT ROWID").map_err(|e|ExportError::Write(e.to_string()))?;
        let guard = Self(file);
        file.conn().execute_batch(&format!("INSERT OR IGNORE INTO temp.os_segment_report_addresses SELECT u.address FROM main.urls u WHERE {}",scope.population_sql())).map_err(|e|ExportError::Write(e.to_string()))?;
        Ok(guard)
    }
}
impl Drop for AddressScope<'_> {
    fn drop(&mut self) {
        let _ = self
            .0
            .conn()
            .execute_batch("DROP TABLE IF EXISTS temp.os_segment_report_addresses");
    }
}
#[allow(clippy::too_many_arguments)]
fn write_scoped_table_with_options(
    job: &crate::catalog::TableJob<'_>,
    scope: &SegmentFilter,
    directory: &Path,
    format: Format,
    stem: &str,
    header: &[String],
    write: impl FnOnce(&mut dyn crate::types::TableWriter) -> Result<u64, ExportError>,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let _addresses = AddressScope::new(job.file, scope)?;
    let path = directory.join(file_name(job.file, stem, Some(scope), format.extension())?);
    let writer = crate::output::open_writer(&path, format, stem, opts)?;
    let mut writer = RowScope {
        writer,
        file: job.file,
        index: None,
        rows: 0,
        prefer_target: job.entry_name.to_ascii_lowercase().ends_with("inlinks"),
    };
    crate::types::TableWriter::header(&mut writer, header)?;
    write(&mut writer)?;
    let rows = writer.rows;
    let mut info = writer.writer.finish()?;
    info.rows = rows;
    Ok(info)
}
