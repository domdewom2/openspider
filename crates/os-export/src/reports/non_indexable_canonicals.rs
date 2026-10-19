//! Nicht indexierbare und unverlinkte Canonical-Ziele (OUT-087).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::links_export::sql;
use crate::types::TableWriter;
use os_analyze::ColumnKind;
use os_model::Value;

pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Canonicals:Non-Indexable Canonicals",
    aliases: &[],
    file_stem: "Canonicals:Non-Indexable Canonicals",
    header: &[
        "Source",
        "Canonical",
        "Status Code",
        "Status",
        "Indexability",
        "Indexability Status",
        "Unlinked",
    ],
    requires: &[ReportRequirement::CrawlAnalysis],
    owner: "M2-D04",
    run,
    header_fn: None,
}];
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if !job.cfg.spider.canonicals.store {
        return Ok(0);
    }
    let mut stmt = sql(job.file.conn().prepare(
        "SELECT u.address, t.address, t.status_code, t.status, t.indexability, t.indexability_status,
            EXISTS(SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'unlinked_canonical')
        FROM urls u JOIN urls t ON t.id = u.canonical_target_id
        WHERE u.is_internal = 1 AND u.in_tables = 1 AND u.status_code BETWEEN 200 AND 299
            AND u.canonical_self = 0 AND t.indexability IS NOT NULL
            AND (t.indexability = 'Non-Indexable' OR t.status_code <> 200 OR
              EXISTS(SELECT 1 FROM url_flags f WHERE f.url_id = u.id AND f.flag = 'unlinked_canonical'))
        ORDER BY u.address COLLATE OS_TEXT"))?;
    let mut rows = sql(stmt.query([]))?;
    let mut count = 0;
    let mut kinds = vec![ColumnKind::Text; 7];
    kinds[2] = ColumnKind::Int;
    while let Some(row) = sql(rows.next())? {
        let mut values = Vec::with_capacity(7);
        for i in 0..6 {
            values.push(if i == 2 {
                sql(row.get::<_, Option<i64>>(i))?
                    .filter(|n| *n > 0)
                    .map_or(Value::Null, Value::Int)
            } else {
                sql(row.get::<_, Option<String>>(i))?.map_or(Value::Null, Value::Text)
            });
        }
        values.push(Value::Text(
            if sql(row.get::<_, bool>(6))? {
                "True"
            } else {
                "False"
            }
            .into(),
        ));
        writer.row(&kinds, &values)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::canonical_chains::tests::{analyse, fixture, read};
    // spec: TECH-025
    // spec: OUT-087
    #[test]
    fn report_lists_error_noindex_and_unlinked_targets_without_unknowns_or_self_references() {
        let fx = fixture();
        analyse(&fx);
        let result = crate::save_report(
            &fx.file,
            &fx.cfg,
            entries()[0].name,
            &fx.dir.path().join("out"),
            crate::Format::Csv,
        )
        .unwrap();
        let table = read(&result.files[0].path);
        assert_eq!(table.header, entries()[0].header);
        for (source, status, unlinked) in [
            ("to-error", "Client Error", "True"),
            ("to-noindex", "noindex", "True"),
            ("annotation", "", "True"),
        ] {
            let row = table
                .rows
                .iter()
                .find(|r| r[0].ends_with(&format!("/{source}")))
                .unwrap();
            assert_eq!(table.cell(row, "Indexability Status"), status);
            assert_eq!(table.cell(row, "Unlinked"), unlinked);
        }
        // Bereits bekannte Ziele bleiben auch ohne zusätzliches Canonical-Crawling bewertbar.
        let mut cfg = fx.cfg.clone();
        cfg.spider.canonicals.crawl = false;
        let known = crate::save_report(
            &fx.file,
            &cfg,
            entries()[0].name,
            &fx.dir.path().join("known"),
            crate::Format::Csv,
        )
        .unwrap();
        assert_eq!(read(&known.files[0].path).rows, table.rows);
        for source in ["c", "linked-source", "to-unknown"] {
            assert!(!table
                .rows
                .iter()
                .any(|r| r[0].ends_with(&format!("/{source}"))));
        }
    }
}
