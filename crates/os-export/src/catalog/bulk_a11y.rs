use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};
use os_analyze::tabs::accessibility as a11y;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Accessibility:All Violations",
        aliases: &[],
        file_stem: "Accessibility:All Violations",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 A",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 A",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.0 AAA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.0 AAA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.1 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.1 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:WCAG 2.2 AA",
        aliases: &[],
        file_stem: "Accessibility:WCAG 2.2 AA",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:Best Practice",
        aliases: &[],
        file_stem: "Accessibility:Best Practice",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
    TableEntry {
        name: "Accessibility:Incomplete",
        aliases: &[],
        file_stem: "Accessibility:Incomplete",
        header: &[
            "Address",
            "Issue",
            "Rule ID",
            "Guideline",
            "Impact",
            "Priority",
            "Location on Page",
            "HTML",
            "Help URL",
        ],
        requires: &[
            ReportRequirement::Rendering,
            ReportRequirement::Config {
                key: "extraction.accessibility",
                what: "accessibility extraction",
            },
        ],
        owner: "M2-D21",
        run,
        header_fn: None,
    },
];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let group = job.entry_name.split(':').nth(1).unwrap_or("");
    let status = if group == "Incomplete" {
        "incomplete"
    } else {
        "violation"
    };
    let group = if group == "Incomplete" || group == "All Violations" {
        ""
    } else {
        group
    };
    let sql=format!("SELECT u.address,a.rule_id,a.impact,n.target,n.snippet FROM a11y_nodes n JOIN a11y_rules a ON a.url_id=n.url_id AND a.rule_id=n.rule_id JOIN urls u ON u.id=a.url_id WHERE {} AND a.status=?1 ORDER BY u.address COLLATE OS_TEXT,a.rule_id,n.id",a11y::POPULATION);
    let mut stmt = crate::links_export::sql(job.file.conn().prepare(&sql))?;
    let mut rows = crate::links_export::sql(stmt.query([status]))?;
    let mut count = 0;
    while let Some(row) = crate::links_export::sql(rows.next())? {
        let id: String = crate::links_export::sql(row.get(1))?;
        let Some(rule) = a11y::rule(&id) else {
            continue;
        };
        if !group.is_empty() && rule.group != group {
            continue;
        }
        let impact: Option<String> = crate::links_export::sql(row.get(2))?;
        let html: String = crate::links_export::sql(row.get(4))?;
        let values = vec![
            Value::Text(crate::links_export::sql(row.get(0))?),
            rule.issue.clone().into(),
            id.clone().into(),
            rule.group.clone().into(),
            a11y::impact(impact.as_deref()).into(),
            rule.priority.clone().into(),
            Value::Text(crate::links_export::sql(row.get(3))?),
            html.chars().take(500).collect::<String>().into(),
            a11y::help_url(&id).into(),
        ];
        writer.row(&[os_analyze::ColumnKind::Text; 9], &values)?;
        count += 1;
    }
    Ok(count)
}

/// Two-part detail artifact (TECH-259, OUT-021). The caller stages and publishes the files.
pub fn export_details(
    job: &TableJob<'_>,
    req: &os_analyze::details::DetailReq<'_>,
    path: &std::path::Path,
    format: crate::Format,
    ctx: &mut crate::types::ExportCtx<'_>,
) -> Result<Vec<crate::types::ExportFileInfo>, ExportError> {
    use crate::types::WorkbookTarget;
    let options = crate::pipeline::writer_options(job.cfg);
    let parts = [
        (os_analyze::details::DetailKind::A11yRules, "Rules"),
        (os_analyze::details::DetailKind::A11yNodes, "Locations"),
    ];
    if format == crate::Format::Xlsx {
        let mut writer = crate::xlsx::XlsxWriter::create(path, "Rules", &options)?;
        for (kind, name) in parts {
            crate::detail_export::export_detail_with_ctx(kind, job, req, writer.sheet(name)?, ctx)?;
        }
        ctx.checkpoint()?;
        return Ok(vec![Box::new(writer).finish()?]);
    }
    let mut files = vec![];
    for (kind, name) in parts {
        let path = detail_path(path, name);
        let mut writer = crate::output::open_writer(&path, format, name, &options)?;
        crate::detail_export::export_detail_with_ctx(kind, job, req, writer.as_mut(), ctx)?;
        ctx.checkpoint()?;
        files.push(writer.finish()?);
    }
    Ok(files)
}
pub fn detail_path(path: &std::path::Path, part: &str) -> std::path::PathBuf {
    let mut name = path.file_stem().unwrap_or_default().to_os_string();
    name.push(format!("_{}", part.to_ascii_lowercase()));
    if let Some(ext) = path.extension() {
        name.push(".");
        name.push(ext);
    }
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp, UrlId};
    fn fixture() -> (tempfile::TempDir, os_store::CrawlFile, CrawlConfig) {
        let dir = tempfile::tempdir().unwrap();
        let at = Timestamp::UNIX_EPOCH;
        let mut cfg = CrawlConfig::default();
        cfg.extraction.accessibility = true;
        cfg.rendering.mode = os_model::RenderingMode::JavaScript;
        let init = os_store::CrawlInit {
            crawl_id: SeededIds::new(21).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: None,
            list_source: None,
            project: String::new(),
            name: "a11y".into(),
            config: cfg.clone(),
            created_at: at,
        };
        let file = os_store::CrawlFile::create(
            &dir.path().join("a11y.ospider"),
            &init,
            os_store::StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/ui/fx-ui-a11y/store.sql"
            ))
            .unwrap();
        (dir, file, cfg)
    }
    // spec: OUT-062
    // spec: OUT-109
    #[test]
    fn bulk_partitions_nodes_and_summary_counts_pages() {
        let (dir, file, cfg) = fixture();
        let tabs = os_analyze::tabs::all();
        let mut counts = Vec::new();
        for entry in entries()
            .iter()
            .chain(crate::reports::a11y_summary::entries())
        {
            let job = TableJob {
                file: &file,
                cfg: &cfg,
                tabs: &tabs,
                entry_name: entry.name,
            };
            let path = dir.path().join(format!("{}.csv", counts.len()));
            let mut writer =
                crate::output::open_writer(&path, crate::Format::Csv, "test", &Default::default())
                    .unwrap();
            writer
                .header(
                    &entry
                        .header
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            let count = (entry.run)(&job, writer.as_mut()).unwrap();
            writer.finish().unwrap();
            let text = std::fs::read_to_string(path).unwrap();
            let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/ui/fx-ui-a11y/golden")
                .join(format!("{}.csv", entry.name.replace([':', ' '], "_")));
            if std::env::var_os("UPDATE_GOLDEN").is_some() {
                std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
                std::fs::write(&golden, &text).unwrap();
            }
            assert_eq!(std::fs::read_to_string(golden).unwrap(), text);

            if entry.name.ends_with("Violations Summary") {
                assert_eq!(count, 3);
                let line = text.lines().find(|l| l.contains(",image-alt,")).unwrap();
                assert!(line.contains(",2,100.00,"), "{line}");
            }
            counts.push(count);
        }
        assert_eq!(&counts[..8], &[5, 3, 0, 0, 0, 0, 2, 1]);
    }
    // spec: TECH-259
    // spec: OUT-021
    #[test]
    fn details_export_two_csv_files_or_two_workbook_sheets() {
        use calamine::{open_workbook_auto, Reader};
        let (dir, file, cfg) = fixture();
        let tabs = os_analyze::tabs::all();
        let job = TableJob {
            file: &file,
            cfg: &cfg,
            tabs: &tabs,
            entry_name: "Accessibility Details",
        };
        let ids = [UrlId(1)];
        let params = serde_json::json!({"result":"All"});
        let req = os_analyze::details::DetailReq {
            url_ids: &ids,
            params: &params,
            sort: &[],
            offset: 0,
            limit: 500,
        };
        let cancel = Default::default();
        let mut ctx = crate::types::ExportCtx {
            cancel: &cancel,
            progress: &|_| {},
            skip_empty: false,
            workbook: None,
            skipped: vec![],
        };
        let files = export_details(
            &job,
            &req,
            &dir.path().join("audit.csv"),
            crate::Format::Csv,
            &mut ctx,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].path.ends_with("audit_rules.csv"));
        assert!(files[1].path.ends_with("audit_locations.csv"));
        assert_eq!(files[0].rows, 6);
        assert_eq!(files[1].rows, 5);
        let files = export_details(
            &job,
            &req,
            &dir.path().join("audit.xlsx"),
            crate::Format::Xlsx,
            &mut ctx,
        )
        .unwrap();
        let mut book = open_workbook_auto(&files[0].path).unwrap();
        assert_eq!(book.sheet_names(), ["Rules", "Locations"]);
        assert_eq!(book.worksheet_range("Rules").unwrap().height(), 7);
        assert_eq!(book.worksheet_range("Locations").unwrap().height(), 6);
    }
}
