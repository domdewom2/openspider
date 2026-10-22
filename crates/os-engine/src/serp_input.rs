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
    let conn = file.conn();
    let db = |e| EngineError::Store(os_store::StoreError::from(e));
    conn.execute_batch("BEGIN IMMEDIATE").map_err(db)?;
    let result = (|| {
        for (i, row) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO urls (id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,
                 title_count,title_1,title_1_len,title_1_px,meta_desc_count,meta_desc_1,meta_desc_1_len,meta_desc_1_px,
                 effective_index,effective_follow,indexability,change_seq,status_code)
                 VALUES (?1,?2,?3,'',1,1,'done','list_input',0,?4,'HTML',1,?5,?6,?7,1,?8,?9,?10,1,1,'Indexable',1,200)",
                ((i + 1) as i64, format!("urn:openspider:serp:{}", i + 1), &row.url,
                 os_parse::text::char_len(&row.url), &row.title, os_parse::text::char_len(&row.title),
                 os_parse::pixel::width_px(&row.title, 20.0, false), &row.description,
                 os_parse::text::char_len(&row.description), os_parse::pixel::width_px(&row.description, 14.0, false)),
            ).map_err(db)?;
        }
        Ok::<_, EngineError>(())
    })();
    if result.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    } else {
        conn.execute_batch("COMMIT").map_err(db)?;
    }
    result
}
