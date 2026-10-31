//! Issues-Export: Ordner `issues/` mit einer Datei je Issue und `issues_overview` (OUT-063, OUT-025).
//!
//! Kanonischer Bulk-Name: [`BULK_NAME`] (`--bulk-export "Issues:All Issues"`). Der Export schreibt unter
//! `<Zielordner>/issues/`
//!
//! * je Zeile des Issues-Panels eine Datei `<issue_type>_<tab>_<filter>.<ext>` (z. B.
//!   `issue_response_codes_internal_client_error_4xx.csv`) mit den betroffenen URLs in den Standardspalten des
//!   Tabs, bei linkbezogenen Issues (Tab „Response Codes“) zusätzlich `…_inlinks.<ext>` mit den Inlinks der
//!   Treffer im Standard-Linkformat (OUT-031, über `export_detail`),
//! * `issues_overview.<ext>` mit den Spalten `Issue Name`, `Issue Type`, `Issue Priority`, `URLs`, `% of
//!   Total`, `Description`, `How To Fix` und – **[OS-Erweiterung]**, hinter den Referenzspalten – `Why It
//!   Matters`.
//!
//! Die Zeilen sind genau die des Issues-Panels (`issues::issues_from_counts`: nur Filter im Zustand „bereit“
//! mit Treffern). `issues.ignore_non_indexable` wirkt, weil Zähler und Dateien aus denselben Plänen stammen
//! (CRAWL-089). `Description` = `what`, `How To Fix` = `fix`, `Why It Matters` = `why` aus
//! `issues::texts` (Englisch, kanonische Exportsprache) mit den Schwellen der Crawl-Konfiguration.

use std::path::Path;

use os_analyze::issues::{self, CatalogEntry, IssueRow, Lang};
use os_analyze::views::{filter_counts_in, query_view_in, stream_view_in, PageStart, ViewQuery};
use os_analyze::{tabs, ColumnKind, NameError, TabDef, ViewError};
use os_model::{CrawlConfig, UrlId, Value};
use os_store::{CrawlFile, StoreError};

use crate::catalog::reports::ReportRequirement;
use crate::error::ExportError;
use crate::fs_paths::NameRegistry;
use crate::naming;
use crate::output::open_writer;
use crate::reports::common::{pct2, writer_options};
use crate::reports::prereq::{pending_analysis_filters, requirement_warnings};
use crate::types::{DetailKind, ExportFileInfo, Format};

/// Kanonischer Name des Bulk-Exports.
pub const BULK_NAME: &str = "Issues:All Issues";

/// Name des Unterordners im Zielordner.
pub const DIR_NAME: &str = "issues";

/// Dateiname-Stamm der Übersicht.
pub const OVERVIEW_STEM: &str = "issues_overview";

/// Voraussetzungen des Exports (OUT-112): Nachlauf-Filter brauchen die Crawl-Analyse.
pub const REQUIRES: &[ReportRequirement] = &[ReportRequirement::CrawlAnalysis];

/// Kopfzeile von `issues_overview` (OUT-025).
pub const OVERVIEW_HEADER: [&str; 8] = [
    "Issue Name",
    "Issue Type",
    "Issue Priority",
    "URLs",
    "% of Total",
    "Description",
    "How To Fix",
    "Why It Matters",
];

/// Ergebnis des Exports.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IssuesExport {
    /// Geschriebene Dateien: zuerst die Issue-Dateien, zuletzt `issues_overview`.
    pub files: Vec<ExportFileInfo>,
    /// Hinweise (fehlende Voraussetzungen nach OUT-112, fehlende Texte).
    pub warnings: Vec<String>,
}

/// Alle kanonischen Namen dieses Katalogs.
pub fn names() -> Vec<String> {
    vec![BULK_NAME.to_string()]
}

/// Löst einen Namen auf (case-insensitiv, Leerräume um `:` tolerant).
pub fn resolve(name: &str) -> Result<&'static str, NameError> {
    let norm = |s: &str| {
        s.split(':')
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(":")
            .to_lowercase()
    };
    if norm(name) == norm(BULK_NAME) {
        return Ok(BULK_NAME);
    }
    let all = names();
    Err(NameError::Unknown {
        input: name.to_string(),
        suggestions: os_analyze::names::column_suggestions(all.iter().map(String::as_str), name),
    })
}

/// Zeigt der Issue-Eintrag auf Links (dann gibt es zusätzlich die Inlinks-Variante)? In M1 sind das die
/// Issues des Tabs „Response Codes“ (Gegenstück zu den Inlinks-Exporten von OUT-050).
pub fn is_link_related(entry: &CatalogEntry) -> bool {
    entry.tab == "Response Codes"
}

/// Dateiname einer Issue-Datei (OUT-063): `<issue_type>_<tab>_<filter>`, nach OUT-008 bereinigt.
pub fn issue_file_name(row: &IssueRow, filter_label: &str, ext: &str) -> String {
    naming::file_name(
        &format!("{}:{}:{}", row.issue_type.as_str(), row.tab, filter_label),
        None,
        ext,
    )
}

/// Dateiname der Inlinks-Variante.
pub fn inlinks_file_name(row: &IssueRow, filter_label: &str, ext: &str) -> String {
    naming::file_name(
        &format!(
            "{}:{}:{}:Inlinks",
            row.issue_type.as_str(),
            row.tab,
            filter_label
        ),
        None,
        ext,
    )
}

/// Führt den Export aus (Tabs aus `os_analyze::tabs::all()`).
pub fn export(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    format: Format,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    export_with_options(file, cfg, out_dir, format, &writer_options(cfg))
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_with_options(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<Vec<ExportFileInfo>, ExportError> {
    Ok(export_in_with_options(
        &tabs::configured_for_file(cfg, file),
        file,
        cfg,
        out_dir,
        format,
        opts,
    )?
    .files)
}

fn abort_marker() -> ViewError {
    ViewError::Store(StoreError::InvalidInput("Export abgebrochen".to_string()))
}

/// Kinds der Ausgabespalten: aus der Tab-Definition, Mehrfachspalten (`Name 1…n`) wie ihre Grundspalte.
fn kinds_for(tab: &TabDef, columns: &[String]) -> Vec<ColumnKind> {
    columns
        .iter()
        .map(|name| {
            tab.columns
                .iter()
                .find(|c| c.key == name)
                .or_else(|| {
                    tab.columns.iter().find(|c| match &c.sql {
                        os_analyze::ColumnSql::Multi { base, .. }
                        | os_analyze::ColumnSql::MultiCrawlWide { base, .. } => name
                            .strip_prefix(*base)
                            .is_some_and(|rest| rest.trim().parse::<u32>().is_ok()),
                        // Verschränkte Teilspalten: `<Basis> <n>` bzw. `<Basis> <n> <Suffix>` (nur Übersetzbarkeit;
                        // die Art je Teilspalte regelt M1-N10).
                        os_analyze::ColumnSql::MultiPaired { base, .. } => {
                            name.strip_prefix(*base).is_some_and(|rest| {
                                rest.split_whitespace()
                                    .next()
                                    .is_some_and(|n| n.parse::<u32>().is_ok())
                            })
                        }
                        os_analyze::ColumnSql::Expr(_) => false,
                    })
                })
                .map_or(ColumnKind::Text, |c| c.kind)
        })
        .collect()
}

/// Werte der Textplatzhalter eines Issues: erst die Werte der Crawl-Konfiguration unter den Pfaden aus
/// `CatalogEntry::text_params`, dahinter die Parameter des Tabs. So stimmen die Schwellen in den Texten
/// unabhängig davon, wie ein Tab seine SQL-Parameter benennt.
fn text_params(
    entry: &CatalogEntry,
    tab: &TabDef,
    cfg: &CrawlConfig,
    cfg_json: &serde_json::Value,
) -> Vec<(String, Value)> {
    let from_config = entry.text_params.iter().filter_map(|(_, path)| {
        let value = match os_analyze::names::config_value(cfg_json, path)? {
            serde_json::Value::Number(n) => n
                .as_i64()
                .map(Value::Int)
                .or_else(|| n.as_f64().map(Value::Float))?,
            serde_json::Value::String(s) => Value::Text(s.clone()),
            serde_json::Value::Bool(b) => Value::Bool(*b),
            _ => return None,
        };
        Some((path.clone(), value))
    });
    let from_tab = (tab.params)(cfg)
        .into_iter()
        .map(|(name, value)| (name.to_string(), value));
    from_config.chain(from_tab).collect()
}

/// Gemeinsamer Kontext der Dateischreiber.
struct Ctx<'a> {
    tabs: &'a [TabDef],
    file: &'a CrawlFile,
    cfg: &'a CrawlConfig,
    format: Format,
}

/// Schreibt die Ansicht eines Filters in eine Datei.
#[allow(clippy::too_many_arguments)]
fn write_view_with_options(
    cx: &Ctx<'_>,
    tab: &TabDef,
    filter: &str,
    path: &Path,
    sheet: &str,
    opts: &crate::types::WriterOptions,
) -> Result<ExportFileInfo, ExportError> {
    let Ctx {
        tabs,
        file,
        cfg,
        format,
    } = *cx;
    let query = ViewQuery::new(tab.id, filter);
    // Spaltennamen vorab über eine Seite mit einer Zeile; die Zeilen kommen dann gestreamt.
    let mut probe = query.clone();
    probe.limit = 1;
    let columns = query_view_in(tabs, file, cfg, &probe)?.columns;
    let kinds = kinds_for(tab, &columns);
    let mut writer = open_writer(path, format, sheet, opts)?;
    writer.header(&columns)?;
    let mut failure: Option<ExportError> = None;
    let streamed = stream_view_in(tabs, file, cfg, &query, &mut |values| {
        writer.row(&kinds, values).map_err(|e| {
            failure = Some(e);
            abort_marker()
        })
    });
    if let Some(e) = failure {
        return Err(e);
    }
    streamed?;
    writer.finish()
}

/// Kennungen aller URLs im Filter (seitenweise gelesen).
fn filter_url_ids(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
) -> Result<Vec<UrlId>, ExportError> {
    let mut ids = Vec::new();
    let mut offset = 0u64;
    loop {
        let mut q = ViewQuery::new(tab, filter);
        q.limit = 500;
        q.start = PageStart::Offset(offset);
        let page = query_view_in(tabs, file, cfg, &q)?;
        if page.row_keys.is_empty() {
            break;
        }
        offset += page.row_keys.len() as u64;
        ids.extend(page.row_keys);
        if offset >= page.total {
            break;
        }
    }
    Ok(ids)
}

/// Wie [`export`], gegen eine eigene Tab-Liste (Tests, abweichende Registry).
pub fn export_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    format: Format,
) -> Result<IssuesExport, ExportError> {
    export_in_with_options(tabs, file, cfg, out_dir, format, &writer_options(cfg))
}

/// Exports with explicit writer options, including presentation header labels.
#[allow(clippy::too_many_arguments)]
pub fn export_in_with_options(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    out_dir: &Path,
    format: Format,
    opts: &crate::types::WriterOptions,
) -> Result<IssuesExport, ExportError> {
    let counts = filter_counts_in(tabs, file, cfg)?;
    let rows = issues::issues_from_counts(&counts);
    let dir = out_dir.join(DIR_NAME);
    let ext = format.extension();
    let mut out = IssuesExport::default();
    let cx = Ctx {
        tabs,
        file,
        cfg,
        format,
    };
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let mut names = NameRegistry::new();
    let mut overview: Vec<[Value; 8]> = Vec::with_capacity(rows.len());

    for row in &rows {
        let Some(entry) = issues::catalog().iter().find(|e| e.id == row.id) else {
            continue;
        };
        let Some(tab) = tabs.iter().find(|t| t.id == row.tab) else {
            continue;
        };
        let label = counts
            .iter()
            .find(|c| c.tab == row.tab && c.filter == row.filter)
            .map_or_else(|| row.filter.clone(), |c| c.label.clone());

        // Datei mit den betroffenen URLs.
        let disc = format!("{}:{}", row.tab, row.filter);
        let name = names.claim(&issue_file_name(row, &label, ext), &disc);
        let info = write_view_with_options(&cx, tab, &row.filter, &dir.join(&name), &label, opts)?;
        out.files.push(info);

        // Inlinks-Variante bei linkbezogenen Issues.
        if is_link_related(entry) {
            let ids = filter_url_ids(tabs, file, cfg, &row.tab, &row.filter)?;
            let in_name = names.claim(&inlinks_file_name(row, &label, ext), &format!("{disc}:in"));
            // `export_detail` nimmt den Pfad der Zieldatei (nicht eines Ordners).
            let info = crate::detail::export_detail_with_options(
                file,
                DetailKind::Inlinks,
                &ids,
                &dir.join(&in_name),
                format,
                opts,
            )?;
            out.files.push(info);
        }

        // Zeile der Übersicht.
        let params = text_params(entry, tab, cfg, &cfg_json);
        let texts = issues::texts(
            &entry.doc_key,
            Lang::En,
            &params
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>(),
        );
        if texts.is_none() {
            out.warnings
                .push(format!("issue '{}' has no description text", entry.doc_key));
        }
        let texts = texts.unwrap_or(issues::IssueTexts {
            what: String::new(),
            why: String::new(),
            fix: String::new(),
        });
        let base = counts
            .iter()
            .find(|c| c.tab == row.tab && c.filter == "All")
            .and_then(|c| c.count);
        let pct = base.and_then(|b| pct2(row.count, b));
        overview.push([
            Value::Text(format!("{}: {}", row.tab, label)),
            Value::Text(row.issue_type.as_str().to_string()),
            Value::Text(row.priority.as_str().to_string()),
            Value::Int(i64::try_from(row.count).unwrap_or(i64::MAX)),
            pct.map_or(Value::Null, Value::Float),
            Value::Text(texts.what),
            Value::Text(texts.fix),
            Value::Text(texts.why),
        ]);
    }

    // Übersicht.
    let overview_name = names.claim(&format!("{OVERVIEW_STEM}.{ext}"), OVERVIEW_STEM);
    let mut writer = open_writer(&dir.join(&overview_name), format, "Issues Overview", opts)?;
    let header: Vec<String> = OVERVIEW_HEADER.iter().map(|h| (*h).to_string()).collect();
    writer.header(&header)?;
    let kinds = [
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
    ];
    for values in &overview {
        writer.row(&kinds, values)?;
    }
    let mut info = writer.finish()?;
    let pending = pending_analysis_filters(&counts);
    let prereq = requirement_warnings(BULK_NAME, REQUIRES, cfg, &pending);
    for w in &prereq {
        tracing::warn!("{w}");
    }
    info.warnings.extend(prereq.iter().cloned());
    out.warnings.splice(0..0, prereq);
    out.files.push(info);
    Ok(out)
}

/// Issues-Übersicht in einem vorhandenen Writer (OUT-063/075).
pub fn export_to(
    job: &crate::catalog::TableJob<'_>,
    writer: &mut dyn crate::types::TableWriter,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<u64, ExportError> {
    ctx.checkpoint()?;
    let counts = filter_counts_in(job.tabs, job.file, job.cfg)?;
    let rows = issues::issues_from_counts(&counts);
    let cfg_json = serde_json::to_value(job.cfg).unwrap_or_default();
    writer.header(
        &OVERVIEW_HEADER
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
    )?;
    let kinds = [
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
    ];
    let mut written = 0;
    for row in rows {
        ctx.checkpoint()?;
        let Some(entry) = issues::catalog().iter().find(|e| e.id == row.id) else {
            continue;
        };
        let Some(tab) = job.tabs.iter().find(|t| t.id == row.tab) else {
            continue;
        };
        let params = text_params(entry, tab, job.cfg, &cfg_json);
        let texts = issues::texts(
            &entry.doc_key,
            Lang::En,
            &params
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>(),
        );
        let texts = texts.unwrap_or(issues::IssueTexts {
            what: String::new(),
            why: String::new(),
            fix: String::new(),
        });
        let label = counts
            .iter()
            .find(|c| c.tab == row.tab && c.filter == row.filter)
            .map_or(&row.filter, |c| &c.label);
        let base = counts
            .iter()
            .find(|c| c.tab == row.tab && c.filter == "All")
            .and_then(|c| c.count);
        writer.row(
            &kinds,
            &[
                Value::Text(format!("{}: {}", row.tab, label)),
                Value::Text(row.issue_type.as_str().into()),
                Value::Text(row.priority.as_str().into()),
                Value::Int(i64::try_from(row.count).unwrap_or(i64::MAX)),
                base.and_then(|b| pct2(row.count, b))
                    .map_or(Value::Null, Value::Float),
                Value::Text(texts.what),
                Value::Text(texts.fix),
                Value::Text(texts.why),
            ],
        )?;
        written += 1;
        (ctx.progress)(written);
    }
    if written == 0 && ctx.skip_empty {
        ctx.skipped.push(job.entry_name.into());
    }
    Ok(written)
}
