//! Auf ein Segment eingeschränkte Sidebar-Aggregate (UI-066/UI-069, Segmentfilter von `sidebar.siteStructure`).
//!
//! Additiv zum Cache in `live.rs` (K2-61): ein einmaliger Durchlauf über die URLs des Segments mit denselben
//! Beiträgen je URL, ohne Temp-Tabellen. Der Cache zählt alle URLs; ein Segment ist nur eine Teilmenge davon.
use std::collections::BTreeMap;

use os_model::CancelToken;
use os_store::{CrawlFile, StoreError};

use super::live::{contribution, SELECT};
use super::{Counts, PathCount, PathTree};
use crate::segments::SegmentFilter;

const CLASSES: [&str; 6] = [
    "2xx",
    "3xx",
    "4xx",
    "5xx",
    "Blocked by robots.txt",
    "No Response",
];

fn empty() -> Counts {
    Counts {
        response_codes: CLASSES.iter().map(|c| ((*c).to_owned(), 0)).collect(),
        ..Default::default()
    }
}

fn add(counts: &mut Counts, indexable: bool, non_indexable: bool, class: usize) {
    counts.count += 1;
    counts.indexable += u64::from(indexable);
    counts.non_indexable += u64::from(non_indexable);
    if let Some(n) = CLASSES
        .get(class)
        .and_then(|name| counts.response_codes.get_mut(*name))
    {
        *n += 1;
    }
}

struct Node {
    name: String,
    parent: String,
    counts: Counts,
}

/// Pfad- und Tiefenzähler eines Segments (oder der URLs ohne Segment).
pub struct Scoped {
    nodes: BTreeMap<String, Node>,
    children: BTreeMap<String, Vec<String>>,
    depths: BTreeMap<u32, Counts>,
}

/// Zählt die sichtbaren internen URLs des Segments.
pub fn collect(
    file: &CrawlFile,
    segment: &SegmentFilter,
    cancel: &CancelToken,
) -> Result<Scoped, StoreError> {
    let sql = format!(
        "{SELECT} u WHERE u.is_internal = 1 AND u.in_tables = 1 AND ({})",
        segment.population_sql()
    );
    let mut stmt = file.conn().prepare(&sql)?;
    let mut rows = stmt.raw_query();
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut depths: BTreeMap<u32, Counts> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        if cancel.is_cancelled() {
            return Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
                None,
            )));
        }
        let (_, c) = contribution(row)?;
        for (path, parent, name) in c.paths {
            let node = nodes.entry(path).or_insert_with(|| Node {
                name,
                parent,
                counts: empty(),
            });
            add(
                &mut node.counts,
                c.indexable,
                c.non_indexable,
                c.response_class,
            );
        }
        if let Some(depth) = c.depth {
            add(
                depths.entry(depth).or_insert_with(empty),
                c.indexable,
                c.non_indexable,
                c.response_class,
            );
        }
    }
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, node) in &nodes {
        children
            .entry(node.parent.clone())
            .or_default()
            .push(path.clone());
    }
    Ok(Scoped {
        nodes,
        children,
        depths,
    })
}

impl Scoped {
    /// Direkte Kinder, nach vollständigem Pfad sortiert (wie [`super::paths`]).
    pub fn paths(&self, parent: &str, offset: u64, limit: u32) -> Vec<PathCount> {
        self.children
            .get(parent)
            .into_iter()
            .flatten()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(limit as usize)
            .map(|path| {
                let node = &self.nodes[path];
                PathCount {
                    name: node.name.clone(),
                    path: path.clone(),
                    counts: node.counts.clone(),
                    has_children: self.children.contains_key(path),
                }
            })
            .collect()
    }

    /// Wie [`super::path_tree`]: höchstens 500 Knoten je Antwort.
    pub fn path_tree(&self, parent: &str, offset: u64, limit: u32, levels: u8) -> Vec<PathTree> {
        self.build(parent, offset, limit, levels, &mut 500)
    }

    fn build(
        &self,
        parent: &str,
        offset: u64,
        limit: u32,
        levels: u8,
        budget: &mut usize,
    ) -> Vec<PathTree> {
        let rows = self.paths(parent, offset, limit.min(*budget as u32));
        *budget -= rows.len();
        rows.into_iter()
            .map(|row| {
                let children = if levels > 1 && row.has_children && *budget > 0 {
                    self.build(&row.path, 0, limit, levels - 1, budget)
                } else {
                    Vec::new()
                };
                let more = row.has_children
                    && (children.is_empty()
                        || !self.paths(&row.path, children.len() as u64, 1).is_empty());
                PathTree {
                    row,
                    children,
                    has_more_children: more,
                }
            })
            .collect()
    }

    /// Tiefen 1–10 (Tiefe 0 fehlt bewusst, wie [`super::depths`]).
    pub fn depths(&self) -> Vec<(u32, Counts)> {
        (1..=10)
            .map(|d| (d, self.depths.get(&d).cloned().unwrap_or_default()))
            .collect()
    }
}
