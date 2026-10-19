//! Selection-scoped structured data tree and findings (TECH-189).
use super::DetailData;
use crate::{CrawlFile, StoreError};
use os_model::{UrlId, Value};
use rusqlite::params;

pub fn detail_rows(
    file: &CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
) -> Result<DetailData, StoreError> {
    let ids = serde_json::to_string(&ids.iter().map(|id| id.0).collect::<Vec<_>>())
        .expect("integer list");
    const FROM: &str = "FROM sd_items s JOIN urls u ON u.id=s.url_id LEFT JOIN sd_issues i ON i.item_id=s.id WHERE s.url_id IN (SELECT value FROM json_each(?1))";
    let total = file
        .conn()
        .query_row(&format!("SELECT COUNT(*) {FROM}"), [&ids], |r| r.get(0))?;
    let mut stmt = file.conn().prepare(&format!("SELECT u.url, s.id, s.format, s.type_path, s.json, i.validation, i.severity, i.code, i.property, i.feature {FROM} ORDER BY s.url_id, CASE s.format WHEN 'json-ld' THEN 0 WHEN 'microdata' THEN 1 ELSE 2 END, s.id, i.id LIMIT ?2 OFFSET ?3"))?;
    let rows = stmt
        .query_map(
            params![ids, limit.min(i64::MAX as u64), offset.min(i64::MAX as u64)],
            |r| {
                let mut row = Vec::new();
                for n in 0..10 {
                    row.push(if n == 1 {
                        Value::Int(r.get(n)?)
                    } else {
                        r.get::<_, Option<String>>(n)?
                            .map_or(Value::Null, Value::Text)
                    });
                }
                Ok(row)
            },
        )?
        .collect::<Result<_, _>>()?;
    Ok(DetailData {
        columns: [
            "Address",
            "Item ID",
            "Format",
            "Type",
            "JSON",
            "Validation Type",
            "Severity",
            "Issue",
            "Property",
            "Feature",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        rows,
        total,
    })
}
