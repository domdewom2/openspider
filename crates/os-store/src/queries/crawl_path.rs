//! Herkunftspfad nach K2-42: erste Entdeckung, spätere Tiefenverbesserungen ändern ihn nicht.
use std::collections::{HashMap, HashSet, VecDeque};

use os_model::{UrlId, Value};
use rusqlite::OptionalExtension;

use crate::{CrawlFile, StoreError};

pub const HEADER: &[&str] = &[
    "Target Address",
    "Step",
    "Address",
    "Status Code",
    "Link Type",
    "Anchor",
    "Crawl Depth",
];

/// Detaildaten plus Hinweise, die ein Export an den Aufrufer weitergeben kann.
pub struct PathData {
    pub data: super::DetailData,
    pub notes: Vec<String>,
}

#[derive(Clone)]
struct Edge {
    source: Option<i64>,
    kind: String,
    link_type: Option<String>,
    anchor: Option<String>,
    depth: Option<i64>,
}

pub fn detail_rows(
    file: &CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
) -> Result<super::DetailData, StoreError> {
    Ok(paths(file, ids, offset, limit)?.data)
}

/// Auswahlreihenfolge bleibt erhalten; Paging bezieht sich auf den gesamten Zeilenstrom.
pub fn paths(
    file: &CrawlFile,
    ids: &[UrlId],
    offset: u64,
    limit: u64,
) -> Result<PathData, StoreError> {
    let conn = file.conn();
    let legacy = !conn.query_row("SELECT EXISTS(SELECT 1 FROM discovery_edges)", [], |r| {
        r.get::<_, bool>(0)
    })?;
    let mut notes = Vec::new();
    if legacy {
        notes.push("Herkunftsdaten fehlen; Rückfall auf BFS über links".into());
    }
    let fallback = if legacy {
        Some(legacy_edges(file)?)
    } else {
        None
    };
    let mut rows = Vec::new();
    let mut total = 0u64;
    for id in ids {
        let Some(target) = conn
            .query_row("SELECT address FROM urls WHERE id=?1", [id.0], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
        else {
            continue;
        };
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(id.0);
        let mut complete = false;
        while let Some(current) = cursor {
            if !seen.insert(current) {
                notes.push(format!("{target}: Zyklus in Herkunftsdaten"));
                break;
            }
            let edge = if let Some(edges) = &fallback {
                edges.get(&current).cloned()
            } else {
                conn.query_row("SELECT source_id, kind, link_type, anchor, depth FROM discovery_edges WHERE target_id=?1 ORDER BY seq LIMIT 1", [current], |r| Ok(Edge { source: r.get(0)?, kind: r.get(1)?, link_type: r.get(2)?, anchor: r.get(3)?, depth: r.get(4)? })).optional()?
            };
            let Some(mut edge) = edge else { break };
            // F05 schreibt Sitemap-Entdeckungen ohne source_id. Die Sitemap-Zuordnung ist die Quelle.
            if edge.kind == "sitemap" && edge.source.is_none() {
                let sm = conn
                    .query_row(
                        "SELECT s.url, COALESCE(s.status_code,u.status_code)
                     FROM sitemap_entries e JOIN sitemaps s ON s.id=e.sitemap_id
                     LEFT JOIN urls u ON u.id=s.url_id WHERE e.url_id=?1 ORDER BY s.id LIMIT 1",
                        [current],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)),
                    )
                    .optional()?;
                if let Some((address, status)) = sm {
                    edge.link_type = Some("XML Sitemap".into());
                    chain.push((current, edge.clone(), None));
                    chain.push((
                        0,
                        Edge {
                            source: None,
                            kind: "start".into(),
                            link_type: None,
                            anchor: None,
                            depth: edge.depth.map(|d| d.saturating_sub(1)),
                        },
                        Some((address, status)),
                    ));
                    complete = true;
                    break;
                }
            }
            cursor = edge.source;
            complete = cursor.is_none() && matches!(edge.kind.as_str(), "start" | "list_input");
            chain.push((current, edge, None));
        }
        if !complete {
            notes.push(format!("{target}: nicht über Links erreicht"));
            continue;
        }
        for (step, (current, edge, synthetic)) in chain.into_iter().enumerate() {
            let (address, status) = match synthetic {
                Some(v) => v,
                None => conn.query_row(
                    "SELECT address, status_code FROM urls WHERE id=?1",
                    [current],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)),
                )?,
            };
            if total >= offset && (rows.len() as u64) < limit {
                rows.push(vec![
                    Value::Text(target.clone()),
                    Value::Int(step as i64),
                    Value::Text(address),
                    status.map_or(Value::Null, Value::Int),
                    edge.link_type.map_or(Value::Null, Value::Text),
                    edge.anchor.map_or(Value::Null, Value::Text),
                    edge.depth.map_or(Value::Null, Value::Int),
                ]);
            }
            total += 1;
        }
    }
    Ok(PathData {
        data: super::DetailData {
            columns: HEADER.iter().map(|s| (*s).into()).collect(),
            rows,
            total,
        },
        notes,
    })
}

/// Nur alte Dateien ohne Herkunftskanten: BFS, Gleichstand nach gespeicherter Linkreihenfolge.
fn legacy_edges(file: &CrawlFile) -> Result<HashMap<i64, Edge>, StoreError> {
    let mut edges = HashMap::new();
    let mut queue = VecDeque::new();
    let mut roots = file.conn().prepare("SELECT id, discovered_via FROM urls WHERE discovered_via IN ('start','list_input') ORDER BY id")?;
    for row in roots.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (id, kind) = row?;
        edges.insert(
            id,
            Edge {
                source: None,
                kind,
                link_type: None,
                anchor: None,
                depth: Some(0),
            },
        );
        queue.push_back(id);
    }
    let mut links = file.conn().prepare("SELECT target_id, link_type, anchor FROM links WHERE source_id=?1 AND target_id IS NOT NULL ORDER BY position, id")?;
    while let Some(source) = queue.pop_front() {
        let depth = edges[&source].depth.map(|d| d + 1);
        for row in links.query_map([source], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (target, link_type, anchor) = row?;
            if let std::collections::hash_map::Entry::Vacant(entry) = edges.entry(target) {
                entry.insert(Edge {
                    source: Some(source),
                    kind: "link".into(),
                    link_type: Some(link_type),
                    anchor,
                    depth,
                });
                queue.push_back(target);
            }
        }
    }
    // Nicht über Hyperlinks erreichbare Sitemap-Einträge bleiben auch im Altformat erklärbar.
    let mut sitemaps = file.conn().prepare("SELECT DISTINCT e.url_id, u.crawl_depth FROM sitemap_entries e JOIN urls u ON u.id=e.url_id ORDER BY e.url_id")?;
    for row in sitemaps.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
    })? {
        let (id, depth) = row?;
        edges.entry(id).or_insert(Edge {
            source: None,
            kind: "sitemap".into(),
            link_type: Some("XML Sitemap".into()),
            anchor: None,
            depth,
        });
    }
    Ok(edges)
}
