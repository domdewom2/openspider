//! Pagination-Inlinks je Filter (OUT-066); Reports OUT-088/089 bleiben P2.

use crate::catalog::{find_table, reports::ReportRequirement, TableEntry, TableJob};
use crate::error::ExportError;
use crate::links_export::{self, LinkOrder};
use crate::types::TableWriter;
use os_analyze::views::{self, ViewQuery};
use os_model::FilterState;

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let registered: Vec<_> = entries().iter().collect();
    let entry = find_table(&registered, job.entry_name).ok_or_else(|| {
        ExportError::Name(os_analyze::NameError::Unknown {
            input: job.entry_name.to_owned(),
            suggestions: Vec::new(),
        })
    })?;
    let filter = entry
        .name
        .strip_prefix("Pagination:")
        .and_then(|s| s.strip_suffix(" Inlinks"))
        .ok_or_else(|| {
            ExportError::Write(format!("Ungültiger Pagination-Export: {}", entry.name))
        })?;
    if views::state_of_in(job.tabs, job.file, job.cfg, "Pagination", filter)? != FilterState::Ready
    {
        return Ok(0);
    }
    let plan = views::plan_in(job.tabs, &ViewQuery::new("Pagination", filter), job.cfg)?;
    let condition = format!(
        "l.target_id IN (SELECT u.id FROM urls u WHERE ({}) AND ({}))",
        plan.population_sql, plan.filter_sql
    );
    links_export::stream_links(
        job.file,
        &condition,
        &plan.params,
        LinkOrder::Inlinks,
        &links_export::link_kinds(),
        writer,
    )
}

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Pagination:Contains Pagination Inlinks",
        aliases: &[],
        file_stem: "Pagination:Contains Pagination Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:First Page Inlinks",
        aliases: &[],
        file_stem: "Pagination:First Page Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Paginated 2+ Pages Inlinks",
        aliases: &[],
        file_stem: "Pagination:Paginated 2+ Pages Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Pagination URL Not In Anchor Tag Inlinks",
        aliases: &[],
        file_stem: "Pagination:Pagination URL Not In Anchor Tag Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Non-200 Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Non-200 Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Unlinked Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Unlinked Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Non-Indexable Inlinks",
        aliases: &[],
        file_stem: "Pagination:Non-Indexable Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Multiple Pagination URLs Inlinks",
        aliases: &[],
        file_stem: "Pagination:Multiple Pagination URLs Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Pagination Loop Inlinks",
        aliases: &[],
        file_stem: "Pagination:Pagination Loop Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D05",
        run,
    },
    TableEntry {
        name: "Pagination:Sequence Error Inlinks",
        aliases: &[],
        file_stem: "Pagination:Sequence Error Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[ReportRequirement::CrawlAnalysis],
        owner: "M2-D05",
        run,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExportFileInfo;
    use os_model::{AnalysisStep, CrawlConfig, CrawlMode, IdSource, SeededIds, Timestamp, Value};
    use os_store::{CrawlFile, CrawlInit, StorageProfile};

    #[derive(Default)]
    struct Rows(Vec<Vec<Value>>);
    impl TableWriter for Rows {
        fn header(&mut self, _: &[String]) -> Result<(), ExportError> {
            panic!("TableRun writes rows only")
        }
        fn row(
            &mut self,
            _: &[os_analyze::ColumnKind],
            values: &[Value],
        ) -> Result<(), ExportError> {
            self.0.push(values.to_vec());
            Ok(())
        }
        fn finish(self: Box<Self>) -> Result<ExportFileInfo, ExportError> {
            unreachable!()
        }
    }

    // spec: TECH-031
    // spec: TECH-032
    // spec: TECH-037
    #[test]
    fn filter_inlinks_use_the_same_population_and_analysis_state_as_the_tab() {
        let dir = tempfile::tempdir().unwrap();
        let at = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
        let cfg = CrawlConfig::default();
        let init = CrawlInit {
            crawl_id: SeededIds::new(5).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "pagination".into(),
            name: "D05".into(),
            config: cfg.clone(),
            created_at: at,
        };
        let file = CrawlFile::create(
            &dir.path().join("export.ospider"),
            &init,
            StorageProfile::Balanced,
        )
        .unwrap();
        file.conn()
            .execute_batch(include_str!(
                "../../../../tests/fixtures/tech-m2-pagination/crawl.sql"
            ))
            .unwrap();
        let tabs = os_analyze::tabs::all();
        let job = |name| TableJob {
            entry_name: name,
            tabs: &tabs,
            file: &file,
            cfg: &cfg,
        };
        let mut rows = Rows::default();
        assert_eq!(
            run(
                &job("Pagination:Unlinked Pagination URLs Inlinks"),
                &mut rows
            )
            .unwrap(),
            0
        );
        os_analyze::analysis::runner::run_only(
            &file,
            &cfg,
            &os_analyze::analysis::CancelToken::new(),
            &|_, _| {},
            &[AnalysisStep::Pagination],
        )
        .unwrap();
        let count = run(
            &job("Pagination:Unlinked Pagination URLs Inlinks"),
            &mut rows,
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            rows.0.iter().map(|r| r[2].clone()).collect::<Vec<_>>(),
            [
                Value::Text("https://site.test/orphan-source".into()),
                Value::Text("https://site.test/to-external".into())
            ]
        );
        for entry in entries() {
            let mut rows = Rows::default();
            let count = (entry.run)(&job(entry.name), &mut rows).unwrap();
            assert_eq!(count as usize, rows.0.len());
            assert!(count > 0, "{}", entry.name);
            assert!(rows
                .0
                .iter()
                .all(|r| r.len() == crate::catalog::LINK_HEADER.len()));
            let variant = entry.name.to_lowercase().replace(':', " :  ");
            let mut alternate = Rows::default();
            let alternate_job = TableJob {
                entry_name: &variant,
                tabs: &tabs,
                file: &file,
                cfg: &cfg,
            };
            assert_eq!(run(&alternate_job, &mut alternate).unwrap(), count);
            assert_eq!(alternate.0, rows.0);
        }
        assert!(matches!(
            run(&job("Pagination:unknown Inlinks"), &mut Rows::default()),
            Err(ExportError::Name(_))
        ));
    }
}
