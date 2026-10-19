//! Spalten aus Crawl-Fakten und beobachteten HTTP-Headern.
use crate::defs::{ColumnDef, ColumnKind, ColumnSource};
use os_model::CrawlConfig;

struct CrawlColumns;
impl ColumnSource for CrawlColumns {
    fn columns(&self, _cfg: &CrawlConfig) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new(
                "Links Truncated",
                "(SELECT v_int FROM url_ext WHERE url_id = u.id AND key = 'crawl.links_truncated')",
                ColumnKind::Bool,
                false,
            ),
            ColumnDef::new(
                "Truncated",
                "(SELECT v_int FROM url_ext WHERE url_id = u.id AND key = 'crawl.truncated')",
                ColumnKind::Bool,
                false,
            ),
        ]
    }
    fn columns_for_file(&self, cfg: &CrawlConfig, file: &os_store::CrawlFile) -> Vec<ColumnDef> {
        let mut columns = self.columns(cfg);
        if cfg.extraction.http_headers {
            if let Ok(mut stmt) = file.conn().prepare("SELECT DISTINCT lower(name) FROM http_headers WHERE direction = 'response' ORDER BY lower(name)") {
                if let Ok(names) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                    for name in names.flatten() {
                        let sql_name = name.replace('\'', "''");
                        columns.push(ColumnDef::dynamic(&format!("Header: {name}"), &format!("(SELECT group_concat(value, char(10)) FROM (SELECT value FROM http_headers WHERE url_id = u.id AND direction = 'response' AND lower(name) = '{sql_name}' ORDER BY position))"), ColumnKind::Text, false));
                    }
                }
            }
        }
        columns
    }
}
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(CrawlColumns))
}
