//! Auswahlbezogene Detailquellen (OUT-010).
use crate::{ColumnKind, SortKey, ViewError};
use os_model::{CrawlConfig, UrlId, Value};
use os_store::CrawlFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailKind {
    Headers,
    Cookies,
    ConsoleLog,
    BlockedResources,
    Network,
    Spelling,
    A11yRules,
    A11yNodes,
    Duplicates,
    NGrams,
    StructuredDataItems,
    StructuredDataIssues,
    RedirectChain,
    SitemapMembership,
    CustomSearch,
    CustomExtraction,
    CustomJs,
    ApiValues,
    Forms,
    Hreflang,
    CrawlPath,
    Anchors,
    ImageInstances,
}
pub struct DetailReq<'a> {
    pub url_ids: &'a [UrlId],
    pub params: &'a serde_json::Value,
    pub sort: &'a [SortKey],
    pub offset: u64,
    pub limit: u64,
}
pub struct DetailRows {
    pub columns: Vec<(String, ColumnKind)>,
    pub rows: Vec<Vec<Value>>,
    pub total: u64,
    pub notes: Vec<(String, String)>,
}
pub type DetailFn = fn(&CrawlFile, &CrawlConfig, &DetailReq<'_>) -> Result<DetailRows, ViewError>;

macro_rules! store_source {
    ($($name:ident => $module:ident),* $(,)?) => {$(
        fn $name(file: &CrawlFile, _: &CrawlConfig, req: &DetailReq<'_>) -> Result<DetailRows, ViewError> {
            let data = os_store::queries::$module::detail_rows(file, req.url_ids, req.offset, req.limit)
                .map_err(|e| match e { os_store::StoreError::NotImplemented(owner) => ViewError::NotImplemented(owner), other => ViewError::Store(other) })?;
            Ok(DetailRows { columns: data.columns.into_iter().map(|n| (n, ColumnKind::Text)).collect(), rows: data.rows, total: data.total, notes: Vec::new() })
        }
    )*};
}
store_source! { anchors => anchors, path => crawl_path, sd => structured_data }

macro_rules! table_source {
    ($($name:ident => $table:literal),* $(,)?) => {$(
        fn $name(file: &CrawlFile, _: &CrawlConfig, req: &DetailReq<'_>) -> Result<DetailRows, ViewError> {
            table_rows(file, req, $table)
        }
    )*};
}
table_source! { headers => "http_headers", cookies => "cookies", console => "render_console",
blocked => "render_blocked", network => "render_network", forms => "forms", sitemaps => "sitemap_entries", spelling => "spelling_findings", a11y_rules => "a11y_rules",
a11y_nodes => "a11y_nodes", search => "custom_search_hits", extraction => "custom_extraction_values",
js => "custom_js_values", api => "api_values" }

/// Dispatch und Besitzer bleiben auch bei noch nicht implementierten Quellen abrufbar.
pub fn detail_source(kind: DetailKind) -> (&'static str, DetailFn) {
    if let Some(source) = crate::tabs::links::detail_source(kind) {
        return source;
    }
    use DetailKind::*;
    match kind {
        Headers => ("M2-E01", headers),
        Cookies => ("M2-E01", cookies),
        ConsoleLog => ("M2-E05", console),
        BlockedResources => ("M2-E05", blocked),
        Network => ("M2-E05", network),
        Spelling => ("M2-D19", spelling),
        A11yRules => ("M2-D21", a11y_rules),
        A11yNodes => ("M2-D21", a11y_nodes),
        Duplicates => ("M2-D02", crate::tabs::duplicates::detail_rows),
        NGrams => ("M2-D02", crate::tabs::duplicates::ngram_details),
        StructuredDataItems | StructuredDataIssues => ("M2-D09", sd),
        RedirectChain => ("M2-D12", crate::tabs::response_codes::redirect_details),
        SitemapMembership => ("M2-D18", sitemaps),
        CustomSearch => ("M2-D14", search),
        CustomExtraction => ("M2-D15", extraction),
        CustomJs => ("M2-E07", js),
        ApiValues => ("M2-I06", api),
        Forms => ("M2-E01", forms),
        Hreflang => ("M2-D07", crate::tabs::hreflang::detail_rows),
        CrawlPath => ("M2-D17", path),
        Anchors => ("M2-D08", anchors),
        ImageInstances => ("M2-D03", crate::tabs::images::detail_rows),
    }
}

fn table_rows(file: &CrawlFile, req: &DetailReq<'_>, table: &str) -> Result<DetailRows, ViewError> {
    let ids = serde_json::to_string(&req.url_ids.iter().map(|id| id.0).collect::<Vec<_>>())
        .expect("integer list");
    let mut schema = file
        .conn()
        .prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = schema
        .query_map([], |r| {
            let name: String = r.get(1)?;
            let kind: String = r.get(2)?;
            Ok((
                name,
                match kind.as_str() {
                    "INTEGER" => ColumnKind::Int,
                    "REAL" => ColumnKind::Float { decimals: 3 },
                    _ => ColumnKind::Text,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let quote = |name: &str| format!("\"{}\"", name.replace('"', "\"\""));
    let mut order = Vec::new();
    for key in req.sort {
        if let Some((name, kind)) = columns.iter().find(|(n, _)| n == &key.column) {
            let name = quote(name);
            let direction = if key.descending { "DESC" } else { "ASC" };
            order.push(format!("({name} IS NULL) ASC"));
            order.push(format!(
                "{name} {} {direction}",
                if matches!(kind, ColumnKind::Text) {
                    "COLLATE OS_TEXT"
                } else {
                    ""
                }
            ));
        }
    }
    order.extend(columns.iter().map(|(name, _)| quote(name)));
    let where_sql = "url_id IN (SELECT value FROM json_each(?1))";
    let total = file.conn().query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {where_sql}"),
        [&ids],
        |r| r.get(0),
    )?;
    let mut stmt = file.conn().prepare(&format!(
        "SELECT * FROM {table} WHERE {where_sql} ORDER BY {} LIMIT ?2 OFFSET ?3",
        order.join(",")
    ))?;
    let rows = stmt
        .query_map(
            rusqlite::params![ids, req.limit.min(500), req.offset.min(i64::MAX as u64)],
            |r| {
                (0..columns.len())
                    .map(|i| {
                        Ok(match r.get_ref(i)? {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(v) => Value::Int(v),
                            rusqlite::types::ValueRef::Real(v) => Value::Float(v),
                            rusqlite::types::ValueRef::Text(v) => {
                                Value::Text(String::from_utf8_lossy(v).into_owned())
                            }
                            rusqlite::types::ValueRef::Blob(_) => Value::Null,
                        })
                    })
                    .collect::<Result<Vec<_>, rusqlite::Error>>()
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DetailRows {
        columns,
        rows,
        total,
        notes: Vec::new(),
    })
}

pub fn url_details(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    url: UrlId,
) -> Result<Vec<(String, String)>, ViewError> {
    let mut rows = crate::tabs::directives::url_details(file, cfg, url)?;
    rows.extend(crate::tabs::hreflang::url_details(file, cfg, url)?);
    rows.extend(crate::tabs::links::url_details(file, cfg, url)?);
    Ok(rows)
}
