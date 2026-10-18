//! Indexierbarkeit und Respect-Sichtbarkeit ohne erneuten Abruf (TECH-068, TECH-051).
use crate::EngineError;
use os_model::{
    CancelToken, CrawlConfig, FetchFacts, IndexabilityInput, IndexabilityStatus, PageFacts,
    RefreshRef, RefreshSource, UrlId,
};
use os_store::{CrawlFile, StoreError};

fn sql<T, E>(result: Result<T, E>) -> Result<T, EngineError>
where
    StoreError: From<E>,
{
    result.map_err(|e| EngineError::Store(StoreError::from(e)))
}

pub fn run(
    file: &mut CrawlFile,
    old: &CrawlConfig,
    new: &CrawlConfig,
    cancel: &CancelToken,
) -> Result<Vec<UrlId>, EngineError> {
    if old.advanced.respect_noindex == new.advanced.respect_noindex
        && old.advanced.respect_canonical == new.advanced.respect_canonical
        && old.advanced.respect_self_ref_meta_refresh == new.advanced.respect_self_ref_meta_refresh
    {
        return Ok(Vec::new());
    }
    cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
    // Eine Transaktion: Abbruch verwirft auch Sichtbarkeit, Marker und Änderungsfeed.
    let tx = sql(file.conn().unchecked_transaction())?;
    let seq = file
        .get_meta("change_seq")?
        .unwrap_or_default()
        .parse::<i64>()
        .unwrap_or(0)
        + 1;
    let mut changed = Vec::new();
    let mut rows_stmt = sql(tx.prepare(
        "SELECT id, url, COALESCE(status_code, 0), robots_blocked, no_response_reason,
                effective_index, canonical_url, indexability, indexability_status, in_tables,
                EXISTS (SELECT 1 FROM url_flags f WHERE f.url_id = urls.id AND f.flag = 'respect_hidden')
         FROM urls WHERE state = 'done' ORDER BY id",
    ))?;
    let mut rows = sql(rows_stmt.query([]))?;
    let mut refresh_stmt = sql(tx.prepare(
        "SELECT source, raw, delay, target_raw, target_url, in_head, position
         FROM refresh WHERE url_id = ?1 ORDER BY CASE source WHEN 'header' THEN 0 ELSE 1 END, position",
    ))?;
    while let Some(row) = sql(rows.next())? {
        cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
        let id: i64 = sql(row.get(0))?;
        let page_url: String = sql(row.get(1))?;
        let fetch = FetchFacts {
            status_code: sql(row.get(2))?,
            robots_blocked: sql(row.get(3))?,
            no_response_reason: sql(row.get::<_, Option<String>>(4))?.and_then(|s| s.parse().ok()),
            ..Default::default()
        };
        let page = PageFacts {
            title: Default::default(),
            meta_description: Default::default(),
            meta_keywords: Default::default(),
            h1: Default::default(),
            h2: Default::default(),
            word_count: None,
            text_ratio: None,
            canonical_count: 0,
            canonical_url: sql(row.get(6))?,
            canonical_target: None,
            canonical_self: None,
            effective_index: sql(row.get::<_, Option<bool>>(5))?.unwrap_or(true),
            effective_follow: true,
            has_rel_next: false,
            has_rel_prev: false,
            hreflang_count: 0,
        };
        let refresh = sql(refresh_stmt.query_map([id], |r| {
            Ok(RefreshRef {
                source: if r.get::<_, String>(0)? == "header" {
                    RefreshSource::Header
                } else {
                    RefreshSource::Meta
                },
                raw: r.get(1)?,
                delay: r.get(2)?,
                target_raw: r.get(3)?,
                target: r.get(4)?,
                in_head: r.get(5)?,
                order: r.get(6)?,
                xpath: None,
            })
        }))?;
        let refresh: Vec<RefreshRef> = sql(refresh.collect())?;
        let (indexability, status) = os_model::evaluate(&IndexabilityInput {
            fetch: &fetch,
            page: Some(&page),
            page_url: &page_url,
            refresh: &refresh,
            cfg: new,
            same_url: os_url::eq_for_compare,
        });
        let was_visible: bool = sql(row.get(9))?;
        let was_hidden: bool = sql(row.get(10))?;
        // Nur Respect-eigene Ausblendungen restaurieren; Speicher-/Scope-Schalter bleiben erhalten.
        let hidden = (was_visible || was_hidden)
            && ((new.advanced.respect_noindex && status == Some(IndexabilityStatus::Noindex))
                || (new.advanced.respect_canonical
                    && status == Some(IndexabilityStatus::Canonicalised)));
        let visible = (was_visible || was_hidden) && !hidden;
        let previous: Option<String> = sql(row.get(7))?;
        let previous_status: Option<String> = sql(row.get(8))?;
        let status = status.map(|s| s.as_str());
        if previous.as_deref() == Some(indexability.as_str())
            && previous_status.as_deref() == status
            && was_visible == visible
            && was_hidden == hidden
        {
            continue;
        }
        sql(tx.execute("UPDATE urls SET indexability = ?2, indexability_status = ?3, in_tables = ?4, change_seq = ?5 WHERE id = ?1",
            (id, indexability.as_str(), status, visible, seq)))?;
        if hidden {
            sql(tx.execute("INSERT OR REPLACE INTO url_flags(url_id, flag, value) VALUES (?1, 'respect_hidden', '1')", [id]))?;
        } else {
            sql(tx.execute(
                "DELETE FROM url_flags WHERE url_id = ?1 AND flag = 'respect_hidden'",
                [id],
            ))?;
        }
        changed.push(UrlId(id));
    }
    drop(rows);
    drop(rows_stmt);
    drop(refresh_stmt);
    if !changed.is_empty() {
        // Abhängige Canonical-/Pagination-/hreflang-Ansichten müssen denselben Feed sehen.
        let mut dependants = std::collections::BTreeSet::new();
        let mut stmt = sql(tx.prepare_cached("SELECT url_id FROM pagination WHERE target_id = ?1 UNION SELECT url_id FROM canonicals WHERE target_id = ?1 UNION SELECT url_id FROM hreflang WHERE target_id = ?1"))?;
        for id in &changed {
            let ids = sql(stmt.query_map([id.0], |r| r.get::<_, i64>(0)))?;
            for id in ids {
                dependants.insert(UrlId(sql(id)?));
            }
        }
        drop(stmt);
        for id in &dependants {
            sql(tx.execute("UPDATE urls SET change_seq = ?1 WHERE id = ?2", (seq, id.0)))?;
        }
        changed.extend(dependants);
        changed.sort_unstable();
        changed.dedup();
        file.set_meta("change_seq", &seq.to_string())?;
        crate::rescan::mark_analysis_stale(file)?;
    }
    cancel.checkpoint().map_err(|_| EngineError::Cancelled)?;
    sql(tx.commit())?;
    Ok(changed)
}
