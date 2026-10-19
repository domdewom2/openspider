//! Security exports with the individual references that caused each finding (OUT-051).
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::links_export::{self, sql};
use crate::types::TableWriter;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Security:HTTP URLs Inlinks",
        aliases: &[],
        file_stem: "Security:HTTP URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
    TableEntry {
        name: "Security:Mixed Content Inlinks",
        aliases: &["Security:Mixed Content"],
        file_stem: "Security:Mixed Content Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
    TableEntry {
        name: "Security:Form URL Insecure Inlinks",
        aliases: &["Security:Form URL Insecure"],
        file_stem: "Security:Form URL Insecure Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
    TableEntry {
        name: "Security:Form On HTTP URL Inlinks",
        aliases: &["Security:Form On HTTP URL"],
        file_stem: "Security:Form On HTTP URL Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
    TableEntry {
        name: "Security:Unsafe Cross-Origin Links Inlinks",
        aliases: &["Security:Unsafe Cross-Origin Links"],
        file_stem: "Security:Unsafe Cross-Origin Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
    TableEntry {
        name: "Security:Protocol-Relative Resource Links Inlinks",
        aliases: &["Security:Protocol-Relative Resource Links"],
        file_stem: "Security:Protocol-Relative Resource Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D11",
        run,
    },
];

const HTML_SOURCE: &str = "s.is_internal=1 AND s.in_tables=1 AND s.content_class='HTML' AND s.status_code BETWEEN 200 AND 299";
const UNSAFE: &str = "l.link_type='Hyperlink' AND t.is_internal=0 AND lower(l.target_attr)='_blank' AND instr(' ' || lower(COALESCE(l.rel,'')) || ' ', ' noopener ')=0 AND instr(' ' || lower(COALESCE(l.rel,'')) || ' ', ' noreferrer ')=0";

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let entries: Vec<_> = ENTRIES.iter().collect();
    let entry = crate::catalog::find_table(&entries, job.entry_name).ok_or_else(|| {
        ExportError::Write(format!("Unknown security export: {}", job.entry_name))
    })?;
    if entry.name.contains("Form ") {
        return forms(job, writer, entry.name.contains("URL Insecure"));
    }
    let condition = if entry.name.contains("HTTP URLs") {
        "s.is_internal=1 AND t.is_internal=1 AND t.in_tables=1 AND t.url LIKE 'http://%'".to_owned()
    } else if entry.name.contains("Unsafe") {
        format!("{HTML_SOURCE} AND {UNSAFE}")
    } else {
        let flag = if entry.name.contains("Mixed") {
            "http"
        } else {
            "protocol_relative"
        };
        let https = if flag == "http" {
            " AND s.url LIKE 'https://%'"
        } else {
            ""
        };
        format!("{HTML_SOURCE}{https} AND EXISTS (SELECT 1 FROM url_ext e, json_each(e.v_text) r WHERE e.url_id=s.id AND e.key='security.resources' AND json_extract(r.value,'$.path')=l.link_path AND json_extract(r.value,'$.raw')=l.target_raw AND json_extract(r.value,'$.{flag}')=1)")
    };
    stream_references(job, writer, &condition, false)
}

fn forms(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    insecure: bool,
) -> Result<u64, ExportError> {
    // The forms table preserves raw actions. Resolved actions and paths travel through security.forms,
    // since they depend on <base> and cannot be reconstructed from the page URL alone.
    let condition = if insecure {
        "json_extract(f.value,'$.action') LIKE 'http://%'"
    } else {
        "s.url LIKE 'http://%'"
    };
    let query = format!("SELECT s.address, json_extract(f.value,'$.action'), json_extract(f.value,'$.raw'), json_extract(f.value,'$.path'), json_extract(f.value,'$.position'), t.size_bytes, CASE WHEN t.state='done' AND t.status_code>0 THEN t.status_code END, COALESCE(t.status,'Not Crawled') FROM url_ext e JOIN urls s ON s.id=e.url_id, json_each(e.v_text) f LEFT JOIN urls t ON t.url=json_extract(f.value,'$.action') WHERE e.key='security.forms' AND {HTML_SOURCE} AND {condition} ORDER BY json_extract(f.value,'$.action') COLLATE OS_TEXT, s.address COLLATE OS_TEXT, CAST(f.key AS INTEGER)");
    let mut stmt = sql(job.file.conn().prepare(&query))?;
    let mut rows = sql(stmt.query([]))?;
    let kinds = links_export::link_kinds();
    let mut count = 0;
    while let Some(row) = sql(rows.next())? {
        let text =
            |i| sql(row.get::<_, Option<String>>(i)).map(|v| v.map_or(Value::Null, Value::Text));
        let action = text(1)?;
        let raw = sql(row.get::<_, Option<String>>(2))?.unwrap_or_default();
        let path_type = if raw.trim().starts_with("//") {
            "Protocol-Relative"
        } else if raw.contains("://") {
            "Absolute"
        } else if raw.starts_with('/') {
            "Root-Relative"
        } else {
            "Path-Relative"
        };
        let bytes = sql(row.get::<_, Option<i64>>(5))?.map_or(Value::Null, Value::Int);
        let status = sql(row.get::<_, Option<i64>>(6))?.map_or(Value::Null, Value::Int);
        writer.row(
            &kinds,
            &[
                Value::from("Form"),
                text(0)?,
                action,
                bytes,
                Value::Null,
                Value::Null,
                status,
                text(7)?,
                Value::from("True"),
                Value::Null,
                Value::Null,
                Value::from(path_type),
                text(3)?,
                text(4)?,
                Value::from("HTML"),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

/// Standard link rows supplemented by security references whose generic transport is disabled.
/// The CTE streams references from url_ext one page at a time; no crawl-wide collection is built.
pub(crate) fn stream_references(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    condition: &str,
    internal: bool,
) -> Result<u64, ExportError> {
    let query = format!(
        r#"WITH refs AS (
        SELECT e.url_id, r.value, CAST(r.key AS INTEGER) AS idx
        FROM url_ext e, json_each(e.v_text) r WHERE e.key='security.references'
    ), security_links AS (
        SELECT id, source_id, target_id, target_raw, link_type, position, alt_text, anchor, follow,
               target_attr, rel, path_type, link_path, link_position, link_origin, NULL AS resolved FROM links
        UNION ALL
        SELECT -r.idx-1, r.url_id, t.id, json_extract(r.value,'$.raw'), json_extract(r.value,'$.type'),
               json_extract(r.value,'$.order'), json_extract(r.value,'$.alt'), NULL,
               json_extract(r.value,'$.follow'), NULL, json_extract(r.value,'$.rel'),
               json_extract(r.value,'$.path_type'), json_extract(r.value,'$.path'),
               json_extract(r.value,'$.position'), 'HTML', json_extract(r.value,'$.resolved')
        FROM refs r LEFT JOIN urls t ON t.url=json_extract(r.value,'$.resolved')
        WHERE NOT EXISTS (SELECT 1 FROM links existing WHERE existing.source_id=r.url_id
            AND existing.link_path=json_extract(r.value,'$.path')
            AND existing.target_raw=json_extract(r.value,'$.raw')
            AND existing.link_type=json_extract(r.value,'$.type'))
        UNION ALL
        SELECT -1000000-h.position, h.url_id, h.target_id, h.href_raw, 'HTTP Hreflang',
               h.position, NULL, NULL, 1, NULL, 'alternate', 'Absolute', NULL, NULL, 'HTML', h.url
        FROM hreflang h WHERE h.source='http'
          AND NOT EXISTS (SELECT 1 FROM links existing WHERE existing.source_id=h.url_id
              AND existing.link_type='HTTP Hreflang' AND existing.target_raw=h.href_raw)
    ) SELECT l.link_type, s.address, COALESCE(t.address,l.resolved,l.target_raw), t.size_bytes, l.alt_text,
             l.anchor, CASE WHEN t.state='done' AND t.status_code>0 THEN t.status_code END,
             CASE WHEN t.state IS NULL OR t.state<>'done' OR t.status_code IS NULL THEN 'Not Crawled' ELSE t.status END,
             CASE WHEN l.follow<>0 THEN 'True' ELSE 'False' END, l.target_attr, l.rel, l.path_type,
             l.link_path, l.link_position, l.link_origin, CASE WHEN t.is_internal=1 THEN 'True' ELSE 'False' END
       FROM security_links l JOIN urls s ON s.id=l.source_id LEFT JOIN urls t ON t.id=l.target_id
       WHERE {condition} ORDER BY COALESCE(t.address,l.resolved,l.target_raw) COLLATE OS_TEXT,
             s.address COLLATE OS_TEXT, l.position, l.id"#
    );
    let mut stmt = sql(job.file.conn().prepare(&query))?;
    let mut rows = sql(stmt.query([]))?;
    let mut kinds = links_export::link_kinds();
    if internal {
        kinds.push(os_analyze::ColumnKind::Text);
    }
    let mut count = 0;
    while let Some(row) = sql(rows.next())? {
        let mut values = Vec::with_capacity(kinds.len());
        for i in 0..kinds.len() {
            values.push(if i == 3 || i == 6 {
                sql(row.get::<_, Option<i64>>(i))?.map_or(Value::Null, Value::Int)
            } else {
                sql(row.get::<_, Option<String>>(i))?.map_or(Value::Null, Value::Text)
            });
        }
        writer.row(&kinds, &values)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Format;
    use os_engine::testing::crawl_fixture_from;
    use os_model::CrawlConfig;

    // spec: OUT-051, ONPAGE-033, ONPAGE-034, ONPAGE-035, ONPAGE-036, ONPAGE-037
    #[tokio::test]
    async fn security_bulk_exports_write_individual_causes_in_standard_format() {
        let mut cfg = CrawlConfig::default();
        cfg.spider.external.crawl = false;
        let fx = crawl_fixture_from("onpage-m2-security", "https://site.test/", cfg.clone()).await;
        let tabs = os_analyze::tabs::all();
        let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/onpage-m2-security/export-causes.json"
        ))
        .unwrap();
        for entry in entries() {
            let job = TableJob {
                entry_name: entry.name,
                tabs: &tabs,
                file: &fx.file,
                cfg: &cfg,
            };
            let path = fx.dir.path().join(entry.file_name("csv"));
            let info = crate::catalog::export_table(entry, &job, &path, Format::Csv).unwrap();
            let mut csv = csv::Reader::from_path(&path).unwrap();
            assert_eq!(
                csv.headers().unwrap().iter().collect::<Vec<_>>(),
                crate::catalog::LINK_HEADER
            );
            let rows: Vec<_> = csv.records().map(Result::unwrap).collect();
            assert_eq!(info.rows, rows.len() as u64);
            for case in expected.iter().filter(|c| c["name"] == entry.name) {
                let suffix = case["source"].as_str().unwrap();
                let mut got: Vec<_> = rows
                    .iter()
                    .filter(|r| r[1].ends_with(suffix))
                    .map(|r| r[2].to_owned())
                    .collect();
                got.sort();
                let want: Vec<_> = case["destinations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_owned())
                    .collect();
                assert_eq!(got, want, "{}", entry.name);
            }
            if entry.name.contains("Mixed") {
                let image = rows
                    .iter()
                    .find(|r| r[1].ends_with("mixed.html") && r[2].ends_with("p.png"))
                    .unwrap();
                assert_eq!(&image[0], "Image");
                assert_eq!(&image[6], ""); // external checking disabled
                assert!(!rows.iter().any(|r| r[1].ends_with("not-resources.html")));
            }
            if entry.name.contains("HTTP URLs") {
                assert!(rows.iter().any(|r| r[2] == *"http://site.test/insecure"));
                assert!(!rows.iter().any(|r| r[2].contains("cdn.test")));
            }
            if entry.name.contains("Form On HTTP") {
                assert_eq!(info.rows, 0); // HTTPS crawl, including forms with HTTP actions
            }
        }
        let fx = crawl_fixture_from("onpage-m2-security", "http://site.test/", cfg.clone()).await;
        let entry = &entries()[3];
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &fx.file,
            cfg: &cfg,
        };
        let path = fx.dir.path().join("forms.csv");
        let info = crate::catalog::export_table(entry, &job, &path, Format::Csv).unwrap();
        assert!(info.rows > 0);
        let mut csv = csv::Reader::from_path(path).unwrap();
        assert!(csv
            .records()
            .map(Result::unwrap)
            .any(|r| r[1].ends_with("form-secure.html") && r[2] == *"https://site.test/send"));
    }
}
