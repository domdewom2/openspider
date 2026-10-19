//! Structured Data: Exporte als RDF-Tripel (OUT-056).
//!
//! Streams selected URLs through the shared view plan; blank nodes are deterministic per URL.

use crate::catalog::{TableEntry, TableJob};
use crate::{
    error::ExportError,
    types::{ExportFileInfo, Format, TableWriter},
};
use os_analyze::{
    views::{self, ColumnSelection, ViewQuery},
    ColumnKind,
};
use os_model::Value;
use serde_json::Value as Json;
use std::{collections::HashSet, path::Path};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Structured Data:Contains Structured Data Triples",
        aliases: &[],
        file_stem: "Structured Data:Contains Structured Data Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:Validation Errors Triples",
        aliases: &[],
        file_stem: "Structured Data:Validation Errors Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:Validation Warnings Triples",
        aliases: &[],
        file_stem: "Structured Data:Validation Warnings Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:Parse Errors Triples",
        aliases: &[],
        file_stem: "Structured Data:Parse Errors Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:Microdata URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:Microdata URLs Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:JSON-LD URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:JSON-LD URLs Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:RDFa URLs Triples",
        aliases: &[],
        file_stem: "Structured Data:RDFa URLs Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Structured Data:Rich Result Feature Detected Triples",
        aliases: &[],
        file_stem: "Structured Data:Rich Result Feature Detected Triples",
        header: &[
            "Address",
            "Format",
            "Subject",
            "Predicate",
            "Object",
            "Validation Type",
            "Severity",
            "Issue",
        ],
        requires: &[],
        owner: "M2-D09",
        run,
        header_fn: None,
    },
];

fn enabled(cfg: &os_model::CrawlConfig, format: &str) -> bool {
    let c = &cfg.extraction.structured_data;
    match format {
        "json-ld" => c.json_ld,
        "microdata" => c.microdata,
        "rdfa" => c.rdfa,
        _ => false,
    }
}
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let filter = job
        .entry_name
        .split_once(':')
        .map_or("Contains Structured Data", |(_, f)| {
            f.trim_end_matches(" Triples")
        });
    let mut q = ViewQuery::new(
        "Structured Data",
        if filter == "Rich Result Feature Detected" {
            "Contains Structured Data"
        } else {
            filter
        },
    );
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    let mut count = 0;
    let mut failure = None;
    let result = views::stream_view(job.file, job.cfg, &q, &mut |row| {
        if let Some(Value::Text(address)) = row.first() {
            match write_url(job, writer, address, filter) {
                Ok(n) => count += n,
                Err(e) => {
                    failure = Some(e);
                    return Err(os_analyze::ViewError::Store(
                        os_store::StoreError::InvalidInput("Structured data export failed".into()),
                    ));
                }
            }
        }
        Ok(())
    });
    if let Some(e) = failure {
        return Err(e);
    }
    result?;
    Ok(count)
}
fn write_url(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    address: &str,
    filter: &str,
) -> Result<u64, ExportError> {
    if filter == "Rich Result Feature Detected" {
        let found: bool = job.file.conn().query_row("SELECT EXISTS(SELECT 1 FROM sd_issues i JOIN urls u ON u.id=i.url_id WHERE u.url=?1 AND i.validation='rich_results' AND i.feature IS NOT NULL)", [address], |r| r.get(0)).map_err(os_store::StoreError::from)?;
        if !found {
            return Ok(0);
        }
    }
    let mut stmt = job.file.conn().prepare("SELECT s.format, s.json FROM sd_items s JOIN urls u ON u.id=s.url_id WHERE u.url=?1 ORDER BY CASE s.format WHEN 'json-ld' THEN 0 WHEN 'microdata' THEN 1 ELSE 2 END,s.id").map_err(os_store::StoreError::from)?;
    let items = stmt
        .query_map([address], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(os_store::StoreError::from)?;
    let mut n = 0;
    let mut blanks = 0;
    let mut seen = HashSet::new();
    for item in items {
        let (format, json) = item.map_err(os_store::StoreError::from)?;
        if !enabled(job.cfg, &format) {
            continue;
        }
        if matches!(filter, "JSON-LD URLs" | "Microdata URLs" | "RDFa URLs")
            && !filter.to_lowercase().starts_with(&format)
        {
            continue;
        }
        let v: Json = serde_json::from_str(&json).map_err(|e| ExportError::Write(e.to_string()))?;
        if let Some(error) = v.get("_os_parse_error") {
            if filter == "Parse Errors" {
                let issue = format!(
                    "{} (line {}, column {})",
                    error["message"].as_str().unwrap_or("Parse Error"),
                    error["line"],
                    error["column"]
                );
                writer.row(
                    &[ColumnKind::Text; 8],
                    &[address, &format, "", "", "", "Parse", "Error", &issue]
                        .map(|s| Value::Text(s.into())),
                )?;
                n += 1;
            }
            continue;
        }
        if filter == "Parse Errors" {
            continue;
        }
        // Validation exports are completed by D20; these paths already share the selection contract.
        let mut triples = Vec::new();
        triples_for(&v, &mut blanks, &mut triples);
        for (s, p, o) in triples {
            if seen.insert((s.clone(), p.clone(), o.clone())) {
                writer.row(
                    &[ColumnKind::Text; 8],
                    &[address, &format, &s, &p, &o, "", "", ""].map(|s| Value::Text(s.into())),
                )?;
                n += 1;
            }
        }
    }
    Ok(n)
}
fn compact(iri: &str) -> String {
    if !iri.contains(':') {
        return format!("schema:{iri}");
    }
    iri.strip_prefix("https://schema.org/")
        .or_else(|| iri.strip_prefix("http://schema.org/"))
        .map_or_else(|| iri.into(), |s| format!("schema:{s}"))
}
fn triples_for(v: &Json, blank: &mut usize, out: &mut Vec<(String, String, String)>) -> String {
    if !v.is_object() {
        return serde_json::to_string(v).expect("JSON literal");
    }
    if let Some(value) = v.get("@value") {
        return serde_json::to_string(value).expect("JSON literal");
    }
    let subject = v
        .get("@id")
        .and_then(Json::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let id = format!("_:b{blank}");
            *blank += 1;
            id
        });
    let ty = v.get("@type");
    for t in ty
        .into_iter()
        .flat_map(|t| t.as_array().map_or_else(|| vec![t], |a| a.iter().collect()))
    {
        if let Some(t) = t.as_str() {
            out.push((subject.clone(), "rdf:type".into(), compact(t)));
        }
    }
    let obj = v.as_object().expect("object");
    let mut order = v
        .get("_os_order")
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(Json::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for key in obj.keys() {
        if !order.contains(&key.as_str()) {
            order.push(key);
        }
    }
    for key in order {
        if key.starts_with('@') || key.starts_with("_os_") {
            continue;
        }
        let Some(values) = obj.get(key) else {
            continue;
        };
        let predicate = if key.contains(':') {
            compact(key)
        } else {
            format!("schema:{key}")
        };
        for value in values
            .as_array()
            .map_or_else(|| vec![values], |a| a.iter().collect())
        {
            let object = triples_for(value, blank, out);
            out.push((subject.clone(), predicate.clone(), object));
        }
    }
    subject
}

/// JSON tree export, selected by `format: "json"` in params or a `.json` destination.
/// Other formats use the same selection-scoped detail table as the app.
pub fn export_detail(
    job: &TableJob<'_>,
    req: &os_analyze::details::DetailReq<'_>,
    path: &Path,
    format: Format,
) -> Result<ExportFileInfo, ExportError> {
    let data = os_store::queries::structured_data::detail_rows(
        job.file,
        req.url_ids,
        req.offset,
        req.limit,
    )?;
    if path.extension().is_some_and(|e| e == "json")
        || req.params.get("format").and_then(Json::as_str) == Some("json")
    {
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        for row in &data.rows {
            if let (Value::Int(id), Value::Text(json)) = (&row[1], &row[4]) {
                if seen.insert(*id) {
                    let value: Json = serde_json::from_str(json)
                        .map_err(|e| ExportError::Write(e.to_string()))?;
                    items.push(
                        serde_json::json!({"address":row[0],"id":id,"format":row[2],"item":value}),
                    );
                }
            }
        }
        let bytes =
            serde_json::to_vec_pretty(&items).map_err(|e| ExportError::Write(e.to_string()))?;
        std::fs::write(path, &bytes).map_err(|source| ExportError::Io {
            path: path.into(),
            source,
        })?;
        return Ok(ExportFileInfo {
            path: path.into(),
            rows: items.len() as u64,
            bytes: bytes.len() as u64,
            warnings: Vec::new(),
        });
    }
    let options = crate::pipeline::writer_options(job.cfg);
    let mut writer = crate::output::open_writer(path, format, job.entry_name, &options)?;
    writer.header(&data.columns)?;
    let mut kinds = vec![ColumnKind::Text; data.columns.len()];
    kinds[1] = ColumnKind::Int;
    for row in data.rows {
        writer.row(&kinds, &row)?;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp, UrlId};
    fn fixture() -> (tempfile::TempDir, os_store::CrawlFile, CrawlConfig) {
        let dir = tempfile::tempdir().unwrap();
        let ids = SeededIds::new(9);
        let at = Timestamp::UNIX_EPOCH;
        let cfg = CrawlConfig::default();
        let init = os_store::CrawlInit {
            crawl_id: ids.new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: String::new(),
            name: "SD".into(),
            config: cfg.clone(),
            created_at: at,
        };
        let file = os_store::CrawlFile::create(
            &dir.path().join("sd.ospider"),
            &init,
            os_store::StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/tech-m2-sd/store.sql"
            ))
            .unwrap();
        let mut cfg = cfg;
        cfg.extraction.structured_data.json_ld = true;
        (dir, file, cfg)
    }
    // spec: OUT-056
    #[test]
    fn deterministic_product_triples_and_format_filters() {
        let (dir, file, cfg) = fixture();
        let tabs = os_analyze::tabs::configured(&cfg);
        let job = TableJob {
            entry_name: ENTRIES[0].name,
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let a = dir.path().join("a.csv");
        let b = dir.path().join("b.csv");
        crate::catalog::export_table(&ENTRIES[0], &job, &a, Format::Csv).unwrap();
        crate::catalog::export_table(&ENTRIES[0], &job, &b, Format::Csv).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let text = std::fs::read_to_string(&a).unwrap();
        os_testsite::golden::assert_golden(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/tech-m2-sd/triples.csv"),
            &text,
        );
        for wanted in [
            "_:b0,rdf:type,schema:Product",
            "_:b0,schema:name",
            "_:b0,schema:offers,_:b1",
            "_:b1,schema:price",
        ] {
            assert!(text.contains(wanted), "{wanted}: {text}");
        }
        assert!(!text.contains("_os_"));
        assert!(!text.contains("schema:Recipe"));
        let mut micro_cfg = cfg.clone();
        micro_cfg.extraction.structured_data.microdata = true;
        let job = TableJob {
            cfg: &micro_cfg,
            entry_name: ENTRIES[4].name,
            ..job
        };
        let path = dir.path().join("micro.xlsx");
        let info = crate::catalog::export_table(&ENTRIES[4], &job, &path, Format::Xlsx).unwrap();
        assert!(info.rows > 0);
    }
    // spec: OUT-056
    // spec: TECH-186
    #[test]
    fn parse_errors_export_message_and_coordinates() {
        let (dir, file, cfg) = fixture();
        let tabs = os_analyze::tabs::configured(&cfg);
        let job = TableJob {
            entry_name: ENTRIES[3].name,
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let path = dir.path().join("errors.csv");
        let info = crate::catalog::export_table(&ENTRIES[3], &job, &path, Format::Csv).unwrap();
        assert!(info.rows >= 1);
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("Trailing comma (line 2, column 9)"));
        assert!(!text.contains("schema:name"));
    }
    // spec: TECH-189
    #[test]
    fn detail_exports_selected_tree_as_json_csv_and_xlsx() {
        let (dir, file, cfg) = fixture();
        let tabs = os_analyze::tabs::configured(&cfg);
        let job = TableJob {
            entry_name: "SD Details",
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let params = serde_json::json!({"tree":true});
        let req = os_analyze::details::DetailReq {
            url_ids: &[UrlId(2)],
            params: &params,
            sort: &[],
            offset: 0,
            limit: 20,
        };
        let path = dir.path().join("tree.json");
        let info = export_detail(&job, &req, &path, Format::Csv).unwrap();
        assert_eq!(info.rows, 1);
        let tree: Json = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(tree[0]["item"]["_os_parse_error"]["column"], 9);
        for format in [Format::Csv, Format::Xlsx] {
            let path = dir.path().join(format!("detail.{}", format.extension()));
            assert_eq!(export_detail(&job, &req, &path, format).unwrap().rows, 1);
        }
    }
}
