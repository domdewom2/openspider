use super::{SegmentError, SegmentFilter};
use crate::search::FilterSource;
use os_model::{CancelToken, CrawlConfig};
use os_store::CrawlFile;
use roaring::RoaringBitmap;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct SegmentStats {
    pub name: Option<String>,
    pub color: Option<String>,
    pub urls: u64,
    pub percent: f64,
    pub indexable: u64,
    pub non_indexable: u64,
    pub depths: BTreeMap<String, u64>,
    pub response_codes: BTreeMap<String, u64>,
    /// Each bitmap union counts URLs once, even with multiple matching issues.
    pub issues: BTreeMap<String, u64>,
}

pub fn statistics(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Vec<SegmentStats>, SegmentError> {
    if cfg.segments.list.is_empty() {
        return Ok(Vec::new());
    }
    let total: u64 =
        file.conn()
            .query_row("SELECT COUNT(*) FROM urls WHERE is_internal=1", [], |r| {
                r.get(0)
            })?;
    let mut issue_maps: BTreeMap<String, RoaringBitmap> = BTreeMap::new();
    for entry in crate::issues::catalog() {
        if cancel.is_cancelled() {
            return Err(SegmentError::Cancelled);
        }
        let tabs = crate::tabs::configured_for_file(cfg, file);
        let Some(tab) = tabs.iter().find(|t| t.id == entry.tab) else {
            continue;
        };
        if tab.is_stub()
            || !tab
                .filters
                .iter()
                .any(|f| crate::issues::matches_filter(&entry.filter, f.id) && !f.is_stub())
        {
            continue;
        }
        let label = crate::issues::filter_label(entry, &(tab.params)(cfg));
        if crate::views::state_of_in(&tabs, file, cfg, &entry.tab, &label)?
            != os_model::FilterState::Ready
        {
            continue;
        }
        let mut query = crate::views::ViewQuery::new(&entry.tab, label);
        query.limit = 500;
        loop {
            let page = crate::views::query_view_in(&tabs, file, cfg, &query)?;
            for id in page.row_keys {
                let id =
                    u32::try_from(id.0).map_err(|e| SegmentError::Evaluation(e.to_string()))?;
                issue_maps
                    .entry(entry.issue_type.as_str().into())
                    .or_default()
                    .insert(id);
                issue_maps
                    .entry(entry.priority.as_str().into())
                    .or_default()
                    .insert(id);
            }
            match page.next_cursor {
                Some(c) => query.start = crate::views::PageStart::Cursor(c),
                None => break,
            }
        }
    }
    let mut result = Vec::new();
    for segment in cfg
        .segments
        .list
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
    {
        let scope = SegmentFilter {
            name: segment.map(|s| s.name.clone()),
        };
        let bitmap = scope.bitmap(file)?;
        let mut stats = SegmentStats {
            name: scope.name.clone(),
            color: segment.map(|s| s.color.clone()),
            urls: bitmap.len(),
            percent: if total == 0 {
                0.0
            } else {
                bitmap.len() as f64 * 100.0 / total as f64
            },
            ..Default::default()
        };
        let mut stmt=file.conn().prepare(&format!("SELECT u.indexability,u.crawl_depth,u.status_code,COUNT(*) FROM urls u WHERE u.is_internal=1 AND ({}) GROUP BY u.indexability,u.crawl_depth,u.status_code",scope.population_sql()))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let index: Option<String> = row.get(0)?;
            let depth: Option<u32> = row.get(1)?;
            let status: Option<u32> = row.get(2)?;
            let n: u64 = row.get(3)?;
            if index.as_deref() == Some("Indexable") {
                stats.indexable += n;
            } else {
                stats.non_indexable += n;
            }
            let bucket = depth
                .map(|d| if d >= 10 { "10+".into() } else { d.to_string() })
                .unwrap_or_else(|| "Unknown".into());
            *stats.depths.entry(bucket).or_default() += n;
            *stats
                .response_codes
                .entry(
                    status
                        .map(|s| format!("{}xx", s / 100))
                        .unwrap_or_else(|| "No Response".into()),
                )
                .or_default() += n;
        }
        for (key, map) in &issue_maps {
            stats
                .issues
                .insert(key.clone(), bitmap.intersection_len(map));
        }
        for key in ["Issue", "Warning", "Opportunity", "High", "Medium", "Low"] {
            stats.issues.entry(key.into()).or_insert(0);
        }
        result.push(stats);
    }
    Ok(result)
}
