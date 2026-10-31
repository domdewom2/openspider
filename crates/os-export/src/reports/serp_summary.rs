//! SERP Summary (OUT-098).
use crate::catalog::{TableEntry, TableJob};
use crate::{error::ExportError, types::TableWriter};
use os_analyze::ColumnKind;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "SERP Summary",
    aliases: &[],
    file_stem: "SERP Summary",
    header: &[
        "Address",
        "Title 1",
        "Title 1 Length",
        "Title 1 Pixel Width",
        "Meta Description 1",
        "Meta Description 1 Length",
        "Meta Description 1 Pixel Width",
    ],
    requires: &[],
    owner: "M2-D01",
    run,
    header_fn: None,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let mut stmt = job
        .file
        .conn()
        .prepare(
            "SELECT address, COALESCE(title_1, ''), COALESCE(title_1_len, 0),
                COALESCE(title_1_px, 0), COALESCE(meta_desc_1, ''),
                COALESCE(meta_desc_1_len, 0), COALESCE(meta_desc_1_px, 0)
         FROM urls
         WHERE is_internal = 1 AND in_tables = 1 AND content_class = 'HTML' AND status_code BETWEEN 200 AND 299
         ORDER BY id",
        )
        .map_err(|e| ExportError::Write(e.to_string()))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| ExportError::Write(e.to_string()))?;
    let mut count = 0;
    while let Some(r) = rows.next().map_err(|e| ExportError::Write(e.to_string()))? {
        let values = (0..7)
            .map(|i| {
                if matches!(i, 0 | 1 | 4) {
                    r.get::<_, String>(i).map(Value::Text)
                } else {
                    r.get::<_, i64>(i).map(Value::Int)
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ExportError::Write(e.to_string()))?;
        writer.row(
            &[
                ColumnKind::Text,
                ColumnKind::Text,
                ColumnKind::Int,
                ColumnKind::Int,
                ColumnKind::Text,
                ColumnKind::Int,
                ColumnKind::Int,
            ],
            &values,
        )?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use os_model::{
        time::{IdSource, SeededIds},
        CrawlConfig, CrawlMode, Timestamp,
    };
    use os_store::{CrawlFile, CrawlInit, StorageProfile};
    // spec: OUT-098
    #[test]
    fn report_streams_original_values_including_unicode() {
        let dir = tempfile::tempdir().unwrap();
        let created = Timestamp::from_db("2026-10-17T12:00:00Z").unwrap();
        let cfg = CrawlConfig::default();
        let init = CrawlInit {
            crawl_id: SeededIds::new(19).new_crawl_id(created),
            mode: CrawlMode::Spider,
            start_url: None,
            list_source: None,
            project: "test".into(),
            name: "serp".into(),
            config: cfg.clone(),
            created_at: created,
        };
        let file = CrawlFile::create(
            &dir.path().join("serp.ospider"),
            &init,
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/onpage-m2-elements/store.sql"
            ))
            .unwrap();
        file.conn().execute("UPDATE urls SET title_1='Über 東京',title_1_len=7,title_1_px=77,meta_desc_1='Beschreibung',meta_desc_1_len=12 WHERE id=1",[]).unwrap();
        file.conn()
            .execute_batch(
                "UPDATE urls SET is_internal=0 WHERE id=3;
             UPDATE urls SET content_class='PDF' WHERE id=4;
             UPDATE urls SET in_tables=0 WHERE id=5;
             INSERT INTO urls (url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,content_class,in_tables) VALUES
             ('https://site.test/error','https://site.test/error','https://site.test',1,'done','link',0,LENGTH('https://site.test/error'),404,'HTML',1),
             ('https://site.test/redirect','https://site.test/redirect','https://site.test',1,'done','link',0,LENGTH('https://site.test/redirect'),301,'HTML',1);",
            )
            .unwrap();
        os_store::queries::serp::save(
            file.conn(),
            1,
            &os_store::queries::serp::SerpEdit {
                device: "mobile".into(),
                title: Some("Preview edit".into()),
                description: None,
                edited_at: "2026-10-17T12:00:00Z".into(),
            },
        )
        .unwrap();
        let result = crate::reports::save_report(
            &file,
            &cfg,
            "SERP Summary",
            &dir.path().join("out"),
            crate::types::Format::Csv,
        )
        .unwrap();
        assert_eq!(result.files[0].rows, 2);
        let mut csv = csv::Reader::from_path(&result.files[0].path).unwrap();
        assert_eq!(
            csv.headers().unwrap().iter().collect::<Vec<_>>(),
            super::ENTRIES[0].header
        );
        let rows = csv.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            rows[0].iter().collect::<Vec<_>>(),
            [
                "https://site.test/1",
                "Über 東京",
                "7",
                "77",
                "Beschreibung",
                "12",
                "985"
            ]
        );
        // All six element values are exactly the tab values, including empty fields.
        for (tab, fields, report_offset) in [
            (
                "Page Titles",
                ["Title 1", "Title 1 Length", "Title 1 Pixel Width"],
                1,
            ),
            (
                "Meta Description",
                [
                    "Meta Description 1",
                    "Meta Description 1 Length",
                    "Meta Description 1 Pixel Width",
                ],
                4,
            ),
        ] {
            let view = os_analyze::views::query_view(
                &file,
                &cfg,
                &os_analyze::views::ViewQuery::new(tab, "All"),
            )
            .unwrap();
            for (row, record) in view.rows.iter().zip(&rows) {
                for (i, field) in fields.iter().enumerate() {
                    let col = view.columns.iter().position(|c| c == field).unwrap();
                    let expected = match &row[col] {
                        os_model::Value::Text(v) => v.clone(),
                        os_model::Value::Int(v) => v.to_string(),
                        os_model::Value::Null => String::new(),
                        value => panic!("Unexpected value: {value:?}"),
                    };
                    assert_eq!(&record[report_offset + i], expected);
                }
            }
        }
    }
}
