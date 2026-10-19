//! Numerischer Metrikkatalog und spaltenstabile Zeitreihen (OUT-210 … 212).
//! Transport, Aufgabenidentität und Tabellenablage gehören X03/T01/L01/U27.
use std::collections::{BTreeMap, HashSet};

use os_analyze::{ColumnKind, TabDef};
use os_model::{Timestamp, Value};
use time::UtcOffset;

use crate::reports::{
    common::db,
    overview::{self, Cell},
};
use crate::{catalog::TableJob, error::ExportError};

const SUMMARY: &[&str] = &[
    "Total URLs Encountered",
    "Total URLs Crawled",
    "Total Internal Blocked by robots.txt",
    "Total External Blocked by robots.txt",
    "URLs Remaining In Queue",
    "Average Response Time (ms)",
    "Average URLs/s",
    "Skipped By Exclude",
    "Skipped Not In Include",
    "Skipped By Depth Limit",
    "Skipped By Redirect Limit",
    "Skipped By Folder Depth",
    "Skipped By Query Parameters",
    "Skipped By URL Length",
];
const AVERAGES: &[(&str, &str)] = &[
    ("Average Word Count", "word_count"),
    ("Average Crawl Depth", "crawl_depth"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metric {
    /// Kanonischer englischer Kopfzeilenname, zugleich stabile Kennung.
    pub name: String,
    pub kind: ColumnKind,
}

/// Neue Filter erscheinen automatisch; nur numerische Summary-Werte sind auswählbar.
pub fn catalog(tabs: &[TabDef]) -> Vec<Metric> {
    let mut out = Vec::new();
    for name in SUMMARY {
        out.push(Metric {
            name: (*name).into(),
            kind: if name.starts_with("Average") {
                ColumnKind::Float { decimals: 2 }
            } else {
                ColumnKind::Int
            },
        });
    }
    for tab in tabs {
        for filter in &tab.filters {
            out.push(Metric {
                name: format!(
                    "{}:{}",
                    tab.id,
                    crate::catalog::placeholders_to_x(filter.id)
                ),
                kind: ColumnKind::Int,
            });
        }
    }
    for kind in ["Issues", "Warnings", "Opportunities"] {
        for priority in ["High", "Medium", "Low"] {
            out.push(Metric {
                name: format!("{kind} ({priority})"),
                kind: ColumnKind::Int,
            });
        }
    }
    for (name, _) in AVERAGES {
        out.push(Metric {
            name: (*name).into(),
            kind: ColumnKind::Float { decimals: 2 },
        });
    }
    let mut seen = HashSet::new();
    out.retain(|m| seen.insert(m.name.clone()));
    out
}

/// CLI/Dialog übernehmen dieselben kanonischen Namen, eine Metrik je Zeile.
pub fn catalog_text(tabs: &[TabDef]) -> String {
    catalog(tabs)
        .into_iter()
        .map(|m| m.name)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Textliste einlesen, Duplikate entfernen, erste Reihenfolge beibehalten.
pub fn parse_selection(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|s| seen.insert((*s).to_owned()))
        .map(str::to_owned)
        .collect()
}

/// Eine Messung, ohne URL-Einzeldaten. Datum bleibt typisiert bis zur Transportgrenze.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    pub started_at: Timestamp,
    pub start_url: String,
    pub values: BTreeMap<String, Value>,
}

pub fn measure(job: &TableJob<'_>) -> Result<Measurement, ExportError> {
    let meta = job.file.meta()?;
    let started_at = meta
        .started_at
        .ok_or_else(|| ExportError::Write("Crawl-Start fehlt".into()))?;
    let start_url = if meta.mode == os_model::CrawlMode::List {
        job.file.get_meta("list_source")?.unwrap_or_default()
    } else {
        meta.start_url.unwrap_or_default()
    };
    let mut values: BTreeMap<String, Value> = catalog(job.tabs)
        .into_iter()
        .map(|m| (m.name, Value::Null))
        .collect();
    for row in overview::build(job.file, job.cfg, job.tabs)?.rows {
        let name = match row.section.as_str() {
            "Summary" | "Issues" => row.item,
            section if job.tabs.iter().any(|t| t.id == section) => {
                format!("{section}:{}", crate::catalog::placeholders_to_x(&row.item))
            }
            _ => continue,
        };
        if let Some(value) = values.get_mut(&name) {
            *value = match row.urls {
                Cell::Int(n) => Value::Int(n.min(i64::MAX as u64) as i64),
                Cell::Float(v) => Value::Float(v),
                _ => Value::Null,
            };
        }
    }
    // Filterlabels im Overview können Schwellen auflösen. Identität folgt immer der Registry.
    for count in os_analyze::views::filter_counts_in(job.tabs, job.file, job.cfg)? {
        let name = format!(
            "{}:{}",
            count.tab,
            crate::catalog::placeholders_to_x(&count.filter)
        );
        values.insert(
            name,
            count
                .count
                .map_or(Value::Null, |n| Value::Int(n.min(i64::MAX as u64) as i64)),
        );
    }
    for (name, column) in AVERAGES {
        let value:Option<f64>=db(job.file.conn().query_row(&format!("SELECT AVG({column}) FROM urls WHERE in_tables=1 AND is_internal=1 AND state='done' AND content_class='HTML' AND status_code BETWEEN 200 AND 299"),[],|r|r.get(0)))?;
        values.insert((*name).into(), value.map_or(Value::Null, Value::Float));
    }
    Ok(Measurement {
        started_at,
        start_url,
        values,
    })
}

/// Zelltyp für Sheets: Date ist ein numerischer serieller Datumswert, kein String.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Date(f64),
    Text(String),
    Number(f64),
    Empty,
}
#[derive(Debug, Clone, PartialEq)]
pub struct AlignedRow {
    pub header: Vec<String>,
    pub cells: Vec<CellValue>,
    /// Hinweise für Lauf-Log und Verlauf; der Transport entscheidet über deren Darstellung.
    pub warnings: Vec<String>,
}

/// Bestehende Spalten bleiben stehen, neu gewählte Namen werden rechts angefügt.
/// Entfernte/nicht befüllte Metriken bekommen leere Zellen statt einer irreführenden Null.
pub fn align(
    existing: &[String],
    selected: &[String],
    measurement: &Measurement,
    offset: UtcOffset,
) -> Result<AlignedRow, ExportError> {
    let mut header = if existing.is_empty() {
        vec!["Date".into(), "Start URL".into()]
    } else {
        existing.to_vec()
    };
    if header.first().map(String::as_str) != Some("Date")
        || header.get(1).map(String::as_str) != Some("Start URL")
    {
        return Err(ExportError::Write(
            "Zeitreihe erwartet Date und Start URL als erste Spalten".into(),
        ));
    }
    let mut seen = HashSet::new();
    if !header.iter().all(|n| seen.insert(n.clone())) {
        return Err(ExportError::Write("Doppelte Zeitreihenspalte".into()));
    }
    for name in selected {
        if seen.insert(name.clone()) {
            header.push(name.clone());
        }
    }
    let chosen: HashSet<&str> = selected.iter().map(String::as_str).collect();
    let mut warnings = Vec::new();
    let mut cells = vec![
        CellValue::Date(date_serial(measurement.started_at, offset)),
        CellValue::Text(measurement.start_url.clone()),
    ];
    for name in header.iter().skip(2) {
        let value = if chosen.contains(name.as_str()) {
            measurement.values.get(name)
        } else {
            None
        };
        cells.push(match value {
            Some(Value::Int(n)) => CellValue::Number(*n as f64),
            Some(Value::Float(v)) if v.is_finite() => CellValue::Number(*v),
            _ => {
                if chosen.contains(name.as_str()) {
                    warnings.push(format!("metric '{name}' is no longer populated"));
                }
                CellValue::Empty
            }
        });
    }
    Ok(AlignedRow {
        header,
        cells,
        warnings,
    })
}

pub const DATE_FORMAT: &str = "yyyy-mm-dd hh:mm:ss";

/// Sheets/Excel-Epoche 1899-12-30; Offset wird vom Aufrufer aus der lokalen Zeitzone
/// zum Crawlstart bestimmt (auch bei historischen Sommerzeitwechseln deterministisch).
pub fn date_serial(at: Timestamp, offset: UtcOffset) -> f64 {
    const UNIX_EPOCH_SERIAL: f64 = 25569.0;
    UNIX_EPOCH_SERIAL + (at.0.unix_timestamp() as f64 + f64::from(offset.whole_seconds())) / 86400.0
}
