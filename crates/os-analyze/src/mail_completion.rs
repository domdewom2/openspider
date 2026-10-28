//! Read-only completion snapshot, sharing overview/issue/compare queries (OUT-282, K2-61).
use os_store::{CrawlFile, OpenMode};
use serde_json::{json, Value};
use std::path::Path;

pub fn snapshot(
    path: &Path,
    previous: Option<&Path>,
    auto_compare: bool,
    skip_empty: bool,
) -> Result<Value, String> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).map_err(|e| e.to_string())?;
    let meta = file.meta().map_err(|e| e.to_string())?;
    let cfg = file.config().map_err(|e| e.to_string())?;
    let overview =
        os_store::queries::app_support::overview_summary(file.conn()).map_err(|e| e.to_string())?;
    let mut stmt=file.conn().prepare("SELECT COALESCE(CAST(status_code AS TEXT),'No Response'),count(*) FROM urls WHERE state='done' AND robots_blocked=0 GROUP BY status_code ORDER BY status_code").map_err(|e|e.to_string())?;
    let codes = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let counts = crate::views::filter_counts(&file, &cfg).map_err(|e| e.to_string())?;
    let issues=crate::issues::issues_from_counts(&counts).into_iter().map(|i|json!({"name":format!("{} / {}",i.tab,i.filter),"kind":i.issue_type.to_string(),"priority":i.priority.to_string(),"urls":i.count})).collect::<Vec<_>>();
    let mut changes = None;
    if auto_compare {
        if let Some(previous) = previous {
            let work = path.parent().unwrap_or(Path::new("."));
            let cancel = os_model::CancelToken::new();
            let current = crate::compare::CrawlSide::open(path, work).map_err(|e| e.to_string())?;
            let previous =
                crate::compare::CrawlSide::open(previous, work).map_err(|e| e.to_string())?;
            let mut cmp =
                crate::compare::Comparison::open(current, previous, &cfg.compare.mapping, &cancel)
                    .map_err(|e| e.to_string())?;
            let mut rows=cmp.overview(&cancel).map_err(|e|e.to_string())?.into_iter().filter_map(|f|Some(json!({"name":format!("{} / {}",f.tab,f.label),"previous":f.previous?,"current":f.current?}))).collect::<Vec<_>>();
            let p = cmp.pairing();
            rows.push(
                json!({"name":"New","previous":0,"current":(&p.current-&p.paired_current).len()}),
            );
            rows.push(json!({"name":"Missing","previous":(&p.previous-&p.paired_previous).len(),"current":0}));
            rows.extend(element_changes(&cmp, &cfg)?);
            changes = Some(rows);
        }
    }
    let start = meta.started_at.map(|t| t.to_db()).unwrap_or_default();
    let end = meta.ended_at.map(|t| t.to_db()).unwrap_or_default();
    let duration = match (meta.started_at, meta.ended_at) {
        (Some(a), Some(b)) => (b.unix_millis() - a.unix_millis()).max(0),
        _ => 0,
    };
    Ok(
        json!({"task":null,"project":meta.project,"start_url":meta.start_url.unwrap_or_default(),"started":start,"ended":end,"duration_ms":duration,"overview":overview,"response_codes":codes,"issues":issues,"auto_compare":auto_compare,"changes":changes,"skip_empty":skip_empty,"error":if meta.state==os_model::CrawlState::Failed {Some("Crawl fehlgeschlagen")}else{None}}),
    )
}

/// Use C02's public classifier, including its link counter and self-link semantics.
fn element_changes(
    cmp: &crate::compare::Comparison,
    cfg: &os_model::CrawlConfig,
) -> Result<Vec<Value>, String> {
    let config = &cfg.compare.change_detection;
    crate::compare::change::calculate(cmp, config, &os_model::CancelToken::new())
        .map_err(|e| e.to_string())?;
    Ok(crate::compare::change::counts(cmp, config, false)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|row| json!({"name":row.element,"previous":0,"current":row.change}))
        .collect())
}
