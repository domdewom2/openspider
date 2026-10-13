//! Reports: Crawl Overview (OUT-080) und Redirects (OUT-081, OUT-082), Voraussetzungen (OUT-112).
//!
//! Einstieg ist [`save_report`] (`--save-report "Crawl Overview,Redirects:Redirect Chains"`). Namen, Dateinamen
//! und Voraussetzungen stehen im Register `catalog::reports`. Jeder Report schreibt über die gemeinsamen
//! Tabellenschreiber (CSV/XLSX); fehlende Voraussetzungen führen zu Warnungen in [`ReportResult::warnings`],
//! nie zum Abbruch (OUT-112).

pub(crate) mod common;
pub mod overview;
pub mod prereq;
pub mod redirects;

use std::path::Path;

use os_analyze::{tabs, TabDef};
use os_model::CrawlConfig;
use os_store::CrawlFile;

use crate::catalog::reports::{self, ReportKind};
use crate::error::ExportError;
use crate::output::open_writer;
use crate::types::{Format, ReportResult};

use self::common::writer_options;
use self::redirects::RedirectReport;

/// Speichert einen Report aus dem Katalog in den Ausgabeordner.
///
/// `name` ist der kanonische Name (`"Crawl Overview"`, `"Redirects:All Redirects"`,
/// `"Redirects:Redirect Chains"`; Groß-/Kleinschreibung und Leerräume um `:` spielen keine Rolle). Der Report
/// rechnet gegen die Tab-Definitionen von `os_analyze::tabs::all()`.
pub fn save_report(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    save_report_in(&tabs::all(), file, cfg, name, out_dir, format)
}

/// Wie [`save_report`], gegen eine eigene Tab-Liste (Tests, abweichende Registry).
pub fn save_report_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    name: &str,
    out_dir: &Path,
    format: Format,
) -> Result<ReportResult, ExportError> {
    let def = reports::resolve(name)?;
    let path = out_dir.join(def.file_name(format.extension()));
    let mut result = ReportResult::default();
    let mut pending: Vec<String> = Vec::new();
    let info = match def.kind {
        ReportKind::CrawlOverview => {
            let ov = overview::build(file, cfg, tabs)?;
            pending = ov.pending_analysis.clone();
            let mut writer = open_writer(&path, format, def.file_stem, &writer_options(cfg))?;
            let header: Vec<String> = overview::HEADER.iter().map(|h| (*h).to_string()).collect();
            writer.header(&header)?;
            for row in &ov.rows {
                let (kinds, values) = row.to_values();
                writer.row(&kinds, &values)?;
            }
            writer.finish()?
        }
        ReportKind::AllRedirects => {
            redirects::write_report(file, cfg, RedirectReport::All, &path, def.file_stem, format)?
        }
        ReportKind::RedirectChains => redirects::write_report(
            file,
            cfg,
            RedirectReport::Chains,
            &path,
            def.file_stem,
            format,
        )?,
    };
    result.warnings = prereq::requirement_warnings(def.name, def.requires, cfg, &pending);
    for w in &result.warnings {
        tracing::warn!("{w}");
    }
    result.files.push(info);
    Ok(result)
}
