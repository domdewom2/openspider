//! Additive read helpers for the app's paged link tables and overview (K2-61).
use super::*;
use os_model::Value;

/// Restrictions applied before counting and paging. Empty URL selections match no links.
#[derive(Debug, Clone, Default)]
pub struct LinkSelection {
    pub url_ids: Vec<UrlId>,
    /// Optional explicit link row IDs, intersected with every view predicate before paging.
    pub row_ids: Option<Vec<i64>>,
    pub incoming: bool,
    pub resources: bool,
    pub exclude_resources: bool,
    pub link_type: Option<String>,
    pub exclude_self: bool,
    pub exclude_nofollow: bool,
    pub internal: Option<bool>,
    pub uncrawlable_only: bool,
    pub include_uncrawlable: bool,
    pub anchor: Option<String>,
    pub search: Option<String>,
}

/// One page, including stable link IDs and the complete standard link format.
pub struct LinkTablePage {
    pub keys: Vec<i64>,
    pub rows: Vec<Vec<Value>>,
    pub total: u64,
}

/// All predicates precede both COUNT and LIMIT; duplicate selected URLs never duplicate links.
pub fn selected_links(
    conn: &Connection,
    selection: &LinkSelection,
    page: &PageReq,
) -> Result<LinkTablePage, StoreError> {
    let ids = serde_json::to_string(&selection.url_ids.iter().map(|id| id.0).collect::<Vec<_>>())
        .expect("integer IDs");
    let row_ids = selection
        .row_ids
        .as_ref()
        .map(|ids| serde_json::to_string(ids).expect("integer IDs"));
    let direction = if selection.incoming {
        Direction::In
    } else {
        Direction::Out
    };
    let (order, _) = order_by(direction, &page.sort)?;
    let side = if selection.incoming {
        "l.target_id"
    } else {
        "l.source_id"
    };
    let resource = "l.link_type IN ('Image','CSS','JavaScript','Iframe','Media')";
    let base = format!("FROM links l JOIN urls s ON s.id=l.source_id LEFT JOIN urls t ON t.id=l.target_id
        WHERE {side} IN (SELECT value FROM json_each(?1))
        AND (?2 IS NULL OR l.link_position=?2)
        AND (?3 IS NULL OR l.link_type=?3)
        AND (?4=0 OR l.source_id IS NOT l.target_id)
        AND (?5=0 OR l.follow=1)
        AND (?6 IS NULL OR t.is_internal=?6)
        AND (?7=0 OR l.crawlable=0)
        AND (?8=1 OR l.crawlable=1)
        AND (?9 IS NULL OR COALESCE(l.anchor,l.alt_text,'')=?9)
        AND (?10 IS NULL OR instr(lower(s.address || ' ' || COALESCE(t.address,l.target_raw) || ' ' || COALESCE(l.anchor,'') || ' ' || COALESCE(l.alt_text,'')),lower(?10))>0)
        AND (?11=0 OR {resource}) AND (?12=0 OR NOT ({resource}))
        AND (?13 IS NULL OR l.id IN (SELECT value FROM json_each(?13)))");
    let params = params![
        ids,
        page.position,
        selection.link_type,
        selection.exclude_self,
        selection.exclude_nofollow,
        selection.internal,
        selection.uncrawlable_only,
        selection.include_uncrawlable && !selection.incoming,
        selection.anchor,
        selection.search,
        selection.resources,
        selection.exclude_resources,
        row_ids
    ];
    let total = conn.query_row(&format!("SELECT COUNT(*) {base}"), params, |r| r.get(0))?;
    let mut stmt = conn.prepare(&format!(
        "SELECT l.id {base} ORDER BY {order} LIMIT {} OFFSET {}",
        page.limit.clamp(1, 500),
        page.offset.min(i64::MAX as u64)
    ))?;
    let keys = stmt
        .query_map(params, |r| r.get(0))?
        .collect::<Result<Vec<i64>, _>>()?;
    let views = link_views_by_ids(conn, &keys)?;
    // Batch the extra columns once for the page instead of executing one query per link.
    let key_json = serde_json::to_string(&keys).expect("integer IDs");
    let mut extras = conn.prepare("SELECT l.id,t.size_bytes,l.rel,l.path_type,l.link_origin FROM links l LEFT JOIN urls t ON t.id=l.target_id WHERE l.id IN (SELECT value FROM json_each(?1))")?;
    let mut extras = extras
        .query_map([key_json], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    let mut rows = Vec::with_capacity(keys.len());
    for (id, link) in keys.iter().zip(views) {
        let (size, rel, path, origin) = extras
            .remove(id)
            .ok_or_else(|| StoreError::InvalidInput("Missing selected link".into()))?;
        let crawlability = if selection.include_uncrawlable {
            Value::from(if link.crawlable {
                "Crawlable"
            } else {
                "Uncrawlable"
            })
        } else {
            Value::Null
        };
        rows.push(vec![
            Value::from(link.link_type.as_str()),
            Value::from(link.source_address),
            Value::from(link.target_address),
            size.into(),
            link.alt_text.into(),
            link.anchor.into(),
            if link.crawlable {
                link.status_code.map(i64::from).into()
            } else {
                Value::Null
            },
            if link.crawlable {
                link.status.into()
            } else {
                Value::from("Uncrawlable")
            },
            link.follow.into(),
            link.target_attr.into(),
            rel.into(),
            path.into(),
            link.link_path.into(),
            link.link_position.into(),
            origin.into(),
            crawlability,
            link.uncrawlable_reason.into(),
            link.original_url.into(),
            link.pdf_page.map(i64::from).into(),
        ]);
    }
    Ok(LinkTablePage { keys, rows, total })
}

/// The nine stable UI-061 summary counters, including hidden encountered URLs.
pub fn overview_summary(conn: &Connection) -> Result<Vec<(String, u64)>, StoreError> {
    let definitions = [
        ("Total URLs Encountered", "1"),
        ("Total URLs Crawled", "state='done' AND robots_blocked=0"),
        ("Total Internal Blocked by robots.txt", "is_internal=1 AND robots_blocked=1"),
        ("Total External Blocked by robots.txt", "is_internal=0 AND robots_blocked=1"),
        ("Total URLs Displayed", "in_tables=1"),
        ("Total Internal URLs", "is_internal=1 AND in_tables=1"),
        ("Total External URLs", "is_internal=0 AND in_tables=1"),
        ("Total Internal Indexable URLs", "is_internal=1 AND content_class='HTML' AND status_code>0 AND indexability='Indexable'"),
        ("Total Internal Non-Indexable URLs", "is_internal=1 AND content_class='HTML' AND status_code>0 AND indexability='Non-Indexable'"),
    ];
    let expressions = definitions
        .iter()
        .map(|(_, p)| format!("COALESCE(SUM(CASE WHEN {p} THEN 1 ELSE 0 END),0)"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(
        conn.query_row(&format!("SELECT {expressions} FROM urls"), [], |r| {
            definitions
                .iter()
                .enumerate()
                .map(|(i, (name, _))| Ok((name.to_string(), r.get(i)?)))
                .collect()
        })?,
    )
}

/// HTTP version of this response, including redirects. No request is performed.
pub fn http_status_line(conn: &Connection, id: UrlId) -> Result<String, StoreError> {
    Ok(conn.query_row("SELECT COALESCE(http_version,'') || CASE WHEN status_code IS NULL THEN '' ELSE ' ' || status_code END FROM urls WHERE id=?1", [id.0], |r| r.get(0)).optional()?.unwrap_or_default())
}

/// Resolve the system timezone at a particular instant, including historical DST.
/// Call on a read-pool worker. No process-start offset or timezone mutation is used.
pub fn local_offset_seconds(conn: &Connection, at: os_model::Timestamp) -> Result<i32, StoreError> {
    conn.query_row(
        "SELECT CAST(strftime('%s', ?1, 'localtime') AS INTEGER) - CAST(strftime('%s', ?1) AS INTEGER)",
        [at.to_db()],
        |row| row.get(0),
    )
    .map_err(StoreError::from)
}

/// Additiv (K2-61, M2-K05): alle Link-IDs einer Auswahl in der Reihenfolge von [`selected_links`], für
/// materialisierte Detail-Views (K-1g, 1 Mio. Inlinks). Die Fenster liest die App danach über `row_ids`, ohne
/// erneutes Zählen und ohne `OFFSET`. Prädikate und Sortierung müssen mit `selected_links` übereinstimmen.
pub fn selected_link_ids(
    conn: &Connection,
    selection: &LinkSelection,
    page: &PageReq,
) -> Result<Vec<i64>, StoreError> {
    let ids = serde_json::to_string(&selection.url_ids.iter().map(|id| id.0).collect::<Vec<_>>())
        .expect("integer IDs");
    let row_ids = selection
        .row_ids
        .as_ref()
        .map(|ids| serde_json::to_string(ids).expect("integer IDs"));
    let direction = if selection.incoming {
        Direction::In
    } else {
        Direction::Out
    };
    let (order, _) = order_by(direction, &page.sort)?;
    let side = if selection.incoming {
        "l.target_id"
    } else {
        "l.source_id"
    };
    let resource = "l.link_type IN ('Image','CSS','JavaScript','Iframe','Media')";
    let mut stmt = conn.prepare(&format!("SELECT l.id FROM links l JOIN urls s ON s.id=l.source_id LEFT JOIN urls t ON t.id=l.target_id
        WHERE {side} IN (SELECT value FROM json_each(?1))
        AND (?2 IS NULL OR l.link_position=?2)
        AND (?3 IS NULL OR l.link_type=?3)
        AND (?4=0 OR l.source_id IS NOT l.target_id)
        AND (?5=0 OR l.follow=1)
        AND (?6 IS NULL OR t.is_internal=?6)
        AND (?7=0 OR l.crawlable=0)
        AND (?8=1 OR l.crawlable=1)
        AND (?9 IS NULL OR COALESCE(l.anchor,l.alt_text,'')=?9)
        AND (?10 IS NULL OR instr(lower(s.address || ' ' || COALESCE(t.address,l.target_raw) || ' ' || COALESCE(l.anchor,'') || ' ' || COALESCE(l.alt_text,'')),lower(?10))>0)
        AND (?11=0 OR {resource}) AND (?12=0 OR NOT ({resource}))
        AND (?13 IS NULL OR l.id IN (SELECT value FROM json_each(?13)))
        ORDER BY {order}"))?;
    let keys = stmt
        .query_map(
            params![
                ids,
                page.position,
                selection.link_type,
                selection.exclude_self,
                selection.exclude_nofollow,
                selection.internal,
                selection.uncrawlable_only,
                selection.include_uncrawlable && !selection.incoming,
                selection.anchor,
                selection.search,
                selection.resources,
                selection.exclude_resources,
                row_ids
            ],
            |r| r.get(0),
        )?
        .collect::<Result<Vec<i64>, _>>()?;
    Ok(keys)
}

/// Additiv (K2-61, M2-K05): Zeilen bekannter Link-IDs in der übergebenen Reihenfolge, im Format von
/// [`selected_links`] (Primärschlüsselzugriff, kein `COUNT`, kein `OFFSET`). Für Fenster materialisierter
/// Detail-Views aus [`selected_link_ids`]; `total` ist die Anzahl der gelieferten Zeilen. Gelöschte Links fehlen.
pub fn selected_links_by_ids(
    conn: &Connection,
    selection: &LinkSelection,
    ids: &[i64],
) -> Result<LinkTablePage, StoreError> {
    let present = serde_json::to_string(ids).expect("integer IDs");
    let mut stmt =
        conn.prepare("SELECT l.id FROM links l WHERE l.id IN (SELECT value FROM json_each(?1))")?;
    let found = stmt
        .query_map([present], |r| r.get::<_, i64>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    let keys = ids
        .iter()
        .copied()
        .filter(|id| found.contains(id))
        .collect::<Vec<_>>();
    let total = keys.len() as u64;
    let views = link_views_by_ids(conn, &keys)?;
    // Batch the extra columns once for the page instead of executing one query per link.
    let key_json = serde_json::to_string(&keys).expect("integer IDs");
    let mut extras = conn.prepare("SELECT l.id,t.size_bytes,l.rel,l.path_type,l.link_origin FROM links l LEFT JOIN urls t ON t.id=l.target_id WHERE l.id IN (SELECT value FROM json_each(?1))")?;
    let mut extras = extras
        .query_map([key_json], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    let mut rows = Vec::with_capacity(keys.len());
    for (id, link) in keys.iter().zip(views) {
        let (size, rel, path, origin) = extras
            .remove(id)
            .ok_or_else(|| StoreError::InvalidInput("Missing selected link".into()))?;
        let crawlability = if selection.include_uncrawlable {
            Value::from(if link.crawlable {
                "Crawlable"
            } else {
                "Uncrawlable"
            })
        } else {
            Value::Null
        };
        rows.push(vec![
            Value::from(link.link_type.as_str()),
            Value::from(link.source_address),
            Value::from(link.target_address),
            size.into(),
            link.alt_text.into(),
            link.anchor.into(),
            if link.crawlable {
                link.status_code.map(i64::from).into()
            } else {
                Value::Null
            },
            if link.crawlable {
                link.status.into()
            } else {
                Value::from("Uncrawlable")
            },
            link.follow.into(),
            link.target_attr.into(),
            rel.into(),
            path.into(),
            link.link_path.into(),
            link.link_position.into(),
            origin.into(),
            crawlability,
            link.uncrawlable_reason.into(),
            link.original_url.into(),
            link.pdf_page.map(i64::from).into(),
        ]);
    }
    Ok(LinkTablePage { keys, rows, total })
}

/// Stored deterministic MinHash signature, used by comparison projections without loading page bodies (TECH-301).
pub fn comparison_signature(conn: &Connection, id: UrlId) -> Result<Option<Vec<u32>>, StoreError> {
    let bytes: Option<Vec<u8>> = conn
        .query_row("SELECT sig FROM minhash WHERE url_id=?1", [id.0], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(bytes.and_then(|b| crate::writer_m2::decode_minhash(&b)))
}
