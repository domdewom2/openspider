//! Structured Data: Validierungs- und Rich-Results-Reports (OUT-100 bis OUT-103, TECH-190).
//!
//! Grundlage sind die Befunde in `sd_issues` (M2-D20, Validierungsarten `schema_org` und `rich_results`) der Seiten
//! des Tabs Structured Data (interne HTML-Seiten mit Status 200) und nur von Elementen eingeschalteter Formate.
//! Erkannte Rich-Result-Features sind Seitenfakten mit dem Code `sd.google.feature`. Texte, Typ und Kennzeichnungen
//! kommen aus `os_analyze::tabs::structured_data` (eine Quelle für UI, Export und MCP). `% of Total` bezieht sich
//! auf alle Seiten des Tabs (Filter `All`), auf zwei Nachkommastellen gerundet.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use os_analyze::issues::Lang;
use os_analyze::tabs::structured_data as sd;
use os_analyze::ColumnKind;
use os_model::{CrawlConfig, Value};
use os_store::StoreError;
use serde_json::Value as Json;

use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::TableWriter;

use super::common::pct2;

#[cfg(test)]
#[path = "structured_data_tests.rs"]
mod tests;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const SUMMARY: &str = "Structured Data:Validation Errors & Warnings Summary";
const DETAIL: &str = "Structured Data:Validation Errors & Warnings";
const FEATURES_SUMMARY: &str = "Structured Data:Google Rich Results Features Summary";
const FEATURES: &str = "Structured Data:Google Rich Results Features";

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: SUMMARY,
        aliases: &[],
        file_stem: SUMMARY,
        header: &[
            "Validation Type",
            "Severity",
            "Type",
            "Feature",
            "Issue",
            "URLs",
            "% of Total",
            "Sample URL",
        ],
        requires: &[],
        owner: "M2-D20",
        run: run_summary,
        header_fn: None,
    },
    TableEntry {
        name: DETAIL,
        aliases: &[],
        file_stem: DETAIL,
        header: &[
            "Address",
            "Validation Type",
            "Severity",
            "Type",
            "Feature",
            "Issue",
            "Property Path",
        ],
        requires: &[],
        owner: "M2-D20",
        run: run_detail,
        header_fn: None,
    },
    TableEntry {
        name: FEATURES_SUMMARY,
        aliases: &[],
        file_stem: FEATURES_SUMMARY,
        header: &[
            "Feature",
            "URLs",
            "% of Total",
            "Valid",
            "With Warnings",
            "With Errors",
            "Sample URL",
        ],
        requires: &[],
        owner: "M2-D20",
        run: run_features_summary,
        header_fn: None,
    },
    TableEntry {
        name: FEATURES,
        aliases: &[],
        file_stem: FEATURES,
        header: &["Address"],
        header_fn: Some(features_header),
        requires: &[],
        owner: "M2-D20",
        run: run_features,
    },
];

/// Seiten des Tabs Structured Data (TECH-183).
const POPULATION: &str =
    "u.is_internal=1 AND u.in_tables=1 AND u.content_class='HTML' AND u.status_code=200";

fn db<T, E>(r: Result<T, E>) -> Result<T, ExportError>
where
    StoreError: From<E>,
{
    r.map_err(|e| ExportError::from(StoreError::from(e)))
}

/// SQL-Bedingung „Element in einem eingeschalteten Format“ für den Alias `s`.
fn active(cfg: &CrawlConfig) -> String {
    let c = &cfg.extraction.structured_data;
    format!(
        "((s.format='json-ld' AND {}) OR (s.format='microdata' AND {}) OR (s.format='rdfa' AND {}))",
        i32::from(c.json_ld),
        i32::from(c.microdata),
        i32::from(c.rdfa)
    )
}

/// Ein Validierungsbefund mit den Angaben, die die Reports zeigen.
pub(crate) struct Finding {
    pub address: String,
    pub validation: String,
    pub severity: String,
    pub feature: Option<String>,
    pub property: Option<String>,
    /// Typ des Knotens, an dem der Befund hängt.
    pub type_name: String,
    /// Eigener Meldungstext (englisch, wie die übrigen Exportinhalte).
    pub issue: String,
}

/// Liefert die Befunde (Fehler und Warnungen) in der Reihenfolge Adresse, Element, Befund. `where_sql` schränkt
/// zusätzlich ein (Aliase `u`, `s`, `i`; nur feste SQL-Teile und Ganzzahlen).
pub(crate) fn for_each_finding(
    job: &TableJob<'_>,
    where_sql: &str,
    f: &mut dyn FnMut(Finding) -> Result<(), ExportError>,
) -> Result<(), ExportError> {
    let sql = format!(
        "SELECT u.url, s.id, s.json, i.validation, i.severity, i.code, i.property, i.feature \
         FROM sd_issues i JOIN sd_items s ON s.id=i.item_id JOIN urls u ON u.id=i.url_id \
         WHERE i.validation IN ('schema_org','rich_results') AND i.severity IN ('error','warning') \
         AND {POPULATION} AND {} {where_sql} ORDER BY u.url, s.id, i.id",
        active(job.cfg)
    );
    let conn = job.file.conn();
    let mut stmt = db(conn.prepare(&sql))?;
    let mut rows = db(stmt.query([]))?;
    let mut item: Option<(i64, Json)> = None;
    while let Some(r) = db(rows.next())? {
        let item_id: i64 = db(r.get(1))?;
        if item.as_ref().is_none_or(|(id, _)| *id != item_id) {
            let json: String = db(r.get(2))?;
            item = Some((item_id, serde_json::from_str(&json).unwrap_or(Json::Null)));
        }
        let json = &item.as_ref().expect("Element gelesen").1;
        let code: String = db(r.get(5))?;
        let property: Option<String> = db(r.get(6))?;
        let feature: Option<String> = db(r.get(7))?;
        let path = property.as_deref().unwrap_or("");
        f(Finding {
            address: db(r.get(0))?,
            validation: db(r.get(3))?,
            severity: db(r.get(4))?,
            type_name: sd::finding_type(json, path).unwrap_or_default(),
            issue: sd::finding_text(
                &code,
                property.as_deref(),
                feature.as_deref(),
                Some(json),
                Lang::En,
            ),
            feature,
            property,
        })?;
    }
    Ok(())
}

/// Anzahl der Seiten des Tabs (Grundlage von `% of Total`).
fn total(job: &TableJob<'_>) -> Result<u64, ExportError> {
    let n: i64 = db(job.file.conn().query_row(
        &format!("SELECT COUNT(*) FROM urls u WHERE {POPULATION}"),
        [],
        |r| r.get(0),
    ))?;
    Ok(u64::try_from(n).unwrap_or(0))
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

fn pct(hits: u64, base: u64) -> Value {
    pct2(hits, base).map_or(Value::Null, Value::Float)
}

// --- OUT-100 ---------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Group {
    urls: u64,
    last: String,
    sample: String,
}

fn run_summary(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    // Schlüssel: Schwere (Fehler zuerst), Validierungsart, Typ, Feature, Meldung.
    let mut groups: BTreeMap<(u8, String, String, String, String), Group> = BTreeMap::new();
    for_each_finding(job, "", &mut |f| {
        let key = (
            u8::from(f.severity != "error"),
            sd::validation_label(&f.validation).to_owned(),
            f.type_name,
            f.feature.unwrap_or_default(),
            f.issue,
        );
        let g = groups.entry(key).or_default();
        if g.last != f.address {
            g.urls += 1;
            if g.sample.is_empty() || f.address < g.sample {
                g.sample.clone_from(&f.address);
            }
            g.last = f.address;
        }
        Ok(())
    })?;
    let base = total(job)?;
    let mut rows: Vec<_> = groups.into_iter().collect();
    rows.sort_by(|(a, ga), (b, gb)| a.0.cmp(&b.0).then(gb.urls.cmp(&ga.urls)).then(a.cmp(b)));
    let kinds = [
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Url,
    ];
    for ((severity, validation, ty, feature, issue), g) in &rows {
        writer.row(
            &kinds,
            &[
                text(validation),
                text(if *severity == 0 { "Error" } else { "Warning" }),
                text(ty),
                text(feature),
                text(issue),
                Value::Int(g.urls as i64),
                pct(g.urls, base),
                text(&g.sample),
            ],
        )?;
    }
    Ok(rows.len() as u64)
}

// --- OUT-101 ---------------------------------------------------------------------------------------------------

fn run_detail(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let kinds = [
        ColumnKind::Url,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
        ColumnKind::Text,
    ];
    let mut n = 0;
    for_each_finding(job, "", &mut |f| {
        writer.row(
            &kinds,
            &[
                text(&f.address),
                text(sd::validation_label(&f.validation)),
                text(sd::severity_label(&f.severity)),
                text(&f.type_name),
                text(f.feature.as_deref().unwrap_or("")),
                text(&f.issue),
                text(f.property.as_deref().unwrap_or("")),
            ],
        )?;
        n += 1;
        Ok(())
    })?;
    Ok(n)
}

// --- OUT-102, OUT-103 --------------------------------------------------------------------------------------------

/// Zustand eines Features auf einer Seite.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Valid,
    Warnings,
    Errors,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Valid => "Valid",
            Status::Warnings => "Warnings",
            Status::Errors => "Errors",
        }
    }
}

/// Adresse → Feature → Zustand, in Adressreihenfolge.
fn feature_status(
    job: &TableJob<'_>,
) -> Result<BTreeMap<String, BTreeMap<String, Status>>, ExportError> {
    let mut out: BTreeMap<String, BTreeMap<String, Status>> = BTreeMap::new();
    {
        let mut stmt = db(job.file.conn().prepare(&format!(
            "SELECT u.url, i.feature FROM sd_issues i JOIN urls u ON u.id=i.url_id \
             WHERE i.validation='rich_results' AND i.code='sd.google.feature' AND i.feature IS NOT NULL \
             AND {POPULATION} ORDER BY u.url, i.id"
        )))?;
        let rows =
            db(stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))))?;
        for row in rows {
            let (url, feature) = db(row)?;
            out.entry(url).or_default().insert(feature, Status::Valid);
        }
    }
    for_each_finding(job, "AND i.validation='rich_results'", &mut |f| {
        let status = if f.severity == "error" {
            Status::Errors
        } else {
            Status::Warnings
        };
        if let (Some(features), Some(feature)) = (out.get_mut(&f.address), f.feature) {
            if let Some(s) = features.get_mut(&feature) {
                *s = (*s).max(status);
            }
        }
        Ok(())
    })?;
    Ok(out)
}

fn run_features_summary(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
) -> Result<u64, ExportError> {
    #[derive(Default)]
    struct Count {
        urls: u64,
        by: [u64; 3],
        sample: String,
    }
    let mut counts: HashMap<String, Count> = HashMap::new();
    for (url, features) in feature_status(job)? {
        for (feature, status) in features {
            let c = counts.entry(feature).or_default();
            c.urls += 1;
            c.by[status as usize] += 1;
            if c.sample.is_empty() {
                c.sample.clone_from(&url);
            }
        }
    }
    let base = total(job)?;
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|(a, ca), (b, cb)| cb.urls.cmp(&ca.urls).then(a.cmp(b)));
    let kinds = [
        ColumnKind::Text,
        ColumnKind::Int,
        ColumnKind::Float { decimals: 2 },
        ColumnKind::Int,
        ColumnKind::Int,
        ColumnKind::Int,
        ColumnKind::Url,
    ];
    for (feature, c) in &rows {
        writer.row(
            &kinds,
            &[
                text(feature),
                Value::Int(c.urls as i64),
                pct(c.urls, base),
                Value::Int(c.by[Status::Valid as usize] as i64),
                Value::Int(c.by[Status::Warnings as usize] as i64),
                Value::Int(c.by[Status::Errors as usize] as i64),
                text(&c.sample),
            ],
        )?;
    }
    Ok(rows.len() as u64)
}

/// Spalten der Matrix: `Address` und je im Crawl erkanntem Feature eine Spalte (alphabetisch).
fn feature_names(job: &TableJob<'_>) -> Result<Vec<String>, ExportError> {
    let mut stmt = db(job.file.conn().prepare(&format!(
        "SELECT DISTINCT i.feature FROM sd_issues i JOIN urls u ON u.id=i.url_id \
         WHERE i.validation='rich_results' AND i.code='sd.google.feature' AND i.feature IS NOT NULL \
         AND {POPULATION}"
    )))?;
    let names = db(stmt.query_map([], |r| r.get::<_, String>(0)))?;
    let mut out = BTreeSet::new();
    for name in names {
        out.insert(db(name)?);
    }
    Ok(out.into_iter().collect())
}

fn features_header(job: &TableJob<'_>) -> Result<Vec<String>, ExportError> {
    let mut header = vec!["Address".to_owned()];
    header.extend(feature_names(job)?);
    Ok(header)
}

fn run_features(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let names = feature_names(job)?;
    let mut kinds = vec![ColumnKind::Url];
    kinds.extend(names.iter().map(|_| ColumnKind::Text));
    let mut n = 0;
    for (url, features) in feature_status(job)? {
        let mut row = vec![text(&url)];
        row.extend(
            names
                .iter()
                .map(|name| features.get(name).map_or(Value::Null, |s| text(s.label()))),
        );
        writer.row(&kinds, &row)?;
        n += 1;
    }
    Ok(n)
}

// --- OUT-056: Validierungszeilen der Tripel-Exporte ---------------------------------------------------------------

/// Validierungsbefunde eines Elements für die Tripel-Exporte (OUT-056): `(Pfad, Validation Type, Severity, Issue)`.
/// Der Export „Validation Errors“ zeigt nur schema.org-Fehler, „Validation Warnings“ nur schema.org-Warnungen,
/// „Rich Result Feature Detected“ nur Rich-Result-Befunde, alle übrigen alle Befunde.
pub(crate) fn triple_findings(
    job: &TableJob<'_>,
    item_id: i64,
    export: &str,
) -> Result<Vec<[String; 4]>, ExportError> {
    let filter = match export {
        "Validation Errors" => "AND i.validation='schema_org' AND i.severity='error'",
        "Validation Warnings" => "AND i.validation='schema_org' AND i.severity='warning'",
        "Rich Result Feature Detected" => "AND i.validation='rich_results'",
        "Parse Errors" => return Ok(Vec::new()),
        _ => "",
    };
    let mut out = Vec::new();
    for_each_finding(job, &format!("AND s.id={item_id} {filter}"), &mut |f| {
        out.push([
            f.property.unwrap_or_default(),
            sd::detail_validation_label(&f.validation, f.feature.as_deref()),
            sd::severity_label(&f.severity).to_owned(),
            f.issue,
        ]);
        Ok(())
    })?;
    Ok(out)
}
