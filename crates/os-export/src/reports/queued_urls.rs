//! Persistierte Warteschlange (OUT-030), in geplanter Abrufreihenfolge.
use super::common::db;
use crate::catalog::{TableEntry, TableJob};
use crate::{error::ExportError, types::TableWriter};
use os_analyze::ColumnKind;
use os_model::Value;

pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Queued URLs",
    aliases: &[],
    file_stem: "Queued URLs",
    header: &["Address", "Crawl Depth", "Found On", "Link Type"],
    header_fn: None,
    requires: &[],
    owner: "M2-D17",
    run,
}];
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let mut stmt = db(job.file.conn().prepare(
        "SELECT u.address, q.depth,
          COALESCE(s.address, (SELECT sm.url FROM sitemap_entries se
            JOIN sitemaps sm ON sm.id=se.sitemap_id WHERE se.url_id=u.id ORDER BY sm.id LIMIT 1)),
          CASE WHEN d.kind='sitemap' THEN 'XML Sitemap' ELSE d.link_type END
         FROM queue q JOIN urls u ON u.id=q.url_id
         LEFT JOIN discovery_edges d ON d.seq=(SELECT MIN(e.seq) FROM discovery_edges e WHERE e.target_id=u.id)
         LEFT JOIN urls s ON s.id=d.source_id
         WHERE u.is_internal=1 AND u.state<>'done' AND u.robots_blocked=0
           AND (u.not_crawled_reason IS NULL OR u.not_crawled_reason IN ('url_limit','stopped'))
         ORDER BY q.priority, q.depth, q.seq, q.url_id",
    ))?;
    let mut rows = db(stmt.query([]))?;
    let mut count = 0;
    while let Some(row) = db(rows.next())? {
        writer.row(
            &[
                ColumnKind::Url,
                ColumnKind::Int,
                ColumnKind::Url,
                ColumnKind::Text,
            ],
            &[
                Value::Text(db(row.get(0))?),
                db(row.get::<_, Option<i64>>(1))?.map_or(Value::Null, Value::Int),
                db(row.get::<_, Option<String>>(2))?.map_or(Value::Null, Value::Text),
                db(row.get::<_, Option<String>>(3))?.map_or(Value::Null, Value::Text),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}
