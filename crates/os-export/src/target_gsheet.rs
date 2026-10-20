//! Streaming RAW Sheets exports and stable, numeric crawl time series (M2-X03).
//! Transport is shared with the integrations library; callers can keep local exports
//! independent by handling the returned error for this target only.
use crate::{
    timeseries::{self, CellValue, Measurement},
    ExportError,
};
use os_integrations::drive::{self, DriveClient};
use os_model::{CancelToken, Value};
use serde_json::json;
use time::UtcOffset;

pub const DEFAULT_CELL_LIMIT: u64 = 10_000_000;
#[derive(Debug, Clone)]
pub struct Destination {
    pub root: String,
    pub project: String,
    pub crawl: String,
    pub name: String,
    pub cell_limit: u64,
}
impl Destination {
    pub fn new(project: &str, crawl: &str, name: &str) -> Self {
        Self {
            root: "OpenSpider".into(),
            project: project.into(),
            crawl: crawl.into(),
            name: name.into(),
            cell_limit: DEFAULT_CELL_LIMIT,
        }
    }
}
#[derive(Debug)]
pub struct SheetResult {
    pub id: String,
    pub url: String,
    pub rows: u64,
    pub warnings: Vec<String>,
}
/// Exact row count is required before any remote write. Rows are consumed in blocks;
/// callers can supply a disk-backed iterator instead of holding a crawl in memory.
pub struct Table<I> {
    pub name: String,
    pub header: Vec<String>,
    pub row_count: u64,
    pub rows: I,
}
fn failure(e: impl std::fmt::Display) -> ExportError {
    ExportError::Write(e.to_string())
}
fn check(cancel: &CancelToken) -> Result<(), ExportError> {
    cancel
        .checkpoint()
        .map_err(|_| failure("Export abgebrochen"))
}
fn validate(destination: &Destination) -> Result<(), ExportError> {
    if [
        &destination.root,
        &destination.project,
        &destination.crawl,
        &destination.name,
    ]
    .iter()
    .any(|s| s.trim().is_empty())
        || destination.cell_limit == 0
    {
        return Err(failure("Drive-Ziel und Zellenlimit müssen gesetzt sein"));
    }
    Ok(())
}
/// One spreadsheet per call, with one or several worksheets. Same-name replacement
/// retains both file ID and matching sheet IDs; obsolete sheets/values are removed.
pub async fn export<I>(
    client: &DriveClient,
    destination: &Destination,
    tables: Vec<Table<I>>,
    cancel: &CancelToken,
) -> Result<SheetResult, ExportError>
where
    I: IntoIterator<Item = Result<Vec<Value>, ExportError>>,
{
    validate(destination)?;
    check(cancel)?;
    let mut sheet_names = Vec::new();
    let tables: Vec<_> = tables
        .into_iter()
        .map(|mut table| {
            table.name = crate::xlsx::unique_sheet_name(&table.name, &sheet_names);
            sheet_names.push(table.name.clone());
            table
        })
        .collect();
    if tables.is_empty() {
        return Err(failure("Keine Tabellen ausgewählt"));
    }
    let mut names = std::collections::HashSet::new();
    let headers = tables.iter().try_fold(0u64, |sum, t| {
        if t.header.is_empty() || t.name.is_empty() || !names.insert(t.name.clone()) {
            return Err(failure("Ungültige oder doppelte Tabellenköpfe"));
        }
        sum.checked_add(t.header.len() as u64)
            .ok_or_else(|| failure("Zellenlimit"))
    })?;
    if headers > destination.cell_limit {
        return Err(failure(
            "Zellenlimit reicht nicht für Kopfzeilen; CSV/XLSX verwenden",
        ));
    }
    let mut budget = destination.cell_limit - headers;
    let mut allowed = Vec::new();
    let mut warnings = Vec::new();
    for table in &tables {
        let rows = table.row_count.min(budget / table.header.len() as u64);
        budget -= rows * table.header.len() as u64;
        allowed.push(rows);
        if rows < table.row_count {
            warnings.push(format!(
                "{}: {} Zeilen abgeschnitten (Zellenlimit); CSV/XLSX empfohlen",
                table.name,
                table.row_count - rows
            ));
        }
    }
    let parent = client
        .folder(
            &destination.root,
            &destination.project,
            Some(&destination.crawl),
        )
        .await
        .map_err(failure)?;
    // A file can disappear between lookup and its first read. Resolve once again on 404.
    let (id, existing) = resolve(client, &parent, &destination.name).await?;
    let mut next_id = existing.iter().map(|s| s.id).max().unwrap_or(0) + 1;
    let mut requests = Vec::new();
    // Shrink/remove old grids first to avoid temporarily exceeding Google's cell limit.
    for sheet in &existing {
        requests.push(json!({"updateSheetProperties":{"properties":{"sheetId":sheet.id,"gridProperties":{"rowCount":1,"columnCount":1}},"fields":"gridProperties(rowCount,columnCount)"}}));
        requests.push(
            json!({"updateCells":{"range":{"sheetId":sheet.id},"fields":"userEnteredValue"}}),
        );
    }
    let mut keep = Vec::new();
    let mut resize = Vec::new();
    for (table, rows) in tables.iter().zip(&allowed) {
        let id = existing
            .iter()
            .find(|s| s.title == table.name)
            .map(|s| s.id)
            .unwrap_or_else(|| {
                let id = next_id;
                next_id += 1;
                id
            });
        keep.push(id);
        let properties = json!({"sheetId":id,"title":table.name,"gridProperties":{"rowCount":rows+1,"columnCount":table.header.len()}});
        if !existing.iter().any(|s| s.id == id) {
            requests.push(json!({"addSheet":{"properties":{"sheetId":id,"title":table.name,"gridProperties":{"rowCount":1,"columnCount":1}}}}));
        }
        resize.push(json!({"updateSheetProperties":{"properties":properties,"fields":"gridProperties(rowCount,columnCount)"}}));
    }
    for sheet in existing.iter().filter(|s| !keep.contains(&s.id)) {
        requests.push(json!({"deleteSheet":{"sheetId":sheet.id}}));
    }
    requests.extend(resize);
    check(cancel)?;
    client.batch(&id, requests).await.map_err(failure)?;
    let mut written = 0;
    for (table, limit) in tables.into_iter().zip(allowed) {
        check(cancel)?;
        let name = drive::quote_sheet(&table.name);
        client
            .write(
                &id,
                &format!("{name}!A1"),
                &[table.header.iter().map(|s| json!(s)).collect()],
            )
            .await
            .map_err(failure)?;
        let mut rows = table.rows.into_iter();
        let mut remaining = limit;
        let mut position = 2;
        while remaining > 0 {
            check(cancel)?;
            let mut block = Vec::new();
            for _ in 0..remaining.min(10_000) {
                let row = rows
                    .next()
                    .ok_or_else(|| failure("Zeilenzahl stimmt nicht mit Datenquelle überein"))??;
                if row.len() != table.header.len() {
                    return Err(failure("Spaltenzahl stimmt nicht mit Kopfzeile überein"));
                }
                block.push(row.into_iter().map(raw).collect());
            }
            check(cancel)?;
            client
                .write(&id, &format!("{name}!A{position}"), &block)
                .await
                .map_err(failure)?;
            let count = block.len() as u64;
            remaining -= count;
            position += count;
            written += count;
        }
    }
    for warning in &warnings {
        tracing::warn!("{warning}");
    }
    Ok(SheetResult {
        url: drive::link(&id),
        id,
        rows: written,
        warnings,
    })
}
async fn resolve(
    client: &DriveClient,
    parent: &str,
    name: &str,
) -> Result<(String, Vec<drive::Sheet>), ExportError> {
    for _ in 0..2 {
        let id = client
            .find_or_create(parent, name, drive::SPREADSHEET)
            .await
            .map_err(failure)?;
        match client.sheets(&id).await {
            Ok(sheets) => return Ok((id, sheets)),
            Err(os_integrations::ApiError::Provider { status: 404, .. }) => continue,
            Err(e) => return Err(failure(e)),
        }
    }
    Err(failure("Drive-Tabelle wurde während des Exports entfernt"))
}
fn raw(value: Value) -> serde_json::Value {
    match value {
        Value::Null => json!(""),
        Value::Bool(b) => json!(b),
        Value::Int(n) => json!(n),
        Value::Float(f) if f.is_finite() => json!(f),
        Value::Text(s) => json!(s),
        _ => json!(""),
    }
}
/// Append one measurement to OpenSpider/<Project>/<Task>_custom_summary_report.
/// A host serializes concurrent writes to the same destination (the App does so).
/// Date values remain numbers and receive an explicit DATE_TIME format.
pub async fn custom_summary(
    client: &DriveClient,
    destination: &Destination,
    selected: &[String],
    measurement: &Measurement,
    offset: UtcOffset,
    cancel: &CancelToken,
) -> Result<SheetResult, ExportError> {
    validate(destination)?;
    check(cancel)?;
    let parent = client
        .folder(&destination.root, &destination.project, None)
        .await
        .map_err(failure)?;
    let name = format!("{}_custom_summary_report", destination.name);
    let (id, sheets) = resolve(client, &parent, &name).await?;
    let sheet = if let Some(sheet) = sheets.iter().find(|s| s.title == "data") {
        sheet.clone()
    } else {
        // Reuse the default empty sheet; any additional sheets remain untouched.
        let sheet = sheets
            .first()
            .ok_or_else(|| failure("Tabellenblatt fehlt"))?;
        client.batch(&id,vec![json!({"updateSheetProperties":{"properties":{"sheetId":sheet.id,"title":"data"},"fields":"title"}})]).await.map_err(failure)?;
        drive::Sheet {
            id: sheet.id,
            title: "data".into(),
        }
    };
    let existing = client.values(&id, "'data'").await.map_err(failure)?;
    let header = existing
        .first()
        .map(|r| {
            r.iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| failure("Ungültige Zeitreihen-Kopfzeile"))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let aligned = timeseries::align(&header, selected, measurement, offset)?;
    let row_count = existing.len().max(1) + 1;
    if row_count as u64 > destination.cell_limit / aligned.header.len() as u64 {
        return Err(failure(
            "Zellenlimit der Zeitreihe erreicht; CSV/XLSX verwenden",
        ));
    }
    client.batch(&id,vec![json!({"updateSheetProperties":{"properties":{"sheetId":sheet.id,"gridProperties":{"rowCount":row_count,"columnCount":aligned.header.len()}},"fields":"gridProperties(rowCount,columnCount)"}}),json!({"repeatCell":{"range":{"sheetId":sheet.id,"startRowIndex":1,"startColumnIndex":0,"endColumnIndex":1},"cell":{"userEnteredFormat":{"numberFormat":{"type":"DATE_TIME","pattern":timeseries::DATE_FORMAT}}},"fields":"userEnteredFormat.numberFormat"}})]).await.map_err(failure)?;
    check(cancel)?;
    client
        .write(
            &id,
            "'data'!A1",
            &[aligned.header.iter().map(|s| json!(s)).collect()],
        )
        .await
        .map_err(failure)?;
    let cells = aligned
        .cells
        .into_iter()
        .map(|c| match c {
            CellValue::Date(n) | CellValue::Number(n) => json!(n),
            CellValue::Text(s) => json!(s),
            CellValue::Empty => json!(""),
        })
        .collect();
    client
        .write(&id, &format!("'data'!A{row_count}"), &[cells])
        .await
        .map_err(failure)?;
    for warning in &aligned.warnings {
        tracing::warn!("{warning}");
    }
    Ok(SheetResult {
        url: drive::link(&id),
        id,
        rows: 1,
        warnings: aligned.warnings,
    })
}

/// Disk-backed adapter for the existing synchronous TableWriter pipeline. It stores
/// typed JSON lines, preserving formulas as text until RAW upload. The caller owns
/// the staging path/lifetime, e.g. a tempfile beside other export staging files.
pub struct StagingWriter {
    path: std::path::PathBuf,
    file: std::io::BufWriter<std::fs::File>,
    columns: usize,
    rows: u64,
}
impl StagingWriter {
    pub fn create(path: &std::path::Path) -> Result<Self, ExportError> {
        let file = std::fs::File::create(path).map_err(failure)?;
        Ok(Self {
            path: path.into(),
            file: std::io::BufWriter::new(file),
            columns: 0,
            rows: 0,
        })
    }
    fn line(&mut self, value: &impl serde::Serialize) -> Result<(), ExportError> {
        use std::io::Write;
        serde_json::to_writer(&mut self.file, value).map_err(failure)?;
        self.file.write_all(b"\n").map_err(failure)
    }
}
impl crate::TableWriter for StagingWriter {
    fn header(&mut self, columns: &[String]) -> Result<(), ExportError> {
        if self.columns != 0 || columns.is_empty() {
            return Err(failure("Ungültige Kopfzeile"));
        }
        self.columns = columns.len();
        self.line(&columns)
    }
    fn row(
        &mut self,
        _kinds: &[os_analyze::ColumnKind],
        values: &[Value],
    ) -> Result<(), ExportError> {
        if self.columns == 0 || values.len() != self.columns {
            return Err(failure("Ungültige Spaltenzahl"));
        }
        self.line(&values)?;
        self.rows += 1;
        Ok(())
    }
    fn finish(mut self: Box<Self>) -> Result<crate::ExportFileInfo, ExportError> {
        use std::io::Write;
        self.file.flush().map_err(failure)?;
        Ok(crate::ExportFileInfo {
            bytes: self.file.get_ref().metadata().map_err(failure)?.len(),
            path: self.path.clone(),
            rows: self.rows,
            warnings: vec![],
        })
    }
}
pub struct StagedRows(std::io::Lines<std::io::BufReader<std::fs::File>>);
impl Iterator for StagedRows {
    type Item = Result<Vec<Value>, ExportError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|line| serde_json::from_str(&line.map_err(failure)?).map_err(failure))
    }
}
pub fn staged_table(
    name: &str,
    info: &crate::ExportFileInfo,
) -> Result<Table<StagedRows>, ExportError> {
    use std::io::BufRead;
    let mut lines =
        std::io::BufReader::new(std::fs::File::open(&info.path).map_err(failure)?).lines();
    let header = serde_json::from_str(
        &lines
            .next()
            .ok_or_else(|| failure("Kopfzeile fehlt"))?
            .map_err(failure)?,
    )
    .map_err(failure)?;
    Ok(Table {
        name: name.into(),
        header,
        row_count: info.rows,
        rows: StagedRows(lines),
    })
}

/// Typed staging for the shared export catalog. The caller owns the temporary
/// directory until upload finishes; no crawl-sized allocation is required.
pub struct StagedBatch {
    pub tables: Vec<(String, crate::ExportFileInfo)>,
    pub warnings: Vec<String>,
    pub skipped: Vec<String>,
}
struct StagingWorkbook {
    directory: std::path::PathBuf,
    sheets: Vec<(String, Box<StagingWriter>)>,
}
impl crate::types::WorkbookTarget for StagingWorkbook {
    fn sheet(&mut self, name: &str) -> Result<&mut dyn crate::TableWriter, ExportError> {
        let path = self
            .directory
            .join(format!("sheet-{}.jsonl", self.sheets.len()));
        self.sheets
            .push((name.into(), Box::new(StagingWriter::create(&path)?)));
        Ok(self
            .sheets
            .last_mut()
            .expect("sheet just inserted")
            .1
            .as_mut())
    }
}
/// Use the same names, prerequisites, ordering, warnings and empty suppression as
/// local multi-export. Binary-only exports cannot be represented in Sheets.
pub fn stage_batch(
    file: &os_store::CrawlFile,
    cfg: &os_model::CrawlConfig,
    selections: &[crate::types::ExportSelection],
    directory: &std::path::Path,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<StagedBatch, ExportError> {
    use crate::TableWriter;
    let mut workbook = StagingWorkbook {
        directory: directory.into(),
        sheets: Vec::new(),
    };
    let metadata = file.meta()?;
    let mut filtered = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    for selection in selections {
        if matches!(
            selection.name.as_str(),
            "Web:Screenshots" | "Web:All Page Source" | "Web:All Page Text"
        ) {
            skipped.push(selection.name.clone());
            warnings.push(format!(
                "{}: Binärdateien sind kein Sheets-Tabellenexport",
                selection.name
            ));
        } else {
            filtered.push(selection.clone());
        }
    }
    let mut control = crate::types::ExportCtx {
        cancel: ctx.cancel,
        progress: ctx.progress,
        skip_empty: ctx.skip_empty,
        workbook: Some(&mut workbook),
        skipped: Vec::new(),
    };
    let outcome = crate::pipeline::export_batch(
        file,
        cfg,
        &filtered,
        &crate::pipeline::BatchOptions {
            format: crate::Format::Csv,
            directory,
            crawl_name: &metadata.name,
            single_workbook: false,
        },
        &mut control,
    )?;
    skipped.extend(outcome.skipped);
    warnings.extend(outcome.warnings);
    let tables = workbook
        .sheets
        .into_iter()
        .map(|(name, writer)| writer.finish().map(|info| (name, info)))
        .collect::<Result<_, _>>()?;
    ctx.skipped.extend(skipped.clone());
    Ok(StagedBatch {
        tables,
        warnings,
        skipped,
    })
}
