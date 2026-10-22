//! Internal reuses the actual integration column definitions, including file-dependent expansion.
use crate::defs::{ColumnDef, ColumnSource};
use os_model::CrawlConfig;

pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(InternalColumns))
}
struct InternalColumns;
fn sources(cfg: &CrawlConfig) -> Vec<Box<dyn ColumnSource>> {
    [
        super::custom_search::column_source(),
        super::custom_js::column_source(),
        super::analytics::column_source(),
        if cfg.api.psi.enabled {
            super::pagespeed::column_source()
        } else {
            None
        },
    ]
    .into_iter()
    .flatten()
    .collect()
}
fn gsc(cfg: &CrawlConfig) -> Vec<ColumnDef> {
    if !cfg.api.gsc.enabled {
        return Vec::new();
    }
    super::search_console::tab()
        .columns
        .into_iter()
        .filter(|c| {
            matches!(c.needs, Some(crate::defs::Needs::Api("gsc")))
                || matches!(
                    c.needs,
                    Some(crate::defs::Needs::Config("api.gsc.url_inspection"))
                )
        })
        .collect()
}
impl ColumnSource for InternalColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        let mut out: Vec<_> = sources(cfg)
            .into_iter()
            .flat_map(|s| s.columns(cfg))
            .collect();
        out.extend(gsc(cfg));
        out
    }
    fn columns_for_file(&self, cfg: &CrawlConfig, file: &os_store::CrawlFile) -> Vec<ColumnDef> {
        let mut effective = cfg.clone();
        if file
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM api_values WHERE source='psi')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
        {
            effective.api.psi.enabled = true;
        }
        let mut out: Vec<_> = sources(&effective)
            .into_iter()
            .flat_map(|s| s.columns_for_file(cfg, file))
            .collect();
        if file
            .conn()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM api_values WHERE source='gsc')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
        {
            effective.api.gsc.enabled = true;
        }
        out.extend(gsc(&effective));
        out
    }
}

/// File-bound prerequisites supplement the canonical live SQL filter definitions (INTEG-015).
pub fn orphan_state(
    def: &crate::defs::FilterDef,
    tab: &crate::defs::TabDef,
    cfg: &CrawlConfig,
    cfg_json: &serde_json::Value,
    analysis: &crate::state::AnalysisSnapshot,
) -> Option<os_model::FilterState> {
    use os_model::{AnalysisStep, FilterState};
    if def.id != "Orphan URLs" {
        return None;
    }
    let (on, key) = match tab.id {
        "Analytics" => (cfg.api.ga4.crawl_new_urls, "api.ga4.crawl_new_urls"),
        "Search Console" => (cfg.api.gsc.crawl_new_urls, "api.gsc.crawl_new_urls"),
        _ => return None,
    };
    if !on {
        return Some(FilterState::NeedsConfig(key.into()));
    }
    if !cfg.analysis.step_enabled(AnalysisStep::Sitemaps) {
        return Some(FilterState::NeedsCrawlAnalysis);
    }
    Some(match analysis.freshness(AnalysisStep::Sitemaps, cfg_json) {
        crate::state::Freshness::Fresh => FilterState::Ready,
        crate::state::Freshness::Stale => FilterState::Stale,
        crate::state::Freshness::Missing => FilterState::NeedsCrawlAnalysis,
    })
}
