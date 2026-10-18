//! Sitemap-Zuordnungen je Filter (OUT-057, TECH-219).
//!
//! Kanonische Namen ohne „Details“; Dateistämme bleiben aus Kompatibilitätsgründen bestehen und
//! unterscheiden Bulk-Dateien von den gleichnamigen Tab-Exporten (OUT-008).

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::reports::common::db;
use crate::types::TableWriter;
use os_analyze::{views, ColumnKind};
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Sitemaps:URLs In Sitemap",
        aliases: &[
            "Sitemaps:URLs In Sitemap Inlinks",
            "Sitemaps:URLs In Sitemap Details",
        ],
        file_stem: "Sitemaps:URLs In Sitemap Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[
            ReportRequirement::SitemapProcessed,
            ReportRequirement::CrawlAnalysis,
        ],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:URLs Not In Sitemap",
        aliases: &[
            "Sitemaps:URLs Not In Sitemap Inlinks",
            "Sitemaps:URLs Not In Sitemap Details",
        ],
        file_stem: "Sitemaps:URLs Not In Sitemap Details",
        header: &["Address"],
        header_fn: None,
        requires: &[
            ReportRequirement::SitemapProcessed,
            ReportRequirement::CrawlAnalysis,
        ],
        owner: "M2-D13",
        run,
    },
    TableEntry {
        name: "Sitemaps:Non-Indexable URLs In Sitemap",
        aliases: &[
            "Sitemaps:Non-Indexable URLs In Sitemap Inlinks",
            "Sitemaps:Non-Indexable URLs In Sitemap Details",
        ],
        file_stem: "Sitemaps:Non-Indexable URLs In Sitemap Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[
            ReportRequirement::SitemapProcessed,
            ReportRequirement::CrawlAnalysis,
        ],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:URLs In Multiple Sitemaps",
        aliases: &[
            "Sitemaps:URLs In Multiple Sitemaps Inlinks",
            "Sitemaps:URLs In Multiple Sitemaps Details",
        ],
        file_stem: "Sitemaps:URLs In Multiple Sitemaps Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[
            ReportRequirement::SitemapProcessed,
            ReportRequirement::CrawlAnalysis,
        ],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:XML Sitemap With Over 50k URLs",
        aliases: &[
            "Sitemaps:XML Sitemap With Over 50k URLs Inlinks",
            "Sitemaps:XML Sitemap With Over 50k URLs Details",
        ],
        file_stem: "Sitemaps:XML Sitemap With Over 50k URLs Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[ReportRequirement::SitemapProcessed],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:XML Sitemap Over 50MB",
        aliases: &[
            "Sitemaps:XML Sitemap Over 50MB Inlinks",
            "Sitemaps:XML Sitemap Over 50MB Details",
        ],
        file_stem: "Sitemaps:XML Sitemap Over 50MB Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
        ],
        requires: &[ReportRequirement::SitemapProcessed],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Sitemaps:Orphan URLs",
        aliases: &[
            "Sitemaps:Orphan URLs Inlinks",
            "Sitemaps:Orphan URLs Details",
        ],
        file_stem: "Sitemaps:Orphan URLs Details",
        header: &[
            "Sitemap",
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
            "Found Via",
        ],
        requires: &[
            ReportRequirement::SitemapProcessed,
            ReportRequirement::CrawlAnalysis,
        ],
        owner: "M2-D13",
        run,
        header_fn: None,
    },
];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let all: Vec<_> = entries().iter().collect();
    let entry = crate::catalog::find_table(&all, job.entry_name)
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?;
    let filter = entry.name.strip_prefix("Sitemaps:").unwrap_or(entry.name);
    let filter = if filter == "XML Sitemap Over 50MB" {
        "XML Sitemap With Over 50MB"
    } else {
        filter
    };
    write_filter(job, writer, filter, false)
}

/// Gemeinsame Auswahl für Bulk-Export und den Sitemap-Anteil des Orphan-Pages-Reports.
pub(crate) fn write_filter(
    job: &TableJob<'_>,
    writer: &mut dyn TableWriter,
    filter: &str,
    orphan_report: bool,
) -> Result<u64, ExportError> {
    if !views::state_of_in(job.tabs, job.file, job.cfg, "Sitemaps", filter)?.yields_rows() {
        tracing::warn!("{} requires the Sitemaps crawl analysis", job.entry_name);
        return Ok(0);
    }
    let processed = db(job.file.conn().query_row(
        "SELECT EXISTS(SELECT 1 FROM sitemaps WHERE kind IN ('urlset', 'index'))",
        [],
        |r| r.get::<_, bool>(0),
    ))?;
    if !processed {
        tracing::warn!("{} requires a processed sitemap", job.entry_name);
        return Ok(0);
    }
    let plan = views::plan_in(
        job.tabs,
        &views::ViewQuery::new("Sitemaps", filter),
        job.cfg,
    )?;
    let without_map = filter == "URLs Not In Sitemap";
    let oversized = filter.starts_with("XML Sitemap");
    let orphan = filter == "Orphan URLs";
    // Folge-Orphans verwenden die Mitgliedschaften ihrer Wurzel. Die DISTINCT-Auswahl schützt auch
    // bei mehreren Mitgliedschaften der Wurzel den Report (eine Zeile je URL).
    let joins = if without_map {
        String::new()
    } else if oversized {
        // In den Grenzfiltern ist die Datei selbst die betroffene URL (TECH-217/218).
        "JOIN sitemaps s ON s.url_id = u.id".into()
    } else if orphan {
        "JOIN url_flags f ON f.url_id = u.id AND f.flag = 'orphan' \
         JOIN sitemap_entries e ON e.url_id = COALESCE(CAST(NULLIF(f.value, '') AS INTEGER), u.id) \
         JOIN sitemaps s ON s.id = e.sitemap_id AND s.kind = 'urlset' \
         LEFT JOIN urls root ON root.id = CAST(NULLIF(f.value, '') AS INTEGER)"
            .into()
    } else {
        "JOIN sitemap_entries e ON e.url_id = u.id \
         JOIN sitemaps s ON s.id = e.sitemap_id AND s.kind = 'urlset'"
            .into()
    };
    let columns = if orphan_report {
        "DISTINCT u.address, 'Sitemap', u.status_code, u.indexability"
    } else if without_map {
        "u.address"
    } else if orphan {
        "s.url, u.address, u.status_code, u.indexability, u.indexability_status, root.address"
    } else {
        "s.url, u.address, u.status_code, u.indexability, u.indexability_status"
    };
    let order = if orphan_report || without_map {
        "u.address"
    } else {
        "u.address, s.url"
    };
    let sql = format!(
        "SELECT {columns} FROM urls u {joins} WHERE {} AND {} ORDER BY {order}",
        plan.population_sql, plan.filter_sql,
    );
    let mut stmt = db(job.file.conn().prepare(&sql))?;
    // Der Sitemap-Tab hat keine Schwellenparameter; CRAWL-089 ist bereits im Plan enthalten.
    let mut rows = db(stmt.query([]))?;
    let mut count = 0;
    while let Some(row) = db(rows.next())? {
        let (kinds, values) = if without_map {
            (vec![ColumnKind::Url], vec![text(db(row.get(0))?)])
        } else if orphan_report {
            (
                vec![
                    ColumnKind::Url,
                    ColumnKind::Text,
                    ColumnKind::Int,
                    ColumnKind::Text,
                ],
                vec![
                    text(db(row.get(0))?),
                    text(db(row.get(1))?),
                    integer(db(row.get(2))?),
                    text(db(row.get(3))?),
                ],
            )
        } else {
            let mut kinds = vec![
                ColumnKind::Url,
                ColumnKind::Url,
                ColumnKind::Int,
                ColumnKind::Text,
                ColumnKind::Text,
            ];
            let mut values = vec![
                text(db(row.get(0))?),
                text(db(row.get(1))?),
                integer(db(row.get(2))?),
                text(db(row.get(3))?),
                text(db(row.get(4))?),
            ];
            if orphan {
                kinds.push(ColumnKind::Url);
                values.push(text(db(row.get(5))?));
            }
            (kinds, values)
        };
        writer.row(&kinds, &values)?;
        count += 1;
    }
    Ok(count)
}

fn text(value: Option<String>) -> Value {
    value.map(Value::Text).unwrap_or(Value::Null)
}

fn integer(value: Option<i64>) -> Value {
    value
        .filter(|v| *v != 0)
        .map(Value::Int)
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{export_bulk, save_report, Format};
    use os_model::time::{IdSource, SeededIds};
    use os_model::{CrawlConfig, CrawlMode, Timestamp};
    use os_store::{CrawlFile, CrawlInit, StorageProfile};

    fn fixture() -> (tempfile::TempDir, CrawlFile) {
        let dir = tempfile::tempdir().unwrap();
        let at = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
        let file = CrawlFile::create(
            &dir.path().join("test.ospider"),
            &CrawlInit {
                crawl_id: SeededIds::new(1).new_crawl_id(at),
                mode: CrawlMode::Spider,
                start_url: Some("https://site.test/".into()),
                list_source: None,
                project: "".into(),
                name: "".into(),
                config: CrawlConfig::default(),
                created_at: at,
            },
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/tech-m2-sitemaps/store.sql"
            ))
            .unwrap();
        (dir, file)
    }

    fn csv(path: &std::path::Path) -> (Vec<String>, serde_json::Value) {
        let mut reader = csv::Reader::from_path(path).unwrap();
        let header = reader
            .headers()
            .unwrap()
            .iter()
            .map(str::to_owned)
            .collect();
        let rows: Vec<Vec<String>> = reader
            .records()
            .map(|r| r.unwrap().iter().map(str::to_owned).collect())
            .collect();
        (header, serde_json::to_value(rows).unwrap())
    }

    fn analyze(file: &CrawlFile, cfg: &CrawlConfig) {
        assert!(
            os_analyze::analysis::run(file, cfg, &os_model::CancelToken::new(), &|_, _| {})
                .unwrap()
                .complete
        );
    }

    // spec: OUT-057
    // spec: TECH-219
    #[test]
    fn all_seven_exports_match_fixture_and_legacy_aliases_resolve() {
        let (dir, file) = fixture();
        let cfg = CrawlConfig::default();
        analyze(&file, &cfg);
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/tech-m2-sitemaps/exports.json"
        ))
        .unwrap();
        for entry in entries() {
            let files = export_bulk(
                &file,
                &cfg,
                entry.name,
                &dir.path().join("out"),
                Format::Csv,
            )
            .unwrap();
            let (header, rows) = csv(&files[0].path);
            assert_eq!(header, entry.header, "{}", entry.name);
            assert_eq!(
                rows,
                expected[entry.name.strip_prefix("Sitemaps:").unwrap()],
                "{}",
                entry.name
            );
            assert_eq!(files[0].rows as usize, rows.as_array().unwrap().len());
            for alias in entry.aliases {
                assert_eq!(
                    crate::catalog::resolve(crate::catalog::ExportKind::Bulk, alias).unwrap(),
                    crate::catalog::ExportRef::Bulk(entry.name.into())
                );
                let alias_files =
                    export_bulk(&file, &cfg, alias, &dir.path().join("alias"), Format::Csv)
                        .unwrap();
                assert_eq!(csv(&alias_files[0].path), (header.clone(), rows.clone()));
            }
        }
    }

    // spec: OUT-057
    #[test]
    fn sitemap_bulk_exports_work_in_excel_as_well_as_csv() {
        use calamine::{open_workbook_auto, Reader};
        let (dir, file) = fixture();
        let cfg = CrawlConfig::default();
        analyze(&file, &cfg);
        let files = export_bulk(
            &file,
            &cfg,
            "Sitemaps:URLs In Multiple Sitemaps",
            &dir.path().join("xlsx"),
            Format::Xlsx,
        )
        .unwrap();
        assert_eq!(files[0].rows, 4);
        let mut workbook = open_workbook_auto(&files[0].path).unwrap();
        let range = workbook.worksheet_range_at(0).unwrap().unwrap();
        assert_eq!(range.height(), 5);
        assert_eq!(
            range.get((1, 0)).unwrap().to_string(),
            "https://site.test/a.xml"
        );
        assert_eq!(range.get((1, 2)).unwrap().to_string(), "200");
    }

    // spec: OUT-099
    #[test]
    fn orphan_report_deduplicates_memberships_preserves_followups_and_omits_linked_pages() {
        let (dir, file) = fixture();
        let cfg = CrawlConfig::default();
        analyze(&file, &cfg);
        let result = save_report(
            &file,
            &cfg,
            "Orphan Pages",
            &dir.path().join("out"),
            Format::Csv,
        )
        .unwrap();
        let (header, rows) = csv(&result.files[0].path);
        assert_eq!(header, ["Address", "Source", "Status Code", "Indexability"]);
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/tech-m2-sitemaps/exports.json"
        ))
        .unwrap();
        assert_eq!(rows, expected["Orphan Pages"]);
    }

    // spec: OUT-099
    #[test]
    fn report_uses_normalized_store_ids_and_preserves_trailing_slash_distinctions() {
        let (dir, file) = fixture();
        let cfg = CrawlConfig::default();
        // Der Ingest-Vertrag liefert normalisierte IDs; Rohwerte sind keine Join-Schlüssel.
        file.conn().execute_batch(
            "UPDATE sitemap_entries SET loc_raw = 'HTTPS://SITE.TEST:443/orphan#fragment' WHERE url_id = 4;
             INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,content_class,indexability)
             VALUES (10,'https://site.test/both/','https://site.test/both/','https://site.test',1,'done','sitemap',0,23,200,'HTML','Indexable');
             INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES (1,10,'https://site.test/both/');"
        ).unwrap();
        analyze(&file, &cfg);
        let result = save_report(
            &file,
            &cfg,
            "Orphan Pages",
            &dir.path().join("out"),
            Format::Csv,
        )
        .unwrap();
        let (_, rows) = csv(&result.files[0].path);
        let addresses: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r[0].as_str().unwrap())
            .collect();
        assert_eq!(
            addresses,
            [
                "https://site.test/both/",
                "https://site.test/orphan",
                "https://site.test/orphan-child",
                "https://site.test/pending"
            ]
        );
    }

    // spec: OUT-057
    // spec: OUT-099
    #[test]
    fn availability_and_empty_outputs_explain_missing_sources_and_pending_analysis() {
        let (dir, file) = fixture();
        let cfg = CrawlConfig::default();
        let report = &crate::reports::orphan_pages::entries()[0];
        let available =
            crate::reports::prereq::availability(report.name, report.requires, &file, &cfg);
        assert!(!available.enabled);
        assert!(available.reason.unwrap().contains("analysis"));
        let result = save_report(
            &file,
            &cfg,
            "Orphan Pages",
            &dir.path().join("pending"),
            Format::Csv,
        )
        .unwrap();
        assert_eq!(result.files[0].rows, 0);
        // Die Dateigrenzfilter sind vor der Analyse verfügbar.
        let live = entries()
            .iter()
            .find(|e| e.name.ends_with("Over 50MB"))
            .unwrap();
        assert!(
            crate::reports::prereq::availability(live.name, live.requires, &file, &cfg).enabled
        );
        file.conn()
            .execute("DELETE FROM sitemap_entries", [])
            .unwrap();
        file.conn().execute("DELETE FROM sitemaps", []).unwrap();
        let available =
            crate::reports::prereq::availability(report.name, report.requires, &file, &cfg);
        assert!(!available.enabled);
        assert!(available.reason.unwrap().contains("OrphanSourceAvailable"));
        for entry in entries() {
            assert!(
                !crate::reports::prereq::availability(entry.name, entry.requires, &file, &cfg)
                    .enabled
            );
            let files = export_bulk(
                &file,
                &cfg,
                entry.name,
                &dir.path().join("empty"),
                Format::Csv,
            )
            .unwrap();
            assert_eq!(files[0].rows, 0);
        }
    }
}
