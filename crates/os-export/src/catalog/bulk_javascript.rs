//! JavaScript filter inlinks, blocked resources and console context (TECH-102/114).
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};
use os_analyze::{defs::ColumnKind, views};
use os_model::{FilterState, Value};

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "JavaScript:Pages With JavaScript Issues",
        aliases: &[],
        file_stem: "JavaScript:Pages With JavaScript Issues",
        header: CONSOLE_HEADER,
        header_fn: None,
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "rendering.js_error_reporting",
                what: "JavaScript error reporting",
            },
        ],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Blocked Resources",
        aliases: &[],
        file_stem: "JavaScript:Blocked Resources",
        header: &["Address", "Resource", "Resource Type"],
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Blocked Resource Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Blocked Resource Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Contains JavaScript Links Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Contains JavaScript Links Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Contains JavaScript Content Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Contains JavaScript Content Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Noindex Only In Original HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Noindex Only In Original HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Nofollow Only In Original HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Nofollow Only In Original HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Canonical Only In Rendered HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Canonical Only In Rendered HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Canonical Mismatch Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Canonical Mismatch Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Page Title Only In Rendered HTML Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Page Title Only In Rendered HTML Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:Pages With JavaScript Errors Inlinks",
        aliases: &[],
        file_stem: "JavaScript:Pages With JavaScript Errors Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
    },
    TableEntry {
        name: "JavaScript:All Console Messages",
        aliases: &[],
        file_stem: "JavaScript:All Console Messages",
        header: &[
            "Address",
            "Level",
            "Message",
            "Source URL",
            "Line",
            "Column",
        ],
        requires: &[ReportRequirement::Rendering],
        owner: "M2-R02",
        run,
        header_fn: None,
    },
];

const CONSOLE_HEADER: &[&str] = &[
    "Address",
    "Level",
    "Message",
    "Source URL",
    "Line",
    "Column",
];
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if job.cfg.rendering.mode != os_model::RenderingMode::JavaScript {
        return Ok(0);
    }
    let console = job.entry_name.ends_with("Console Messages")
        || job.entry_name.ends_with("JavaScript Issues");
    let blocked = job.entry_name.ends_with(":Blocked Resources");
    if console && !job.cfg.rendering.js_error_reporting {
        return Ok(0);
    }
    if console || blocked {
        let condition = if job.entry_name.ends_with("JavaScript Issues") {
            format!("{} AND EXISTS (SELECT 1 FROM render_console e WHERE e.url_id=u.id AND e.level IN ('error','exception'))", os_analyze::tabs::javascript::POPULATION)
        } else {
            "1".into()
        };
        let query = if console {
            format!("SELECT u.address,e.level,e.text,e.source,e.line,e.col FROM render_console e JOIN urls u ON u.id=e.url_id WHERE {condition} ORDER BY u.address COLLATE BINARY,u.id,e.position")
        } else {
            "SELECT u.address,b.url,COALESCE((SELECT n.resource_type FROM render_network n WHERE n.url_id=b.url_id AND n.url=b.url LIMIT 1), (SELECT CASE t.content_class WHEN 'JavaScript' THEN 'Script' WHEN 'CSS' THEN 'Stylesheet' WHEN 'Images' THEN 'Image' ELSE t.content_class END FROM urls t WHERE t.url=b.url), '') FROM render_blocked b JOIN urls u ON u.id=b.url_id WHERE b.reason='robots' ORDER BY u.address COLLATE BINARY,u.id,b.position".into()
        };
        let mut statement = crate::links_export::sql(job.file.conn().prepare(&query))?;
        let mut rows = crate::links_export::sql(statement.query([]))?;
        let mut count = 0;
        while let Some(row) = crate::links_export::sql(rows.next())? {
            writer.checkpoint()?;
            let length = if console { 6 } else { 3 };
            let mut values = Vec::new();
            let mut kinds = Vec::new();
            for i in 0..length {
                if console && i >= 4 {
                    values.push(
                        crate::links_export::sql(row.get::<_, Option<i64>>(i))?
                            .map_or(Value::Null, Value::Int),
                    );
                    kinds.push(ColumnKind::Int);
                } else {
                    values.push(
                        crate::links_export::sql(row.get::<_, Option<String>>(i))?
                            .map_or(Value::Null, Value::Text),
                    );
                    kinds.push(ColumnKind::Text);
                }
            }
            writer.row(&kinds, &values)?;
            count += 1;
        }
        return Ok(count);
    }
    let filter = job
        .entry_name
        .strip_prefix("JavaScript:")
        .and_then(|n| n.strip_suffix(" Inlinks"))
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?;
    let filter = if filter == "Blocked Resource" {
        "Pages With Blocked Resources"
    } else {
        filter
    };
    if views::state_of_in(job.tabs, job.file, job.cfg, "JavaScript", filter)? != FilterState::Ready
    {
        return Ok(0);
    }
    let plan = views::plan_in(
        job.tabs,
        &views::ViewQuery::new("JavaScript", filter),
        job.cfg,
    )?;
    let condition = format!(
        "l.target_id IN (SELECT u.id FROM {} WHERE ({}) AND ({}))",
        views::URLS_FROM,
        plan.population_sql,
        plan.filter_sql
    );
    crate::links_export::stream_links(
        job.file,
        &condition,
        &plan.params,
        crate::links_export::LinkOrder::Inlinks,
        &crate::links_export::link_kinds(),
        writer,
    )
}

#[cfg(test)]
mod tests {
    use os_model::{
        time::{IdSource, SeededIds},
        CrawlConfig, CrawlMode, RenderingMode, Timestamp,
    };
    use os_store::{CrawlFile, CrawlInit, StorageProfile};
    fn setup() -> (tempfile::TempDir, CrawlFile, CrawlConfig) {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.rendering.mode = RenderingMode::JavaScript;
        let at = Timestamp::from_db("2026-10-21T00:00:00.000Z").unwrap();
        let file = CrawlFile::create(
            &dir.path().join("js.ospider"),
            &CrawlInit {
                crawl_id: SeededIds::new(2).new_crawl_id(at),
                mode: CrawlMode::Spider,
                start_url: Some("https://site.test/".into()),
                list_source: None,
                project: "JavaScript".into(),
                name: "Fixture".into(),
                config: cfg.clone(),
                created_at: at,
            },
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/tech-m2-javascript/facts.sql"
            ))
            .unwrap();
        (dir, file, cfg)
    }
    fn records(path: &std::path::Path) -> Vec<Vec<String>> {
        let bytes = std::fs::read(path).unwrap();
        csv::Reader::from_reader(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap())
            .records()
            .map(|r| r.unwrap().iter().map(str::to_string).collect())
            .collect()
    }
    // spec: TECH-114
    #[test]
    fn issues_export_keeps_all_context_for_error_pages_and_omits_warning_only_pages() {
        use calamine::Reader;
        let (dir, file, mut cfg) = setup();
        for format in [crate::Format::Csv, crate::Format::Xlsx] {
            let files = crate::export_bulk(
                &file,
                &cfg,
                "JavaScript:Pages With JavaScript Issues",
                dir.path(),
                format,
            )
            .unwrap();
            assert_eq!(files[0].rows, 2);
            if format == crate::Format::Csv {
                let rows = records(&files[0].path);
                assert_eq!(
                    rows.iter().map(|r| r[1].as_str()).collect::<Vec<_>>(),
                    ["warning", "error"]
                );
                assert!(rows.iter().all(|r| r[0] == "https://site.test/spa"));
            } else {
                let mut workbook: calamine::Xlsx<_> =
                    calamine::open_workbook(&files[0].path).unwrap();
                assert_eq!(workbook.worksheet_range_at(0).unwrap().unwrap().height(), 3);
            }
        }
        let files = crate::export_bulk(
            &file,
            &cfg,
            "JavaScript:All Console Messages",
            dir.path(),
            crate::Format::Csv,
        )
        .unwrap();
        assert_eq!(files[0].rows, 3);
        cfg.rendering.js_error_reporting = false;
        let files = crate::export_bulk(
            &file,
            &cfg,
            "JavaScript:Pages With JavaScript Issues",
            dir.path(),
            crate::Format::Csv,
        )
        .unwrap();
        assert_eq!(files[0].rows, 0);
        assert!(!files[0].warnings.is_empty());
    }
    // spec: TECH-102
    // spec: TECH-103
    #[test]
    fn blocked_resource_export_excludes_errors_and_crawl_exclusions_and_inlinks_are_real() {
        let (dir, file, cfg) = setup();
        file.conn().execute("INSERT INTO render_network(url_id,position,url,status,resource_type) VALUES (1,0,'https://site.test/blocked.js',0,'script')",[]).unwrap();
        let files = crate::export_bulk(
            &file,
            &cfg,
            "JavaScript:Blocked Resources",
            dir.path(),
            crate::Format::Csv,
        )
        .unwrap();
        let rows = records(&files[0].path);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            [
                "https://site.test/spa",
                "https://site.test/blocked.js",
                "script"
            ]
        );
        let files = crate::export_bulk(
            &file,
            &cfg,
            "JavaScript:Contains JavaScript Links Inlinks",
            dir.path(),
            crate::Format::Csv,
        )
        .unwrap();
        assert!(files[0].rows > 0);
        let rows = records(&files[0].path);
        assert!(rows.iter().any(|r| r[14] == "Rendered HTML"));
    }
}
