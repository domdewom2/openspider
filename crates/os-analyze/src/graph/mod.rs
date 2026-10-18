//! Immutable graph snapshots (K-16). No DOM or page bodies are retained.
mod load;
mod words;

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

pub use words::{WordOptions, WordUnit};

/// Data shared by the tree and force-layout clients. Layout is a presentation concern.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: i64,
    pub label: String,
    pub address: Option<String>,
    pub depth: u32,
    pub status_code: Option<u16>,
    pub indexability: Option<String>,
    pub unique_inlinks: Option<u64>,
    pub link_score: Option<f64>,
    pub segment_id: Option<String>,
    pub segment_color: Option<String>,
    pub metric: Option<f64>,
    pub color_value: Option<f64>,
    pub hidden_children: u64,
    pub hidden_descendants: u64,
}

#[derive(Debug, Clone, Default)]
struct Tree {
    nodes: BTreeMap<i64, Record>,
    children: HashMap<i64, Vec<i64>>,
    roots: Vec<i64>,
}

#[derive(Debug, Clone)]
struct Record {
    node: Node,
    html: bool,
    internal: bool,
    metrics: BTreeMap<String, f64>,
}

/// One consistent SQLite read transaction, released before opening the window.
#[derive(Debug)]
pub struct Snapshot {
    crawl: Tree,
    directories: Tree,
    anchors: Vec<os_store::queries::anchors::SnapshotGroup>,
    pub change_seq: u64,
    pub reset_seq: u64,
    pub provenance_missing: bool,
}

/// Request limits apply to the data, independently of the chosen 2D layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphOptions {
    pub directory: bool,
    pub root: Option<i64>,
    pub budget: u32,
    pub expand_depth: u32,
    pub explore_node: Option<i64>,
    pub expanded_nodes: Vec<i64>,
    pub include_non_html: bool,
    pub include_non_indexable: bool,
    pub metric: String,
    pub color_metric: Option<String>,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            directory: false,
            root: None,
            budget: 10_000,
            expand_depth: 3,
            explore_node: None,
            expanded_nodes: Vec::new(),
            include_non_html: false,
            include_non_indexable: true,
            metric: "crawl_depth".into(),
            color_metric: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<(i64, i64)>,
    pub truncated: bool,
}

fn invalid(message: &str) -> os_store::StoreError {
    os_store::StoreError::InvalidInput(message.into())
}

pub(crate) fn metric_key(name: &str) -> String {
    match name {
        "" | "Crawl Depth" | "crawlDepth" => "crawl_depth".into(),
        "Unique Inlinks" | "uniqueInlinks" => "unique_inlinks".into(),
        "Word Count" | "wordCount" => "word_count".into(),
        "Response Time" | "responseTime" => "response_time".into(),
        "Link Score" | "linkScore" => "link_score".into(),
        "GA Sessions" => "ga4.sessions".into(),
        "GSC Clicks" => "gsc.clicks".into(),
        "GSC Impressions" => "gsc.impressions".into(),
        other => other.into(),
    }
}

impl Snapshot {
    /// Capture through the application's ReadPool, or directly in a headless caller.
    pub fn capture(file: &os_store::CrawlFile) -> Result<Self, os_store::StoreError> {
        load::capture(file)
    }

    pub fn contains_url(&self, id: i64) -> bool {
        self.crawl.nodes.contains_key(&id)
    }

    pub fn contains_node(&self, id: i64, directory: bool) -> bool {
        if directory {
            self.directories.nodes.contains_key(&id)
        } else {
            self.contains_url(id)
        }
    }

    pub fn is_internal(&self, id: i64) -> Option<bool> {
        self.crawl.nodes.get(&id).map(|r| r.internal)
    }

    /// Numeric data actually present in this snapshot; unavailable API metrics are absent.
    pub fn available_metrics(&self) -> BTreeSet<String> {
        self.crawl
            .nodes
            .values()
            .flat_map(|r| r.metrics.keys().cloned())
            .collect()
    }

    /// Breadth-first projection, with exact counts for collapsed/budgeted descendants.
    pub fn graph(&self, options: &GraphOptions) -> Result<Graph, os_store::StoreError> {
        if !(1..=50_000).contains(&options.budget) {
            return Err(invalid("budget must be between 1 and 50000"));
        }
        let tree = if options.directory {
            &self.directories
        } else {
            &self.crawl
        };
        if options.root.is_some_and(|r| !tree.nodes.contains_key(&r)) {
            return Err(invalid("unknown graph root"));
        }
        let roots = options.root.map_or_else(|| tree.roots.clone(), |r| vec![r]);
        // Promote children of excluded URLs, preserving their discovery order.
        let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut projected_roots = Vec::new();
        let mut order = Vec::new();
        let mut queue: VecDeque<_> = roots.into_iter().map(|r| (r, None)).collect();
        while let Some((id, parent)) = queue.pop_front() {
            let record = &tree.nodes[&id];
            let eligible = record.node.address.is_none()
                || ((options.include_non_html || record.html)
                    && (options.include_non_indexable
                        || record.node.indexability.as_deref() == Some("Indexable")));
            let next_parent = if eligible {
                match parent {
                    Some(parent) => children.entry(parent).or_default().push(id),
                    None => projected_roots.push(id),
                }
                order.push(id);
                Some(id)
            } else {
                parent
            };
            for child in tree.children.get(&id).into_iter().flatten() {
                queue.push_back((*child, next_parent));
            }
        }
        let mut descendants = HashMap::<i64, u64>::new();
        for id in order.iter().rev() {
            let count = children
                .get(id)
                .into_iter()
                .flatten()
                .map(|c| 1 + descendants.get(c).copied().unwrap_or(0))
                .sum();
            descendants.insert(*id, count);
        }
        let mut visible = Vec::new();
        let mut selected = BTreeSet::new();
        let mut queue: VecDeque<_> = projected_roots.into_iter().map(|r| (r, 0)).collect();
        while let Some((id, depth)) = queue.pop_front() {
            if visible.len() == options.budget as usize {
                break;
            }
            selected.insert(id);
            visible.push(id);
            if depth < options.expand_depth {
                for child in children.get(&id).into_iter().flatten() {
                    queue.push_back((*child, depth + 1));
                }
            }
        }
        for explore in options
            .expanded_nodes
            .iter()
            .copied()
            .chain(options.explore_node)
        {
            if !selected.contains(&explore) {
                return Err(invalid("explore node must be visible"));
            }
            let extra: Vec<_> = children
                .get(&explore)
                .into_iter()
                .flatten()
                .filter(|c| !selected.contains(c))
                .copied()
                .collect();
            if visible.len() + extra.len() > options.budget as usize + 1_000 {
                return Err(invalid(
                    "explore exceeds budget by more than 1000; focus this node instead",
                ));
            }
            for id in extra {
                selected.insert(id);
                visible.push(id);
            }
        }
        let mut visible_descendants = HashMap::<i64, u64>::new();
        for id in order.iter().rev() {
            let count = children
                .get(id)
                .into_iter()
                .flatten()
                .filter(|c| selected.contains(c))
                .map(|c| 1 + visible_descendants.get(c).copied().unwrap_or(0))
                .sum();
            visible_descendants.insert(*id, count);
        }
        let size_key = metric_key(&options.metric);
        let color_key = options.color_metric.as_deref().map(metric_key);
        let mut nodes = Vec::with_capacity(visible.len());
        let mut edges = Vec::with_capacity(visible.len());
        for id in visible {
            let record = &tree.nodes[&id];
            let mut node = record.node.clone();
            node.metric = record.metrics.get(&size_key).copied();
            node.color_value = color_key
                .as_ref()
                .and_then(|k| record.metrics.get(k))
                .copied();
            for child in children.get(&id).into_iter().flatten() {
                if selected.contains(child) {
                    edges.push((id, *child));
                } else {
                    node.hidden_children += 1;
                    node.hidden_descendants += 1 + descendants[child];
                }
            }
            // Include hidden descendants below visible children as well.
            node.hidden_descendants = descendants[&id] - visible_descendants[&id];
            nodes.push(node);
        }
        let truncated = nodes.iter().any(|n| n.hidden_children > 0);
        Ok(Graph {
            nodes,
            edges,
            truncated,
        })
    }
}
