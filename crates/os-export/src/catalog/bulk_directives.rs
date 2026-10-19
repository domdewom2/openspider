//! Directives-Inlinks aus denselben SQL-Plänen wie die Tab-Filter (OUT-055).
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let filter = job
        .entry_name
        .strip_prefix("Directives:")
        .and_then(|name| name.strip_suffix(" Inlinks"))
        .ok_or_else(|| {
            ExportError::Unsupported(format!("ungültiger Directives-Export: {}", job.entry_name))
        })?;
    // TECH-060/OUT-055: Altlasten bleiben P2; dafür nur die registrierte Kopfzeile.
    if matches!(filter, "NoODP" | "NoYDIR") {
        return Ok(0);
    }
    let state = os_analyze::views::state_of_in(job.tabs, job.file, job.cfg, "Directives", filter)?;
    if !state.yields_rows() {
        return Ok(0);
    }
    let plan = os_analyze::views::plan_in(
        job.tabs,
        &os_analyze::views::ViewQuery::new("Directives", filter),
        job.cfg,
    )?;
    crate::links_export::stream_links(
        job.file,
        &format!(
            "l.target_id IN (SELECT u.id FROM {} WHERE {} AND {})",
            os_analyze::views::URLS_FROM,
            plan.population_sql,
            plan.filter_sql
        ),
        &plan.params,
        crate::links_export::LinkOrder::Inlinks,
        &crate::links_export::link_kinds(),
        writer,
    )
}

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Directives:Index Inlinks",
        aliases: &[],
        file_stem: "Directives:Index Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Noindex Inlinks",
        aliases: &[],
        file_stem: "Directives:Noindex Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Follow Inlinks",
        aliases: &[],
        file_stem: "Directives:Follow Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Nofollow Inlinks",
        aliases: &[],
        file_stem: "Directives:Nofollow Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:None Inlinks",
        aliases: &[],
        file_stem: "Directives:None Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoArchive Inlinks",
        aliases: &[],
        file_stem: "Directives:NoArchive Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoSnippet Inlinks",
        aliases: &[],
        file_stem: "Directives:NoSnippet Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Max-Snippet Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Snippet Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Max-Image-Preview Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Image-Preview Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Max-Video-Preview Inlinks",
        aliases: &[],
        file_stem: "Directives:Max-Video-Preview Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoODP Inlinks",
        aliases: &[],
        file_stem: "Directives:NoODP Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoYDIR Inlinks",
        aliases: &[],
        file_stem: "Directives:NoYDIR Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoImageIndex Inlinks",
        aliases: &[],
        file_stem: "Directives:NoImageIndex Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:NoTranslate Inlinks",
        aliases: &[],
        file_stem: "Directives:NoTranslate Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Unavailable_After Inlinks",
        aliases: &[],
        file_stem: "Directives:Unavailable_After Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Refresh Inlinks",
        aliases: &[],
        file_stem: "Directives:Refresh Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D06",
        run,
    },
    TableEntry {
        name: "Directives:Outside <head> Inlinks",
        aliases: &[],
        file_stem: "Directives:Outside <head> Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[crate::catalog::reports::ReportRequirement::Config {
            key: "advanced.html_validation",
            what: "HTML validation",
        }],
        owner: "M2-D06",
        run,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use os_engine::testing::crawl_fixture;
    use os_model::CrawlConfig;

    // spec: OUT-055
    #[tokio::test]
    async fn every_p1_filter_exports_its_actual_inlinks_in_standard_format() {
        let cfg = CrawlConfig::default();
        let fx = crawl_fixture("tech-m2-directives", cfg.clone()).await;
        fx.file.conn().execute("INSERT OR REPLACE INTO url_ext(url_id, key, v_int) SELECT id, 'validation.checked', 1 FROM urls", []).unwrap();
        let tabs = os_analyze::tabs::all();
        let tab = tabs.iter().find(|t| t.id == "Directives").unwrap();
        let out = fx.dir.path().join("exports");
        for filter in tab.filters.iter().filter(|f| f.id != "All") {
            let name = format!("Directives:{} Inlinks", filter.id);
            let entry = entries().iter().find(|e| e.name == name).unwrap();
            let mut q = os_analyze::views::ViewQuery::new("Directives", filter.id);
            q.limit = 500;
            q.columns = os_analyze::views::ColumnSelection::Named(vec!["Address".into()]);
            let expected: std::collections::BTreeSet<_> =
                os_analyze::views::query_view(&fx.file, &cfg, &q)
                    .unwrap()
                    .rows
                    .into_iter()
                    .map(|r| match &r[0] {
                        os_model::Value::Text(s) => s.clone(),
                        _ => panic!("Adresse"),
                    })
                    .collect();
            let job = TableJob {
                entry_name: entry.name,
                tabs: &tabs,
                file: &fx.file,
                cfg: &cfg,
            };
            let path = out.join(entry.file_name("csv"));
            std::fs::create_dir_all(&out).unwrap();
            let info =
                crate::catalog::export_table(entry, &job, &path, crate::Format::Csv).unwrap();
            let text = std::fs::read_to_string(&info.path).unwrap();
            let mut csv = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
            assert_eq!(
                csv.headers().unwrap().iter().collect::<Vec<_>>(),
                crate::links_export::LINK_COLUMNS
            );
            let actual: std::collections::BTreeSet<String> =
                csv.records().map(|r| r.unwrap()[2].to_string()).collect();
            assert_eq!(actual, expected, "{}", filter.id);
            if filter.id == "Noindex" {
                assert!(actual.contains(&format!("{}/header.html", fx.site.base_url())));
                assert!(actual.contains(&format!("{}/body.html", fx.site.base_url())));
            }
        }
        // P2-Verträge bleiben registriert, ohne vorgezogene Implementierung.
        for entry in entries()
            .iter()
            .filter(|e| e.name.contains("NoODP") || e.name.contains("NoYDIR"))
        {
            assert_eq!(entry.owner, "M2-D06");
        }
    }
}
