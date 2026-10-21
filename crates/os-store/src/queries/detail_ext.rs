//! Additive storage readers for the K09 detail service. Callers execute them on their database worker.
use super::{serp, StoredHtml};
use crate::{CrawlFile, StoreError};
use os_model::UrlId;
use rusqlite::{params, Connection, OptionalExtension};

pub struct SerpData {
    pub address: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub edit: Option<serp::SerpEdit>,
    pub favicon: Option<i64>,
}
pub fn serp_data(conn: &Connection, id: i64) -> Result<Option<SerpData>, StoreError> {
    let row = conn
        .query_row(
            "SELECT url,title_1,meta_desc_1 FROM urls WHERE id=?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((address, title, description)) = row else {
        return Ok(None);
    };
    let favicon=conn.query_row("SELECT t.id FROM links l JOIN urls t ON t.id=l.target_id WHERE l.source_id=?1 AND l.link_type='Image' AND instr(' ' || lower(COALESCE(l.rel,'')) || ' ',' icon ')>0 AND EXISTS(SELECT 1 FROM page_html h WHERE h.url_id=t.id) ORDER BY l.position,l.id LIMIT 1",[id],|r|r.get::<_,i64>(0)).optional()?;
    Ok(Some(SerpData {
        address,
        title,
        description,
        edit: serp::load(conn, id)?,
        favicon,
    }))
}
/// Edit/reset and its feed stamp form one transaction. False means an unknown URL.
pub fn save_serp_edit(
    conn: &mut Connection,
    id: i64,
    edit: Option<&serp::SerpEdit>,
) -> Result<bool, StoreError> {
    let tx = conn.transaction()?;
    let exists = tx.query_row("SELECT EXISTS(SELECT 1 FROM urls WHERE id=?1)", [id], |r| {
        r.get::<_, bool>(0)
    })?;
    if !exists {
        return Ok(false);
    }
    match edit {
        Some(edit) => serp::save(&tx, id, edit)?,
        None => serp::reset(&tx, id)?,
    };
    tx.execute("UPDATE urls SET change_seq=(SELECT CAST(value AS INTEGER)+1 FROM crawl_meta WHERE key='change_seq') WHERE id=?1",[id])?;
    tx.execute(
        "UPDATE crawl_meta SET value=CAST(value AS INTEGER)+1 WHERE key='change_seq'",
        [],
    )?;
    tx.commit()?;
    Ok(true)
}
pub fn screenshot_summary(conn: &Connection, id: i64) -> Result<(u32, u32, u32), StoreError> {
    Ok(conn.query_row("SELECT COALESCE(MAX(width),0),COALESCE(SUM(height),0),COUNT(*) FROM screenshots WHERE url_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)
}
pub fn screenshot_tile(
    conn: &Connection,
    id: i64,
    tile: u32,
) -> Result<Option<Vec<u8>>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT png FROM screenshots WHERE url_id=?1 AND tile=?2",
            params![id, tile],
            |r| r.get(0),
        )
        .optional()?)
}
pub fn image_address(conn: &Connection, id: i64) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT url FROM urls WHERE id=?1 AND content_class='Images'",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}
pub fn stored_image(file: &CrawlFile, id: i64) -> Result<Option<(String, Vec<u8>)>, StoreError> {
    let mime = file
        .conn()
        .query_row(
            "SELECT content_type FROM urls WHERE id=?1 AND content_class='Images'",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    let Some(mime) = mime else { return Ok(None) };
    Ok(super::stored_html(file, UrlId(id))?.map(|body| (mime, body.bytes)))
}
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub stored: Option<StoredHtml>,
}
pub fn image_data(file: &CrawlFile, id: i64) -> Result<Option<ImageData>, StoreError> {
    if image_address(file.conn(), id)?.is_none() {
        return Ok(None);
    }
    let dimension = |key: &str| -> Result<u32, StoreError> {
        Ok(file
            .conn()
            .query_row(
                "SELECT v_int FROM url_ext WHERE url_id=?1 AND key=?2",
                params![id, key],
                |r| r.get::<_, Option<u32>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0))
    };
    Ok(Some(ImageData {
        width: dimension("image.real_w")?,
        height: dimension("image.real_h")?,
        stored: super::stored_html(file, UrlId(id))?,
    }))
}
pub fn content_text(conn: &Connection, id: i64) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT v_text FROM url_ext WHERE url_id=?1 AND key='content.text'",
            [id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}
pub fn url_id_by_address(conn: &Connection, address: &str) -> Result<Option<i64>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id FROM urls WHERE address=?1 ORDER BY id LIMIT 1",
            [address],
            |r| r.get(0),
        )
        .optional()?)
}
pub fn list_origin(conn: &Connection, id: i64) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT discovered_via FROM urls WHERE id=?1 AND discovered_via='list_input'",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}
pub struct SdIssue {
    pub validation: String,
    pub feature: Option<String>,
    pub severity: String,
    pub property: Option<String>,
    pub code: String,
}
pub struct SdItem {
    pub format: String,
    pub type_path: String,
    pub json: String,
    pub issues: Vec<SdIssue>,
}
pub fn sd_items(conn: &Connection, id: i64) -> Result<Vec<SdItem>, StoreError> {
    let mut stmt=conn.prepare("SELECT id,format,type_path,json FROM sd_items WHERE url_id=?1 ORDER BY CASE format WHEN 'json-ld' THEN 0 WHEN 'microdata' THEN 1 ELSE 2 END,id")?;
    let mut issues=conn.prepare("SELECT validation,feature,severity,property,code FROM sd_issues WHERE url_id=?1 AND item_id=?2 AND severity IN ('error','warning') ORDER BY id")?;
    let mut items = vec![];
    for row in stmt.query_map([id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (item_id, format, type_path, json) = row?;
        let findings = issues
            .query_map(params![id, item_id], |r| {
                Ok(SdIssue {
                    validation: r.get(0)?,
                    feature: r.get(1)?,
                    severity: r.get(2)?,
                    property: r.get(3)?,
                    code: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        items.push(SdItem {
            format,
            type_path,
            json,
            issues: findings,
        });
    }
    Ok(items)
}

/// App path reader, also accepting parentless discovery roots identified by the crawl's start URL.
/// Older/imported discovery records can retain `kind=link` on that root. The established path reader remains
/// authoritative for regular provenance, list inputs, sitemaps and legacy link-only files.
pub fn path_rows(file: &CrawlFile, id: i64) -> Result<super::DetailData, StoreError> {
    let mut data = super::crawl_path::paths(file, &[UrlId(id)], 0, u64::MAX)?.data;
    if !data.rows.is_empty() {
        return Ok(data);
    }
    let Some(start) = file.get_meta("start_url")? else {
        return Ok(data);
    };
    let mut seen = std::collections::HashSet::new();
    let mut current = Some(id);
    let mut chain = Vec::new();
    let mut reached_start = false;
    while let Some(cursor) = current {
        if !seen.insert(cursor) {
            break;
        }
        let row = file
            .conn()
            .query_row(
                "SELECT u.address,u.url,u.status_code,e.source_id,e.link_type,e.anchor,e.depth
             FROM discovery_edges e JOIN urls u ON u.id=e.target_id
             WHERE e.target_id=?1 ORDER BY e.seq LIMIT 1",
                [cursor],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((address, url, status, parent, via, anchor, depth)) = row else {
            break;
        };
        current = parent;
        reached_start = parent.is_none() && (url == start || address == start);
        chain.push((address, status, via, anchor, depth));
    }
    if !reached_start {
        return Ok(data);
    }
    let target = chain.first().map(|r| r.0.clone()).unwrap_or_default();
    let count = chain.len();
    data.rows = chain
        .into_iter()
        .enumerate()
        .map(|(i, (address, status, via, anchor, depth))| {
            use os_model::Value;
            vec![
                Value::Text(target.clone()),
                Value::Int(i as i64),
                Value::Text(address),
                status.map_or(Value::Null, Value::Int),
                via.map_or(Value::Null, Value::Text),
                anchor.map_or(Value::Null, Value::Text),
                Value::Int(depth.unwrap_or((count - i - 1) as i64)),
            ]
        })
        .collect();
    data.total = data.rows.len() as u64;
    Ok(data)
}
