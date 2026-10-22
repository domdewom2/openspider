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

/// Counts use the mapped pairs from the manual comparison; each pair is read once per side.
fn element_changes(
    cmp: &crate::compare::Comparison,
    cfg: &os_model::CrawlConfig,
) -> Result<Vec<Value>, String> {
    use os_model::ChangeElement;
    use rusqlite::OptionalExtension;
    fn query(cfg: &os_model::CrawlConfig) -> String {
        let sd = &cfg.extraction.structured_data;
        let types = if sd.json_ld || sd.microdata || sd.rdfa {
            format!("COALESCE((SELECT group_concat(type_path,'|') FROM sd_items s WHERE s.url_id=u.id AND ((s.format='json-ld' AND {}) OR (s.format='microdata' AND {}) OR (s.format='rdfa' AND {}))),'')",sd.json_ld as u8,sd.microdata as u8,sd.rdfa as u8)
        } else {
            "NULL".into()
        };
        format!("SELECT json_array(u.indexability,u.indexability_status),u.title_1,u.meta_desc_1,u.h1_1,CAST(u.word_count AS TEXT),CAST(u.crawl_depth AS TEXT),CAST(c.inlinks AS TEXT),CAST(c.unique_inlinks AS TEXT),CAST(c.outlinks AS TEXT),CAST(c.unique_outlinks AS TEXT),CAST(c.external_outlinks AS TEXT),CAST(c.unique_external_outlinks AS TEXT),{types},(SELECT hex(sig) FROM minhash m WHERE m.url_id=u.id) FROM urls u LEFT JOIN url_counters c ON c.url_id=u.id WHERE u.id=?1 AND u.is_internal=1 AND u.content_class='HTML' AND u.status_code>0 AND u.state='done'")
    }
    let cq = query(&cmp.current().config);
    let pq = query(&cmp.previous().config);
    let mut current = cmp
        .current()
        .file
        .conn()
        .prepare(&cq)
        .map_err(|e| e.to_string())?;
    let mut previous = cmp
        .previous()
        .file
        .conn()
        .prepare(&pq)
        .map_err(|e| e.to_string())?;
    let mut counts = [0u64; 14];
    for id in cmp.pairing().paired_current.iter() {
        let pid = cmp
            .partner(crate::compare::Side::Current, i64::from(id))
            .ok_or("Vergleichspartner fehlt")?;
        let read = |r: &rusqlite::Row<'_>| {
            (0..14)
                .map(|i| r.get::<_, Option<String>>(i))
                .collect::<Result<Vec<_>, _>>()
        };
        let a = current
            .query_row([id], read)
            .optional()
            .map_err(|e| e.to_string())?;
        let b = previous
            .query_row([pid], read)
            .optional()
            .map_err(|e| e.to_string())?;
        let (Some(a), Some(b)) = (a, b) else {
            continue;
        };
        for i in 0..14 {
            let normalize = |value: &Option<String>| match i {
                0 => {
                    let v: Vec<Option<String>> =
                        serde_json::from_str(value.as_deref().unwrap_or("[]")).unwrap_or_default();
                    let state = v.first().and_then(|s| s.as_deref()).unwrap_or("");
                    let reasons = v.get(1).and_then(|s| s.as_deref()).unwrap_or("");
                    let mut reasons = reasons
                        .split([';', ','])
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    reasons.sort();
                    reasons.dedup();
                    Some(format!("{state}:{}", reasons.join(";")))
                }
                1..=3 => Some(
                    value
                        .as_deref()
                        .unwrap_or("")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                12 => value.as_ref().map(|v| {
                    let mut types = v.split('|').filter(|s| !s.is_empty()).collect::<Vec<_>>();
                    types.sort();
                    types.dedup();
                    types.join("|")
                }),
                _ => value.clone(),
            };
            let a = normalize(&a[i]);
            let b = normalize(&b[i]);
            if i == 13 {
                if let (Some(a), Some(b)) = (a, b) {
                    if (a.is_empty() || a.len() == 1024) && (b.is_empty() || b.len() == 1024) {
                        if a.is_empty() || b.is_empty() {
                            if a != b && cfg.compare.change_detection.content_threshold_pct < 100 {
                                counts[i] += 1;
                            }
                            continue;
                        }
                        let same = a
                            .as_bytes()
                            .as_chunks::<8>()
                            .0
                            .iter()
                            .zip(b.as_bytes().as_chunks::<8>().0.iter())
                            .filter(|(x, y)| x == y)
                            .count();
                        let change = ((1.0 - same as f64 / 128.0) * 10000.0).round() / 100.0;
                        if change > f64::from(cfg.compare.change_detection.content_threshold_pct) {
                            counts[i] += 1;
                        }
                    }
                }
            } else if (i <= 3 || (a.is_some() && b.is_some())) && a != b {
                counts[i] += 1;
            }
        }
    }
    let mut rows = vec![];
    for element in &cfg.compare.change_detection.elements {
        let (i, name) = match element {
            ChangeElement::Indexability => (0, "Indexability"),
            ChangeElement::PageTitles => (1, "Page Titles"),
            ChangeElement::MetaDescription => (2, "Meta Description"),
            ChangeElement::H1 => (3, "H1"),
            ChangeElement::WordCount => (4, "Word Count"),
            ChangeElement::CrawlDepth => (5, "Crawl Depth"),
            ChangeElement::Inlinks => (6, "Inlinks"),
            ChangeElement::UniqueInlinks => (7, "Unique Inlinks"),
            ChangeElement::InternalOutlinks => (8, "Internal Outlinks"),
            ChangeElement::UniqueInternalOutlinks => (9, "Unique Internal Outlinks"),
            ChangeElement::ExternalOutlinks => (10, "External Outlinks"),
            ChangeElement::UniqueExternalOutlinks => (11, "Unique External Outlinks"),
            ChangeElement::StructuredDataUniqueTypes => (12, "Structured Data Unique Types"),
            ChangeElement::Content => (13, "Content"),
        };
        rows.push(json!({"name":name,"previous":0,"current":counts[i]}));
    }
    Ok(rows)
}
