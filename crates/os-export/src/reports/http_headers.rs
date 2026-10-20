//! HTTP Header Summary (OUT-110).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};
use os_analyze::ColumnKind;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "HTTP Header Summary",
    aliases: &[],
    file_stem: "HTTP Header Summary",
    header: &[
        "Header Name",
        "URLs",
        "% of Total",
        "Distinct Values",
        "Most Common Value",
    ],
    requires: &[ReportRequirement::Config {
        key: "extraction.http_headers",
        what: "HTTP header extraction",
    }],
    owner: "M2-D18",
    run,
    header_fn: None,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if !job.cfg.extraction.http_headers {
        return Ok(0);
    }
    // Repeated instances count once for URL coverage. Ties use binary value order.
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(
        "WITH headers AS (SELECT LOWER(h.name) AS name,h.value,h.url_id FROM http_headers h JOIN urls u ON u.id=h.url_id WHERE h.direction='response' AND u.in_tables=1 AND u.state='done'),
         frequencies AS (SELECT name,value,COUNT(*) AS n FROM headers GROUP BY name,value),
         ranked AS (SELECT name,value,ROW_NUMBER() OVER (PARTITION BY name ORDER BY n DESC,value COLLATE BINARY) AS rank FROM frequencies),
         totals AS (SELECT name,COUNT(DISTINCT url_id) AS urls,COUNT(DISTINCT value) AS vals FROM headers GROUP BY name)
         SELECT t.name,t.urls,ROUND(100.0*t.urls/NULLIF((SELECT COUNT(*) FROM urls WHERE in_tables=1 AND state='done'),0),2),t.vals,r.value FROM totals t JOIN ranked r ON r.name=t.name AND r.rank=1 ORDER BY t.name COLLATE BINARY",
    ))?;
    let mut rows = crate::links_export::sql(stmt.query([]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        writer.row(
            &[
                ColumnKind::Text,
                ColumnKind::Int,
                ColumnKind::Float { decimals: 2 },
                ColumnKind::Int,
                ColumnKind::Text,
            ],
            &[
                Value::Text(crate::links_export::sql(row.get(0))?),
                Value::Int(crate::links_export::sql(row.get(1))?),
                Value::Float(crate::links_export::sql(row.get(2))?),
                Value::Int(crate::links_export::sql(row.get(3))?),
                Value::Text(crate::links_export::sql(row.get(4))?),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}
