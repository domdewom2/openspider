//! Tab-Export: eine Ansicht (Tab und Filter) als CSV/XLSX (OUT-020, ONPAGE-191).
//!
//! * Spalten: ohne Angabe die sichtbaren Standardspalten (`ColumnSelection::Default`), mit
//!   `ColumnSelection::All` alle Spalten des Tabs („Alle Spalten exportieren“); Spalten mit
//!   `ColumnDef.extension` fehlen im Standardexport und stehen bei „alle Spalten“ hinter den
//!   Referenzspalten – generisch über das Kennzeichen, ohne Sonderlogik je Tab.
//! * Zeilen: Filter, Suche (`ViewQuery.search`) und Sortierung (`ViewQuery.sort`) kommen ausschließlich aus
//!   `views::stream_view`; dieses Modul hat keine eigene `ORDER BY`-Logik.
//! * Dateiname (CLI, Scheduling, Multi-Export): kanonischer Name `Tab:Filter` nach OUT-008 ohne Datum,
//!   z. B. `response_codes_client_error_4xx.csv`. Den Vorschlag der Oberfläche mit Crawl-Startdatum liefert
//!   [`suggest_file_name`].

use std::path::Path;

use os_analyze::defs::TabDef;
use os_analyze::views::ViewQuery;
use os_analyze::{names, tabs, NameError};
use os_model::{CrawlConfig, Timestamp};
use os_store::CrawlFile;
use time::UtcOffset;

use crate::error::ExportError;
use crate::fs_paths;
use crate::naming;

use crate::types::{ExportFileInfo, Format};

/// Exportiert die Ansicht in den Ausgabeordner (Dateiname nach OUT-008, ohne Datum).
pub fn export_tab(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    out_dir: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    crate::pipeline::snapshot(file, || {
        export_tab_in(
            &tabs::configured_for_file(cfg, file),
            file,
            cfg,
            q,
            out_dir,
            format,
        )
    })
}

/// Single-tab entry point for app/CLI callers with cancellation and upload-order control.
/// `None` means an empty export was skipped or the table went into a shared workbook.
pub fn export_tab_with(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    out_dir: &Path,
    format: Format,
    preserve_upload_order: bool,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<Option<ExportFileInfo>, ExportError> {
    crate::pipeline::snapshot(file, || {
        let tabs = tabs::configured_for_file(cfg, file);
        let canonical = canonical_name(&tabs, cfg, &q.tab, &q.filter)?;
        let path = out_dir.join(naming::file_name(&canonical, None, format.extension()));
        crate::pipeline::export_view_with(
            &tabs,
            file,
            cfg,
            q,
            &path,
            format,
            &canonical.replace(':', " "),
            preserve_upload_order,
            ctx,
        )
    })
}

/// Wie [`export_tab`], gegen eine eigene Tab-Liste.
pub fn export_tab_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    out_dir: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let canonical = canonical_name(tabs, cfg, &q.tab, &q.filter)?;
    fs_paths::create_dir_all(out_dir).map_err(|source| ExportError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;
    let path = out_dir.join(naming::file_name(&canonical, None, format.extension()));
    let sheet = canonical.replacen(':', " ", 1);
    // Existing CLI callers retain ordinary view order; L01/K06 opt into the
    // new upload-order contract through `export_tab_with` / `export_view_with`.
    let cancel = os_model::CancelToken::new();
    let mut ctx = crate::types::ExportCtx {
        cancel: &cancel,
        progress: &|_| {},
        skip_empty: false,
        workbook: None,
        skipped: Vec::new(),
    };
    crate::pipeline::export_view_with(tabs, file, cfg, q, &path, format, &sheet, false, &mut ctx)?
        .ok_or_else(|| ExportError::Write("Missing tab output".into()))
}

/// Kanonischer Exportname `Tab:Filter` mit aufgelösten Schwellen (`Page Titles:Over 60 Characters`).
/// Tab und Filter dürfen in beliebiger Groß-/Kleinschreibung bzw. mit `X` oder Zahl als Schwelle stehen.
pub fn canonical_name(
    tabs: &[TabDef],
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
) -> Result<String, ExportError> {
    let unknown = || {
        ExportError::Name(NameError::Unknown {
            input: format!("{tab}:{filter}"),
            suggestions: names::suggestions(tabs, &format!("{tab}:{filter}")),
        })
    };
    let t = names::find_tab(tabs, tab).ok_or_else(unknown)?;
    let f = names::find_filter(t, filter).ok_or_else(unknown)?;
    Ok(format!("{}:{}", t.id, names::label(f.id, cfg)))
}

/// Dateinamensvorschlag der Oberfläche (OUT-020): `<tab>_<filter>_<YYYY-MM-DD>` aus dem Basisnamen nach
/// OUT-008 plus Datum.
///
/// Das Datum ist der **Start des Crawls in lokaler Zeit** (nicht das Exportdatum); `offset` ist der lokale
/// UTC-Offset (`None` = Systemzeitzone). Ein Segmentsuffix steht vor dem Datum. Ohne Startzeit (Crawl nie
/// gestartet) entfällt das Datum.
pub fn suggest_file_name(
    canonical: &str,
    segment: Option<&str>,
    crawl_started: Option<Timestamp>,
    offset: Option<UtcOffset>,
    format: Format,
) -> String {
    let base = naming::file_name(canonical, segment, "");
    let ext = format.extension();
    match crawl_started {
        Some(ts) => {
            let stamp = match offset {
                Some(o) => ts.to_export_with(o),
                None => ts.to_export(),
            };
            let date: String = stamp.chars().take(10).collect();
            format!("{base}_{date}.{ext}")
        }
        None => format!("{base}.{ext}"),
    }
}

/// Dateinamensvorschlag zu einer Ansicht der Crawl-Datei (Startdatum aus `crawl_meta.started_at`).
pub fn suggest_tab_file_name(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
    segment: Option<&str>,
    offset: Option<UtcOffset>,
    format: Format,
) -> Result<String, ExportError> {
    suggest_tab_file_name_in(
        &tabs::configured_for_file(cfg, file),
        file,
        cfg,
        tab,
        filter,
        segment,
        offset,
        format,
    )
}

/// Wie [`suggest_tab_file_name`], gegen eine eigene Tab-Liste.
#[allow(clippy::too_many_arguments)]
pub fn suggest_tab_file_name_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
    segment: Option<&str>,
    offset: Option<UtcOffset>,
    format: Format,
) -> Result<String, ExportError> {
    let canonical = canonical_name(tabs, cfg, tab, filter)?;
    let started = file.meta()?.started_at;
    Ok(suggest_file_name(
        &canonical, segment, started, offset, format,
    ))
}
