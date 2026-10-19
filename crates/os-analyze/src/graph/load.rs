use super::{Node, Record, Snapshot, Tree};
use os_store::{CrawlFile, StoreError};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

pub(super) fn capture(file: &CrawlFile) -> Result<Snapshot, StoreError> {
    let tx = file.conn().unchecked_transaction()?;
    check_capture_limits(file)?;
    let mut tree = Tree::default();
    let mut stmt = tx.prepare(
        "SELECT u.id, u.address, u.content_class, u.is_internal, u.crawl_depth,
         u.status_code, u.indexability, c.unique_inlinks, u.word_count, u.response_time_ms
         FROM urls u LEFT JOIN url_counters c ON c.url_id = u.id WHERE u.in_tables = 1 ORDER BY u.id",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id = row.get(0)?;
        let address: String = row.get(1)?;
        let depth: Option<u32> = row.get(4)?;
        let inlinks: Option<u64> = row.get(7)?;
        let mut metrics = BTreeMap::new();
        for (key, value) in [
            ("crawl_depth", depth.map(f64::from)),
            ("unique_inlinks", inlinks.map(|n| n as f64)),
            ("word_count", row.get::<_, Option<f64>>(8)?),
            (
                "response_time",
                row.get::<_, Option<f64>>(9)?.map(|n| n / 1000.0),
            ),
        ] {
            if let Some(value) = value {
                metrics.insert(key.into(), value);
            }
        }
        tree.nodes.insert(
            id,
            Record {
                html: row.get::<_, String>(2)? == "HTML",
                internal: row.get(3)?,
                metrics,
                node: Node {
                    id,
                    label: path_label(&address),
                    address: Some(address),
                    depth: depth.unwrap_or(0),
                    status_code: row.get(5)?,
                    indexability: row.get(6)?,
                    unique_inlinks: inlinks,
                    link_score: None,
                    segment_id: None,
                    segment_color: None,
                    metric: None,
                    color_value: None,
                    hidden_children: 0,
                    hidden_descendants: 0,
                },
            },
        );
    }
    drop(rows);
    drop(stmt);
    for (sql, custom) in [
        ("SELECT url_id, key, COALESCE(v_real, v_int) FROM url_ext WHERE v_real IS NOT NULL OR v_int IS NOT NULL ORDER BY url_id, key", false),
        ("SELECT url_id, source || '.' || metric, v_real FROM api_values WHERE level = 'url' AND v_real IS NOT NULL ORDER BY url_id, source, metric", false),
        ("SELECT url_id, 'custom_extraction.' || def_index || '.' || number, value FROM custom_extraction_values WHERE error IS NULL ORDER BY url_id, def_index, number", true),
    ] {
        let mut stmt = tx.prepare(sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let key: String = row.get(1)?;
            let value = if custom { row.get::<_, String>(2)?.parse::<f64>().ok() } else { row.get(2)? };
            if let (Some(record), Some(value)) = (tree.nodes.get_mut(&id), value.filter(|n| n.is_finite())) {
                record.metrics.entry(key.clone()).or_insert(value);
                if custom {
                    // Ersten Zahlenwert als Kurznamen der Definition erhalten;
                    // nummerierte Extraktionsspalten zusätzlich einzeln anbieten.
                    if let Some((definition, _)) = key.rsplit_once('.') {
                        record.metrics.entry(definition.into()).or_insert(value);
                    }
                }
                if matches!(key.as_str(), "linkscore.score" | "linkscore.link_score" | "link_score") {
                    record.node.link_score = Some(value);
                    record.metrics.insert("link_score".into(), value);
                }
            }
        }
    }
    let config = file.config()?;
    let mut stmt =
        tx.prepare("SELECT url_id, MIN(segment_id) FROM url_segments GROUP BY url_id")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, usize>(1)?)))? {
        let (id, segment) = row?;
        if let Some(record) = tree.nodes.get_mut(&id) {
            record.node.segment_id = Some(segment.to_string());
            record.node.segment_color = config.segments.list.get(segment).map(|s| s.color.clone());
        }
    }
    drop(stmt);
    let has_discovery: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'discovery_edges')", [], |r| r.get(0),
    )?;
    let count: u64 = if has_discovery {
        tx.query_row("SELECT COUNT(*) FROM discovery_edges", [], |r| r.get(0))?
    } else {
        0
    };
    let provenance_missing = count == 0 && !tree.nodes.is_empty();
    let sql = if provenance_missing {
        "SELECT source_id, target_id, id FROM links WHERE target_id IS NOT NULL AND link_type IN ('Hyperlink', 'HTTP Redirect', 'Meta Refresh', 'Image', 'CSS', 'JavaScript', 'Iframe') ORDER BY id"
    } else if has_discovery {
        "SELECT source_id, target_id, seq FROM discovery_edges ORDER BY seq"
    } else {
        "SELECT NULL, NULL, NULL WHERE 0"
    };
    let mut adjacency: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    let mut seeds = Vec::new();
    let mut stmt = tx.prepare(sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let source: Option<i64> = row.get(0)?;
        let target: i64 = row.get(1)?;
        let seq: i64 = row.get(2)?;
        if !tree.nodes.contains_key(&target) {
            continue;
        }
        match source.filter(|s| tree.nodes.contains_key(s)) {
            Some(source) => adjacency.entry(source).or_default().push((target, seq)),
            None => seeds.push((seq, target)),
        }
    }
    drop(rows);
    drop(stmt);
    let start = file.get_meta("start_url")?;
    let root = start.and_then(|start| {
        tree.nodes
            .values()
            .find(|r| r.node.address.as_ref() == Some(&start))
            .map(|r| r.node.id)
    });
    if let Some(root) = root {
        seeds.insert(0, (i64::MIN, root));
    }
    if seeds.is_empty() {
        if let Some(id) = tree.nodes.keys().next() {
            seeds.push((0, *id));
        }
    }
    let mut heap = BinaryHeap::new();
    for (seq, id) in seeds {
        heap.push(Reverse((0_u32, seq, id, None)));
    }
    let mut visited = std::collections::HashSet::new();
    let mut remaining = tree.nodes.keys().copied().collect::<Vec<_>>().into_iter();
    loop {
        while let Some(Reverse((depth, _, id, parent))) = heap.pop() {
            if !visited.insert(id) {
                continue;
            }
            let record = tree.nodes.get_mut(&id).expect("known node");
            record.node.depth = depth;
            record
                .metrics
                .insert("crawl_depth".into(), f64::from(depth));
            if let Some(parent) = parent {
                tree.children.entry(parent).or_default().push(id);
            } else {
                tree.roots.push(id);
            }
            for (child, seq) in adjacency.get(&id).into_iter().flatten() {
                heap.push(Reverse((depth.saturating_add(1), *seq, *child, Some(id))));
            }
        }
        // Listenmodus, getrennte Inseln und unvollständige Altdateien bleiben als Wald sichtbar.
        match remaining.find(|id| !visited.contains(id)) {
            Some(id) => heap.push(Reverse((0, i64::MAX, id, None))),
            None => break,
        }
    }
    let directories = directories(&tree);
    let snapshot = Snapshot {
        crawl: tree,
        directories,
        anchors: snapshot_groups(file)?,
        change_seq: file
            .get_meta("change_seq")?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        reset_seq: file
            .get_meta("reset_seq")?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        provenance_missing,
    };
    tx.commit()?;
    Ok(snapshot)
}

fn path_label(address: &str) -> String {
    address
        .split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|i| rest[i..].to_owned()))
        .unwrap_or_else(|| address.to_owned())
}

fn directories(crawl: &Tree) -> Tree {
    let mut tree = Tree::default();
    let mut paths = BTreeMap::new();
    let mut next = -1_i64;
    // URLs ohne Query belegen ihren Pfadknoten; Query-Varianten werden dessen Kinder.
    let real: HashMap<_, _> = crawl
        .nodes
        .values()
        .filter_map(|r| {
            let address = r.node.address.as_ref()?;
            (!address.contains(['?', '#']))
                .then(|| (address.trim_end_matches('/').to_owned(), r.clone()))
        })
        .collect();
    for record in crawl.nodes.values() {
        let address = record.node.address.as_deref().expect("URL record");
        let Some((scheme, rest)) = address.split_once("://") else {
            continue;
        };
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let host = &rest[..end];
        let path = rest[end..].split(['?', '#']).next().unwrap_or("");
        let mut keys = vec![
            (format!("{scheme}://"), scheme.to_owned()),
            (format!("{scheme}://{host}"), host.to_owned()),
        ];
        let mut key = format!("{scheme}://{host}");
        for part in path.split('/').filter(|s| !s.is_empty()) {
            key.push('/');
            key.push_str(part);
            keys.push((key.clone(), part.to_owned()));
        }
        let mut parent = None;
        for (depth, (key, label)) in keys.into_iter().enumerate() {
            let id = *paths.entry(key.clone()).or_insert_with(|| {
                let mut r = real.get(&key).cloned().unwrap_or_else(|| {
                    let mut r = record.clone();
                    r.node.id = next;
                    next -= 1;
                    r.node.address = None;
                    r.node.status_code = None;
                    r.node.indexability = None;
                    r.node.unique_inlinks = None;
                    r.node.link_score = None;
                    r.node.segment_id = None;
                    r.node.segment_color = None;
                    r.metrics.clear();
                    r
                });
                r.node.label = label;
                r.node.depth = depth as u32;
                let id = r.node.id;
                tree.nodes.insert(id, r);
                if let Some(parent) = parent {
                    tree.children.entry(parent).or_default().push(id);
                } else {
                    tree.roots.push(id);
                }
                id
            });
            parent = Some(id);
        }
        let parent = parent.expect("scheme and host");
        if parent != record.node.id {
            let mut r = record.clone();
            r.node.depth = tree.nodes[&parent].node.depth + 1;
            tree.children.entry(parent).or_default().push(r.node.id);
            tree.nodes.insert(r.node.id, r);
        }
    }
    tree
}

/// Unveränderliche Ankergruppen mit getrennten Follow-/Selbstlink-Optionen (UI-099).
#[derive(Debug, Clone)]
pub(super) struct SnapshotGroup {
    pub target_id: i64,
    pub text: String,
    pub follow: bool,
    pub self_link: bool,
    pub count: u64,
}

/// Interne Ankerphrasen mit Alt-Text als Rückfall; N01 stellt nach D08 auf dessen Aggregation um.
fn snapshot_groups(file: &CrawlFile) -> Result<Vec<SnapshotGroup>, StoreError> {
    let mut stmt = file.conn().prepare(
        "SELECT l.target_id, l.anchor, l.alt_text, l.follow, l.source_id = l.target_id, COUNT(*)
         FROM links l JOIN urls s ON s.id = l.source_id
         WHERE s.is_internal = 1 AND l.link_type = 'Hyperlink' AND l.target_id IS NOT NULL
         GROUP BY l.target_id, l.anchor, l.alt_text, l.follow, l.source_id = l.target_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let anchor: Option<String> = r.get(1)?;
        let alt: Option<String> = r.get(2)?;
        let text = anchor
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(alt.as_deref())
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(SnapshotGroup {
            target_id: r.get(0)?,
            text: if text.is_empty() {
                "(leer)".into()
            } else {
                text
            },
            follow: r.get(3)?,
            self_link: r.get(4)?,
            count: r.get(5)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

/// Schutz vor vollständigen Millionen-Crawls und unbeschränkten Metrik-/Kantenmengen je Fenster.
fn check_capture_limits(file: &CrawlFile) -> Result<(), StoreError> {
    for (table, limit) in [
        ("urls", 50_000_u64),
        ("url_ext", 250_000),
        ("api_values", 250_000),
        ("custom_extraction_values", 250_000),
        ("url_segments", 250_000),
        ("links", 250_000),
        ("discovery_edges", 250_000),
    ] {
        let exists: bool = file.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |r| r.get(0),
        )?;
        if !exists {
            continue;
        }
        let count: u64 = file.conn().query_row(
            &format!(
                "SELECT COUNT(*) FROM (SELECT 1 FROM {table} LIMIT {})",
                limit + 1
            ),
            [],
            |r| r.get(0),
        )?;
        if count > limit {
            return Err(super::invalid(&format!(
                "Visualisierungs-Schnappschuss zu groß: {table} überschreitet {limit} Zeilen. Bitte einen kleineren Crawl öffnen."
            )));
        }
    }
    Ok(())
}
