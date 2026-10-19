//! Filter-based inlink exports, derived from the canonical tab registry (OUT-066).

use crate::{
    catalog::{TableEntry, TableJob},
    error::ExportError,
    types::TableWriter,
};
use os_analyze::{defs::intern, names, views};
use std::sync::OnceLock;

pub const TABS: &[&str] = &[
    "Page Titles",
    "Meta Description",
    "Meta Keywords",
    "H1",
    "H2",
    "Hreflang",
    "Pagination",
    "Mobile",
    "Validation",
    "Link Metrics",
    "Analytics",
    "Search Console",
    "PageSpeed",
];

pub fn entries() -> &'static [TableEntry] {
    static ENTRIES: OnceLock<Vec<TableEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        os_analyze::tabs::all()
            .into_iter()
            .filter(|t| TABS.contains(&t.id))
            .flat_map(|tab| {
                tab.filters
                    .iter()
                    .filter(|filter| {
                        let name =
                            format!("{}:{} Inlinks", tab.id, super::placeholders_to_x(filter.id));
                        !super::bulk_pagination::entries()
                            .iter()
                            .any(|entry| entry.name == name)
                    })
                    .map(|filter| {
                        let name = intern(&format!(
                            "{}:{} Inlinks",
                            tab.id,
                            super::placeholders_to_x(filter.id)
                        ));
                        TableEntry {
                            name,
                            aliases: &[],
                            file_stem: name,
                            header: super::LINK_HEADER,
                            header_fn: None,
                            requires: &[],
                            owner: "M2-X01",
                            run,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    if !job.cfg.export.inlinks_for_all_filters {
        return Ok(0);
    }
    let (tab_name, filter) = job
        .entry_name
        .rsplit_once(':')
        .and_then(|(tab, filter)| filter.strip_suffix(" Inlinks").map(|f| (tab, f)))
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?;
    let tab = names::find_tab(job.tabs, tab_name)
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?;
    let filter = names::find_filter(tab, filter)
        .or_else(|| {
            tab.filters
                .iter()
                .find(|f| names::label(f.id, job.cfg).eq_ignore_ascii_case(filter))
        })
        .ok_or_else(|| ExportError::Unsupported(job.entry_name.into()))?;
    let state = views::state_of_in(job.tabs, job.file, job.cfg, tab.id, filter.id)?;
    if state != os_model::FilterState::Ready {
        tracing::warn!(
            "{}",
            crate::pipeline::filter_warning(tab.id, filter.id, &state)
        );
        return Ok(0);
    }
    let plan = views::plan_in(job.tabs, &views::ViewQuery::new(tab.id, filter.id), job.cfg)?;
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

/// Maps a displayed threshold back to the static catalogue name.
pub fn canonical_name(name: &str, cfg: &os_model::CrawlConfig) -> Option<&'static str> {
    let (tab_name, filter) = name.trim().rsplit_once(':')?;
    let filter = filter.split_whitespace().collect::<Vec<_>>().join(" ");
    let (filter, suffix) = filter.rsplit_once(' ')?;
    if !suffix.eq_ignore_ascii_case("Inlinks") {
        return None;
    }
    let tabs = os_analyze::tabs::configured(cfg);
    let tab = names::find_tab(&tabs, tab_name)?;
    if !TABS.contains(&tab.id) {
        return None;
    }
    let filter = tab.filters.iter().find(|candidate| {
        names::label(candidate.id, cfg).eq_ignore_ascii_case(filter)
            || super::placeholders_to_x(candidate.id).eq_ignore_ascii_case(filter)
    })?;
    let canonical = format!("{}:{} Inlinks", tab.id, super::placeholders_to_x(filter.id));
    entries()
        .iter()
        .find(|entry| entry.name == canonical)
        .map(|entry| entry.name)
}

/// Whether a canonical name is governed by the generic-inlink setting,
/// including the Pagination entries supplied by D05.
pub fn is_generic_name(name: &str) -> bool {
    name.rsplit_once(':')
        .is_some_and(|(tab, filter)| TABS.contains(&tab) && filter.ends_with(" Inlinks"))
}
