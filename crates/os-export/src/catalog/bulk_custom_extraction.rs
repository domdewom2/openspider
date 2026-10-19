//! Custom Extraction: streamed long-format values and per-extractor inlinks.
use crate::{
    catalog::{DynamicEntry, TableEntry, TableJob, LINK_HEADER},
    error::ExportError,
    links_export::{self, sql, LinkOrder},
    types::TableWriter,
};
use os_analyze::{defs::ColumnKind, tabs::custom_extraction::extractor_name};
use os_model::Value;

/// Entries in menu order.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}
const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Custom Extraction:All Extractions",
    aliases: &[],
    file_stem: "Custom Extraction:All Extractions",
    header: &["Address", "Extractor", "Index", "Value"],
    header_fn: None,
    requires: &[],
    owner: "M2-D15",
    run: all_extractions,
}];

fn all_extractions(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let mut stmt = sql(job.file.conn().prepare("SELECT u.address, e.def_index, e.number, e.value FROM custom_extraction_values e JOIN urls u ON u.id = e.url_id WHERE u.is_internal = 1 AND u.in_tables = 1 AND u.content_class = 'HTML' AND u.status_code BETWEEN 200 AND 299 ORDER BY u.address, e.def_index, e.number"))?;
    let mut rows = sql(stmt.query([]))?;
    let mut count = 0;
    while let Some(row) = sql(rows.next())? {
        let index: usize = sql(row.get(1))?;
        // Config is persisted with the crawl; stale rows from removed definitions
        // are not mislabeled as a different extractor.
        if index >= job.cfg.custom_extraction.extractors.len() {
            continue;
        }
        writer.row(
            &[
                ColumnKind::Url,
                ColumnKind::Text,
                ColumnKind::Int,
                ColumnKind::Text,
            ],
            &[
                Value::Text(sql(row.get(0))?),
                Value::Text(extractor_name(job.cfg, index)),
                Value::Int(sql(row.get::<_, i64>(2))? + 1),
                Value::Text(sql(row.get(3))?),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Inlinks use the shared standard-link exporter and the tab's non-empty criterion.
pub fn dynamic_entries(cfg: &os_model::CrawlConfig) -> Vec<DynamicEntry> {
    cfg.custom_extraction.extractors.iter().take(100).enumerate().map(|(i, _)| {
        let name = format!("Custom Extraction:{} Inlinks", extractor_name(cfg, i));
        DynamicEntry { file_stem: name.clone(), name, header: LINK_HEADER.iter().map(|s| (*s).into()).collect(), run: Box::new(move |job, writer| {
            links_export::stream_links(job.file,
                &format!("t.is_internal = 1 AND t.in_tables = 1 AND t.content_class = 'HTML' AND t.status_code BETWEEN 200 AND 299 AND EXISTS (SELECT 1 FROM custom_extraction_values e WHERE e.url_id = t.id AND e.def_index = {i} AND e.value <> '')"),
                &[], LinkOrder::Inlinks, &links_export::link_kinds(), writer)
        }) }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{export_bulk, pipeline::export_view, Format};
    use calamine::{Data, Reader};
    use os_analyze::{tabs, views::ViewQuery};
    mod fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/integ-custom-extraction/store.rs"
        ));
    }

    fn csv(path: &std::path::Path) -> (Vec<String>, Vec<Vec<String>>) {
        let content = std::fs::read_to_string(path).unwrap();
        let mut reader =
            csv::Reader::from_reader(content.trim_start_matches('\u{feff}').as_bytes());
        let header = reader
            .headers()
            .unwrap()
            .iter()
            .map(str::to_owned)
            .collect();
        let rows = reader
            .records()
            .map(|r| r.unwrap().iter().map(str::to_owned).collect())
            .collect();
        (header, rows)
    }

    // spec: OUT-061
    #[test]
    fn long_format_and_extractor_inlinks_use_saved_names_and_one_based_indices() {
        let dir = tempfile::tempdir().unwrap();
        let file = fixture::create(&dir.path().join("extract.ospider"));
        let cfg = file.config().unwrap();
        let out = dir.path().join("out");
        let files = export_bulk(
            &file,
            &cfg,
            "Custom Extraction:All Extractions",
            &out,
            Format::Csv,
        )
        .unwrap();
        let (header, rows) = csv(&files[0].path);
        assert_eq!(header, ["Address", "Extractor", "Index", "Value"]);
        let mut golden = csv::Reader::from_reader(
            include_bytes!(
                "../../../../tests/fixtures/integ-custom-extraction/expected-extractions.csv"
            )
            .as_slice(),
        );
        let expected: Vec<Vec<String>> = golden
            .records()
            .map(|r| r.unwrap().iter().map(str::to_owned).collect())
            .collect();
        assert_eq!(rows, expected);
        assert_eq!(rows[0], ["https://extract.test/a", "h2", "1", "First"]);
        assert_eq!(rows[2], ["https://extract.test/a", "h2", "3", "Third"]);
        assert_eq!(rows[3][1], "Extractor 2");
        assert_eq!(rows[3][3], "line one\nline two, \"quoted\"");
        let files = export_bulk(
            &file,
            &cfg,
            "Custom Extraction:Extractor 2 Inlinks",
            &out,
            Format::Csv,
        )
        .unwrap();
        let (header, rows) = csv(&files[0].path);
        assert_eq!(header, LINK_HEADER);
        assert_eq!(rows.len(), 2); // Both link instances to A, no link to B.
        assert!(rows.iter().all(|r| r[2] == "https://extract.test/a"));
    }

    // spec: INTEG-053
    #[test]
    fn tab_csv_filter_quoting_and_xlsx_cell_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = fixture::create(&dir.path().join("extract.ospider"));
        let cfg = file.config().unwrap();
        let tabs = tabs::configured(&cfg);
        let q = ViewQuery::new("Custom Extraction", "Extractor 2");
        let path = dir.path().join("tab.csv");
        export_view(&tabs, &file, &cfg, &q, &path, Format::Csv, "Extraction").unwrap();
        let (header, rows) = csv(&path);
        assert_eq!(
            &header[..4],
            ["Address", "Content", "Status Code", "Status"]
        );
        assert!(!header.iter().any(|h| h.starts_with("h2")));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][4], "line one\nline two, \"quoted\"");
        let long = "x".repeat(40_000);
        file.conn()
            .execute(
                "UPDATE custom_extraction_values SET value=?1 WHERE def_index=1",
                [&long],
            )
            .unwrap();
        export_view(&tabs, &file, &cfg, &q, &path, Format::Csv, "Extraction").unwrap();
        assert_eq!(csv(&path).1[0][4].len(), 40_000);
        let path = dir.path().join("tab.xlsx");
        export_view(&tabs, &file, &cfg, &q, &path, Format::Xlsx, "Extraction").unwrap();
        let mut workbook: calamine::Xlsx<_> = calamine::open_workbook(&path).unwrap();
        let range = workbook.worksheet_range("Extraction").unwrap();
        let Some(Data::String(value)) = range.get((1, 4)) else {
            panic!("text cell");
        };
        assert_eq!(value.chars().count(), 32_767);
        assert!(value.starts_with(&"x".repeat(32_750)));
    }
}
