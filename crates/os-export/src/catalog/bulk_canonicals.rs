//! Canonicals: Exporte je Filter (OUT-054).
//!
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::links_export::{link_kinds, stream_links, LinkOrder};
use crate::types::TableWriter;
use os_analyze::views::{self, ViewQuery};

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let entries: Vec<_> = entries().iter().collect();
    let name = crate::catalog::find_table(&entries, job.entry_name)
        .map_or(job.entry_name, |entry| entry.name);
    let filter = name
        .split_once(':')
        .map(|(_, name)| name.trim())
        .and_then(|name| name.strip_suffix(" Inlinks"))
        .unwrap_or("");
    if !views::state_of_in(job.tabs, job.file, job.cfg, "Canonicals", filter)?.yields_rows() {
        return Ok(0);
    }
    let plan = views::plan_in(job.tabs, &ViewQuery::new("Canonicals", filter), job.cfg)?;
    stream_links(
        job.file,
        &format!(
            "l.target_id IN (SELECT u.id FROM {} WHERE ({}) AND ({}))",
            views::URLS_FROM,
            plan.population_sql,
            plan.filter_sql
        ),
        &plan.params,
        LinkOrder::Inlinks,
        &link_kinds(),
        writer,
    )
}

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[
    TableEntry {
        name: "Canonicals:Contains Canonical Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Contains Canonical Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Self Referencing Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Self Referencing Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Canonicalised Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Canonicalised Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Missing Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Missing Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Multiple Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Multiple Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Multiple Conflicting Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Multiple Conflicting Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Non-Indexable Canonical Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Non-Indexable Canonical Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Canonical Is Relative Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Canonical Is Relative Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Unlinked Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Unlinked Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Invalid Attribute In Annotation Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Invalid Attribute In Annotation Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Contains Fragment URL Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Contains Fragment URL Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
    TableEntry {
        name: "Canonicals:Outside <head> Inlinks",
        aliases: &[],
        file_stem: "Canonicals:Outside <head> Inlinks",
        header: crate::catalog::LINK_HEADER,
        header_fn: None,
        requires: &[],
        owner: "M2-D04",
        run,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::canonical_chains::tests::{analyse, fixture, read};
    // spec: OUT-054
    #[test]
    fn every_canonical_filter_has_exactly_one_standard_inlinks_export_with_real_rows() {
        let fx = fixture();
        analyse(&fx);
        let tab = os_analyze::tabs::canonicals::tab();
        assert_eq!(entries().len(), tab.filters.len() - 1);
        for filter in tab.filters.iter().filter(|f| f.id != "All") {
            let name = format!("Canonicals:{} Inlinks", filter.id);
            let entry = entries()
                .iter()
                .filter(|e| e.name == name)
                .collect::<Vec<_>>();
            assert_eq!(entry.len(), 1, "{name}");
            let result = crate::export_bulk(
                &fx.file,
                &fx.cfg,
                &name,
                &fx.dir.path().join("bulk"),
                crate::Format::Csv,
            )
            .unwrap();
            let table = read(&result[0].path);
            assert_eq!(table.header, crate::catalog::LINK_HEADER);
            if [
                "Contains Canonical",
                "Canonicalised",
                "Canonical Is Relative",
                "Unlinked",
                "Invalid Attribute In Annotation",
                "Contains Fragment URL",
                "Outside <head>",
            ]
            .contains(&filter.id)
            {
                assert!(!table.rows.is_empty(), "{name}");
            }
        }
        let result = crate::export_bulk(
            &fx.file,
            &fx.cfg,
            "canonicals : canonicalised inlinks",
            &fx.dir.path().join("bulk"),
            crate::Format::Csv,
        )
        .unwrap();
        let table = read(&result[0].path);
        assert!(table
            .rows
            .iter()
            .any(|r| table.cell(r, "Source") == "https://site.test/home"
                && table.cell(r, "Destination") == "https://site.test/a"));
        let result = crate::export_bulk(
            &fx.file,
            &fx.cfg,
            "Canonicals:Contains Fragment URL Inlinks",
            &fx.dir.path().join("bulk"),
            crate::Format::Xlsx,
        )
        .unwrap();
        assert_eq!(result[0].rows, 1);
    }
}
