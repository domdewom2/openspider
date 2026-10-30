//! Per-filter URL and standard-format Inlinks exports (OUT-060, INTEG-034).

use os_analyze::{
    defs::ColumnKind,
    tabs::custom_search::{filter_name, predicate},
};
use os_model::{CrawlConfig, Value};

use crate::{
    catalog::{DynamicEntry, TableEntry, TableJob},
    error::ExportError,
    links_export::{self, sql, LinkOrder},
    types::TableWriter,
};

const HEADER: &[&str] = &[
    "Address",
    "Filter",
    "Occurrences",
    "Status Code",
    "Indexability",
];

/// Static aggregate entry retained for existing callers.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Custom Search:All Filters",
    aliases: &[],
    file_stem: "Custom Search:All Filters",
    header: HEADER,
    requires: &[],
    owner: "M2-D14",
    run: all_filters,
    header_fn: None,
}];

fn all_filters(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let mut count = 0;
    for (i, f) in job.cfg.custom_search.filters.iter().enumerate() {
        count += write_filter(job, writer, i, &filter_name(f, i), f.contains)?;
    }
    Ok(count)
}

fn write_filter(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    index: usize,
    name: &str,
    contains: bool,
) -> Result<u64, ExportError> {
    let query = format!("SELECT u.address,h.count,u.status_code,u.indexability FROM urls u JOIN custom_search_hits h ON h.url_id=u.id AND h.def_index={index} WHERE u.is_internal=1 AND {} ORDER BY u.id", predicate(index, contains));
    let mut stmt = sql(job.file.conn().prepare(&query))?;
    let mut rows = sql(stmt.query([]))?;
    let kinds = [
        ColumnKind::Url,
        ColumnKind::Text,
        ColumnKind::Int,
        ColumnKind::Int,
        ColumnKind::Text,
    ];
    let mut count = 0;
    while let Some(row) = sql(rows.next())? {
        writer.row(
            &kinds,
            &[
                Value::Text(sql(row.get(0))?),
                Value::Text(name.to_owned()),
                Value::Int(sql(row.get(1))?),
                sql(row.get::<_, Option<i64>>(2))?.map_or(Value::Null, Value::Int),
                sql(row.get::<_, Option<String>>(3))?.map_or(Value::Null, Value::Text),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Two entries per configured filter, preserving profile order.
pub fn dynamic_entries(cfg: &CrawlConfig) -> Vec<DynamicEntry> {
    let mut entries = Vec::new();
    for (i, filter) in cfg.custom_search.filters.iter().enumerate() {
        let name = filter_name(filter, i);
        let generated_index = filter.name.trim().is_empty().then_some(i + 1);
        let contains = filter.contains;
        let export_name = format!("Custom Search:{name}");
        let url_name = name.clone();
        entries.push(DynamicEntry {
            file_stem: export_name.clone(),
            name: export_name,
            label_key: Some(super::custom_label_key(
                "search",
                &name,
                false,
                generated_index,
            )),
            header: HEADER.iter().map(|s| (*s).to_owned()).collect(),
            run: Box::new(move |job, writer| write_filter(job, writer, i, &url_name, contains)),
        });
        let export_name = format!("Custom Search:{name} Inlinks");
        entries.push(DynamicEntry {
            file_stem: export_name.clone(),
            name: export_name,
            label_key: Some(super::custom_label_key(
                "search",
                &name,
                true,
                generated_index,
            )),
            header: links_export::link_header(),
            run: Box::new(move |job, writer| {
                let condition = format!(
                    "l.target_id IN (SELECT u.id FROM urls u WHERE u.is_internal=1 AND {})",
                    predicate(i, contains)
                );
                links_export::stream_links(
                    job.file,
                    &condition,
                    &[],
                    LinkOrder::Inlinks,
                    &links_export::link_kinds(),
                    writer,
                )
            }),
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pipeline, types::Format};
    use os_analyze::views::ViewQuery;
    use os_engine::testing::crawl_fixture;

    // spec: UI-121
    // spec: OUT-060
    #[test]
    fn display_metadata_preserves_user_names_and_marks_generated_filter_indices() {
        let mut cfg = CrawlConfig {
            custom_search: serde_json::from_str(include_str!(
                "../../../../tests/fixtures/integ-custom-search/filters.json"
            ))
            .unwrap(),
            ..Default::default()
        };
        let names = ["Screenshots", "Extractor 2", "Stock Inlinks", "  "];
        for (filter, name) in cfg.custom_search.filters.iter_mut().zip(names) {
            filter.name = name.into();
        }
        let entries = dynamic_entries(&cfg);
        assert_eq!(entries.len(), 8);
        for (i, name) in ["Screenshots", "Extractor 2", "Stock Inlinks", "Filter 4"]
            .iter()
            .enumerate()
        {
            for (offset, inlinks) in [(0, false), (1, true)] {
                let entry = &entries[i * 2 + offset];
                let canonical = format!(
                    "Custom Search:{name}{}",
                    if inlinks { " Inlinks" } else { "" }
                );
                assert_eq!(entry.name, canonical);
                assert_eq!(entry.file_stem, canonical);
                assert_eq!(
                    entry.header,
                    if inlinks {
                        links_export::link_header()
                    } else {
                        HEADER.iter().map(|s| (*s).to_owned()).collect()
                    }
                );
                let metadata: serde_json::Value = serde_json::from_str(
                    entry
                        .label_key
                        .as_ref()
                        .unwrap()
                        .strip_prefix("custom-export:")
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(
                    metadata,
                    serde_json::json!({
                        "group": "search", "name": name, "inlinks": inlinks,
                        "generatedIndex": if i == 3 { Some(4) } else { None },
                    })
                );
            }
        }
    }

    // spec: OUT-060
    // spec: INTEG-034
    // spec: INTEG-031
    #[tokio::test]
    async fn fixture_exports_match_filter_counts_and_standard_inlinks() {
        let cfg = CrawlConfig {
            custom_search: serde_json::from_str(include_str!(
                "../../../../tests/fixtures/integ-custom-search/filters.json"
            ))
            .unwrap(),
            ..Default::default()
        };
        let fx = crawl_fixture("integ-custom-search", cfg.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let files = pipeline::export_bulk(
            &fx.file,
            &cfg,
            "Custom Search:Stock",
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        assert_eq!(files[0].rows, 2);
        assert!(files[0]
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("stock"));
        let csv = std::fs::read_to_string(&files[0].path).unwrap();
        assert_eq!(
            csv.trim_start_matches('\u{feff}')
                .replace("\r\n", "\n")
                .replace(&fx.site.base_url(), "{BASE}"),
            include_str!("../../../../tests/fixtures/integ-custom-search/expected-stock.csv")
        );
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        assert_eq!(reader.headers().unwrap().iter().collect::<Vec<_>>(), HEADER);
        let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
        assert_eq!(rows.iter().map(|r| &r[2]).collect::<Vec<_>>(), ["3", "1"]);
        assert!(rows.iter().all(|r| &r[1] == "Stock"));
        for (name, expected) in [("Stock", 1), ("Missing stock", 4)] {
            let files = pipeline::export_bulk(
                &fx.file,
                &cfg,
                &format!("Custom Search:{name} Inlinks"),
                dir.path(),
                Format::Csv,
            )
            .unwrap();
            assert_eq!(files[0].rows, expected);
            let csv = std::fs::read_to_string(&files[0].path).unwrap();
            let mut reader = csv::Reader::from_reader(csv.as_bytes());
            assert_eq!(
                reader.headers().unwrap().iter().collect::<Vec<_>>(),
                links_export::LINK_COLUMNS
            );
            for row in reader.records() {
                let row = row.unwrap();
                assert!(if name == "Stock" {
                    row[2].ends_with("/missing")
                } else {
                    row[2].ends_with("/empty") || row[2].ends_with("/redirect")
                });
            }
        }
        let tabs = os_analyze::tabs::configured(&cfg);
        let path = dir.path().join("tab.csv");
        let info = pipeline::export_view(
            &tabs,
            &fx.file,
            &cfg,
            &ViewQuery::new("Custom Search", "Stock"),
            &path,
            Format::Csv,
            "Search",
        )
        .unwrap();
        assert_eq!(info.rows, 2);
        let csv = std::fs::read_to_string(path).unwrap();
        assert!(csv.trim_start_matches('\u{feff}').starts_with(
            "Address,Content,Status Code,Status,Stock,Missing stock,Redirect,Assumed\r\n"
        ));
        let negative = pipeline::export_bulk(
            &fx.file,
            &cfg,
            "Custom Search:Missing stock",
            dir.path(),
            Format::Csv,
        )
        .unwrap();
        assert_eq!(negative[0].rows, 2);
        let csv = std::fs::read_to_string(&negative[0].path).unwrap();
        assert!(csv::Reader::from_reader(csv.as_bytes())
            .records()
            .all(|r| &r.unwrap()[2] == "0"));
    }
}
