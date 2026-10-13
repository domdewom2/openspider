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
use crate::output::open_writer;
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
    write_view(tabs, file, cfg, q, || {
        open_writer(path, format, sheet, &writer_options(cfg))
    })
}

/// Wie [`export_view`], aber mit frei wählbarem Schreiber: `open` wird erst aufgerufen, wenn Tab und Filter
/// aufgelöst sind. Ein Fehler des Schreibers (auch mitten im Zeilenstrom) bricht den Export ab und kommt
/// unverändert beim Aufrufer an. Dient auch Tests mit einem Schreiber, der gezielt scheitert.
pub fn write_view(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    open: impl FnOnce() -> Result<Box<dyn TableWriter>, ExportError>,
) -> Result<ExportFileInfo, ExportError> {
    // Kopfzeile vorab: `stream_view` meldet die Spaltennamen erst nach dem letzten Datensatz.
    let mut probe = q.clone();
    probe.limit = 1;
    probe.start = PageStart::Offset(0);
    let header = views::query_view_in(tabs, file, cfg, &probe)?.columns;
    let tab = names::find_tab(tabs, &q.tab).ok_or_else(|| {
        ExportError::Name(os_analyze::NameError::Unknown {
            input: q.tab.clone(),
            suggestions: names::suggestions(tabs, &format!("{}:{}", q.tab, q.filter)),
        })
    })?;
    let kinds: Vec<ColumnKind> = header.iter().map(|n| column_kind(tab, n)).collect();
    let state = views::state_of_in(tabs, file, cfg, tab.id, &q.filter)?;

    let mut writer = open()?;
    writer.header(&header)?;

    let mut failure: Option<ExportError> = None;
    let width = header.len();
    let mut padded: Vec<Value> = Vec::new();
    let streamed = views::stream_view_in(tabs, file, cfg, q, &mut |values: &[Value]| {
        // Läuft der Crawl noch, kann eine Mehrfachspalte zwischen Kopfzeile und Daten wachsen: auf die
        // Breite der Kopfzeile bringen statt eine schiefe Datei zu schreiben.
        let row: &[Value] = if values.len() == width {
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

    let mut info = writer.finish()?;
    if state != FilterState::Ready {
        let w = filter_warning(tab.id, &q.filter, &state);
        tracing::warn!("{w}");
        info.warnings.push(w);
    }
    Ok(info)
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
    export_bulk_in(&os_analyze::tabs::all(), file, cfg, name, out_dir, format)
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
    // Auflösung über den gemeinsamen Katalog: Vorschläge bei Tippfehlern stammen aus allen Bulk-Namen.
    let canonical = match catalog::resolve(ExportKind::Bulk, name)? {
        ExportRef::Bulk(canonical) => canonical,
        other => unreachable!("Bulk-Auflösung lieferte {other:?}"),
    };
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
    export_report_in(&os_analyze::tabs::all(), file, cfg, name, out_dir, format)
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
    fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;
    crate::reports::save_report_in(tabs, file, cfg, name, out_dir, format)
}
