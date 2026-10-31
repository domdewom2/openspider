//! Offline SERP uploads (CRAWL-031, OUT-098). No network or HTML interpretation.
use std::path::Path;

use crate::error::EngineError;

/// One uploaded draft, in input order. Text is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerpRow {
    pub url: String,
    pub title: String,
    pub description: String,
}

/// Reads delimited TXT/CSV or an XLS/XLSX workbook (first sheet).
pub fn read(path: &Path) -> Result<Vec<SerpRow>, EngineError> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "xls" | "xlsx") {
        use calamine::Reader;
        let mut book = calamine::open_workbook_auto(path)
            .map_err(|e| EngineError::Invalid(format!("SERP: {e}")))?;
        let (_, sheet) = book
            .worksheets()
            .into_iter()
            .next()
            .ok_or_else(|| EngineError::Invalid("SERP: Kein Tabellenblatt".into()))?;
        return from_rows(
            sheet
                .rows()
                .map(|row| row.iter().map(ToString::to_string).collect())
                .collect(),
        );
    }
    let bytes = std::fs::read(path)?;
    parse(&crate::list_input::decode_text(&bytes))
}

/// Parses CSV, semicolon-separated CSV or tab-delimited text, including quoted newlines.
pub fn parse(text: &str) -> Result<Vec<SerpRow>, EngineError> {
    let text = text.trim_start_matches('\u{feff}');
    let delimiter = b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| {
            text.lines()
                .next()
                .unwrap_or_default()
                .bytes()
                .filter(|b| b == d)
                .count()
        })
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let rows = reader
        .records()
        .map(|r| r.map(|r| r.iter().map(str::to_string).collect()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| EngineError::Invalid(format!("SERP: {e}")))?;
    from_rows(rows)
}

fn from_rows(rows: Vec<Vec<String>>) -> Result<Vec<SerpRow>, EngineError> {
    let header = rows.first().cloned().unwrap_or_default();
    let column = |name: &str, aliases: &[&str]| {
        header
            .iter()
            .position(|h| aliases.iter().any(|a| h.trim().eq_ignore_ascii_case(a)))
            .ok_or_else(|| EngineError::Invalid(format!("SERP: Pflichtspalte {name} fehlt")))
    };
    let url = column("URL", &["URL", "Address"])?;
    let title = column("Title / Titel", &["Title", "Titel", "Title 1"])?;
    let description = column(
        "Description / Beschreibung",
        &["Description", "Beschreibung", "Meta Description 1"],
    )?;
    Ok(rows
        .into_iter()
        .skip(1)
        .enumerate()
        .map(|(i, row)| {
            let get = |n| row.get(n).cloned().unwrap_or_default();
            let address: String = get(url);
            SerpRow {
                url: if address.trim().is_empty() {
                    format!("Zeile {}", i + 2)
                } else {
                    address
                },
                title: get(title),
                description: get(description),
            }
        })
        .collect())
}

/// Seeds visible HTML drafts, using precisely the crawler's text and pixel functions.
/// A private row key preserves duplicate addresses; the exported address remains unchanged.
/// The synthetic HTML/200 classification admits drafts to the existing element filters;
/// no response headers, timestamps or transfer measurements are fabricated.
pub(crate) fn seed(file: &os_store::CrawlFile, rows: &[SerpRow]) -> Result<(), EngineError> {
    use os_model::{SerpSeed, UrlId, WriteBatch};
    use os_store::{CrawlSink, SqliteSink, StorageProfile};
    let mut writer = SqliteSink::open(file.path(), StorageProfile::Balanced)?;
    writer.write(WriteBatch {
        serp_seeds: rows
            .iter()
            .enumerate()
            .map(|(i, row)| SerpSeed {
                id: UrlId((i + 1) as i64),
                address: row.url.clone(),
                url_length: os_parse::text::char_len(&row.url),
                title: row.title.clone(),
                title_length: os_parse::text::char_len(&row.title),
                title_px: os_parse::pixel::width_px(&row.title, 20.0, false),
                description: row.description.clone(),
                description_length: os_parse::text::char_len(&row.description),
                description_px: os_parse::pixel::width_px(&row.description, 14.0, false),
            })
            .collect(),
        ..Default::default()
    })?;
    writer.finish()?;
    Ok(())
}
