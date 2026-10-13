//! Detail-Export (Inlinks/Outlinks) für eine URL-Liste (OUT-021, OUT-022, ONPAGE-192).
//!
//! Das ist der Datenteil der Lower-Tabs „Inlinks“ und „Outlinks“ einer oder mehrerer in der Haupttabelle
//! gewählter URLs: alle Links im Standard-Linkformat (15 Spalten, siehe [`crate::links_export`]) in einer
//! Datei. Die Reihenfolge folgt der übergebenen URL-Liste (doppelte Einträge zählen einmal); innerhalb einer
//! URL stehen Inlinks nach `Source` und Dokumentreihenfolge, Outlinks in Dokumentreihenfolge. Die
//! Verarbeitung streamt (auch bei 50 000 URLs).

use std::collections::HashSet;
use std::path::Path;

use os_model::{UrlId, Value};
use os_store::CrawlFile;

use crate::error::ExportError;
use crate::links_export::{link_header, link_kinds, stream_links, LinkOrder};
use crate::output::open_writer;
use crate::pipeline::writer_options;
use crate::types::{DetailKind, ExportFileInfo, Format};

/// Exportiert Inlinks bzw. Outlinks der angegebenen URLs im Standard-Linkformat (ONPAGE-192).
///
/// `out` ist der Pfad der Zieldatei. Schreiboptionen (BOM, Zeilenende) stammen aus der in der Crawl-Datei
/// gespeicherten Konfiguration. Die Datei hat immer eine Kopfzeile, auch ohne Links.
pub fn export_detail(
    file: &CrawlFile,
    detail: DetailKind,
    urls: &[UrlId],
    out: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let cfg = file.config()?;
    let (sheet, where_sql) = match detail {
        DetailKind::Inlinks => ("Inlinks", "l.target_id = :url_id"),
        DetailKind::Outlinks => ("Outlinks", "l.source_id = :url_id"),
    };
    let kinds = link_kinds();
    let mut writer = open_writer(out, format, sheet, &writer_options(&cfg))?;
    writer.header(&link_header())?;
    let mut seen: HashSet<i64> = HashSet::new();
    for id in urls {
        if !seen.insert(id.0) {
            continue;
        }
        let params = [(":url_id".to_string(), Value::Int(id.0))];
        stream_links(
            file,
            where_sql,
            &params,
            LinkOrder::Outlinks,
            &kinds,
            &mut *writer,
        )?;
    }
    writer.finish()
}
