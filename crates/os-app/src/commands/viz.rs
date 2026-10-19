//! Graph-Schnappschüsse je Fenster und CSV-Export (M2-K16).
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures_util::future::BoxFuture;
use os_analyze::graph::{GraphOptions, Snapshot, WordOptions, WordUnit};

use crate::dto::*;
use crate::pool::CrawlKey;
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{internal, no_crawl, parse_args, store_error, to_json};
use crate::runtime::lifecycle_bus::CrawlEvent;
use crate::{App, CallCtx, WindowId};

#[derive(Clone)]
struct Entry {
    data: Arc<Snapshot>,
    key: CrawlKey,
    window: WindowId,
    owner: WindowId,
    root: Option<i64>,
    last_graph: Option<GraphDto>,
    last_options: Option<GraphOptions>,
    notified: bool,
    wake: Arc<tokio::sync::Notify>,
}

/// Zustand je App; der Fensterschluss-Hook gibt Schnappschüsse frei.
#[derive(Default)]
pub struct VizState {
    entries: Mutex<HashMap<String, Entry>>,
    init: OnceLock<()>,
}
impl VizState {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn state(app: &App) -> Arc<VizState> {
    let state = app.ext::<VizState>();
    state.init.get_or_init(|| {
        app.on_window_closed(Box::new(|app, window| {
            app.ext::<VizState>().lock().retain(|_, e| {
                if &e.window == window {
                    e.wake.notify_one();
                    false
                } else {
                    true
                }
            });
        }));
        app.crawl_events().subscribe(Box::new(|app, event| {
            let key = match event {
                CrawlEvent::Started(key)
                | CrawlEvent::Paused(key)
                | CrawlEvent::Resumed(key)
                | CrawlEvent::Stopped(key)
                | CrawlEvent::Completed(key)
                | CrawlEvent::Opened(key)
                | CrawlEvent::Closed(key)
                | CrawlEvent::Cleared(key)
                | CrawlEvent::ConfigChanged { key, .. } => key,
            };
            for entry in app
                .ext::<VizState>()
                .lock()
                .values()
                .filter(|e| &e.key == key && !e.notified)
            {
                entry.wake.notify_one();
            }
        }));
    });
    state
}
fn stale() -> CmdError {
    CmdError::new(ErrorCode::Stale, "err-viz-snapshot-closed")
}
fn invalid(field: &str) -> CmdError {
    CmdError::invalid_input("err-invalid-input", field)
}
fn entry(app: &App, id: &str) -> Result<Entry, CmdError> {
    state(app).lock().get(id).cloned().ok_or_else(stale)
}
fn graph_kind(kind: VizKind) -> Result<bool, CmdError> {
    match kind {
        VizKind::CrawlTree | VizKind::CrawlGraph => Ok(false),
        VizKind::DirectoryTree => Ok(true),
        _ => Err(CmdError::new(ErrorCode::Unsupported, "err-viz-kind")),
    }
}
fn window_name(id: &str) -> String {
    // Der F05-Vertrag kodiert Snapshot-ID-Bytes im Fensterlabel hexadezimal.
    format!(
        "viz-{}",
        id.as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn open<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: VizOpenArgs = parse_args(args)?;
        if !matches!(args.kind, VizKind::AnchorWords) {
            graph_kind(args.kind)?;
        }
        let previous = args
            .refresh_snapshot_id
            .as_deref()
            .map(|id| entry(app, id))
            .transpose()?;
        let (key, owner, root) = if let Some(ref previous) = previous {
            (
                previous.key.clone(),
                previous.owner.clone(),
                args.root_url_id
                    .or_else(|| previous.last_options.as_ref().and_then(|o| o.root))
                    .or(previous.root),
            )
        } else {
            let session = app
                .sessions()
                .current(&ctx.window)
                .ok_or_else(|| no_crawl(&ctx.window.0))?;
            (session.key, ctx.window.clone(), args.root_url_id)
        };
        let data = Arc::new(
            app.read_pool()
                .run_with(key.clone(), ctx.cancel, |file, _| Snapshot::capture(file))
                .await?,
        );
        if root.is_some_and(|id| !data.contains_node(id, args.kind == VizKind::DirectoryTree)) {
            return Err(invalid("rootUrlId"));
        }
        let id = args.refresh_snapshot_id.unwrap_or_else(|| {
            app.config()
                .ids
                .new_crawl_id(app.config().clock.now())
                .to_string()
        });
        let window = previous
            .as_ref()
            .map_or_else(|| WindowId(window_name(&id)), |e| e.window.clone());
        let snapshot = Entry {
            data,
            key,
            owner,
            window: window.clone(),
            root,
            last_graph: None,
            last_options: None,
            notified: false,
            wake: Arc::new(tokio::sync::Notify::new()),
        };
        {
            let state = state(app);
            let mut entries = state.lock();
            if let Some(ref old) = previous {
                if entries
                    .get(&id)
                    .is_none_or(|e| !Arc::ptr_eq(&old.data, &e.data))
                {
                    return Err(stale());
                }
            }
            if let Some(old) = entries.insert(id.clone(), snapshot.clone()) {
                old.wake.notify_one();
            }
        }
        if previous.is_none() {
            if let Some(host) = app.window_host() {
                if let Err(err) = host.open_viz(&id) {
                    state(app).lock().remove(&id);
                    return Err(err);
                }
            }
        }
        watch(app, id.clone(), &snapshot);
        if snapshot.data.provenance_missing {
            notice(app, &window, "viz-provenance-missing", &id);
        }
        to_json(&VizOpenResult {
            snapshot_id: id,
            window: window.0,
        })
    })
}

fn notice(app: &App, window: &WindowId, code: &str, id: &str) -> bool {
    app.send_event(
        window,
        UiEvent::Notice {
            notice: Notice {
                level: NoticeLevel::Info,
                code: code.into(),
                args: BTreeMap::from([("snapshotId".into(), serde_json::json!(id))]),
            },
        },
    )
}
fn watch(app: &App, id: String, snapshot: &Entry) {
    let weak = app.downgrade();
    let wake = snapshot.wake.clone();
    app.handle().spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                _ = wake.notified() => {},
            }
            let Some(app) = weak.upgrade() else {
                return;
            };
            let Ok(old) = entry(&app, &id) else {
                return;
            };
            if old.notified || !Arc::ptr_eq(&wake, &old.wake) {
                return;
            }
            let stamp = app
                .read_pool()
                .run(old.key.clone(), |file, _| {
                    Ok((file.get_meta("change_seq")?, file.get_meta("reset_seq")?))
                })
                .await;
            let Ok((change, reset)) = stamp else {
                continue;
            };
            let parse = |s: Option<String>| s.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            if (parse(change), parse(reset)) == (old.data.change_seq, old.data.reset_seq) {
                continue;
            }
            let window = {
                let state = state(&app);
                let mut entries = state.lock();
                let Some(current) = entries.get_mut(&id) else {
                    return;
                };
                if !Arc::ptr_eq(&old.data, &current.data) || current.notified {
                    return;
                }
                current.notified = true;
                current.window.clone()
            };
            if !notice(&app, &window, "viz-data-changed", &id) {
                // Bis zur erfolgreichen Zustellung warten, falls das Fenster noch nicht abonniert ist.
                let state = state(&app);
                let mut entries = state.lock();
                if let Some(current) = entries.get_mut(&id) {
                    if Arc::ptr_eq(&old.data, &current.data) {
                        current.notified = false;
                    }
                }
            } else {
                return;
            }
        }
    });
}

fn graph<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: VizGraphArgs = parse_args(args)?;
        let old = entry(app, &args.snapshot_id)?;
        let mut options = GraphOptions {
            directory: graph_kind(args.kind)?,
            root: args.root.or(old.root),
            budget: args.budget,
            expand_depth: args.expand_depth.unwrap_or(3),
            explore_node: args.explore_node,
            expanded_nodes: Vec::new(),
            include_non_html: args.include_non_html.unwrap_or(false),
            include_non_indexable: args.include_non_indexable.unwrap_or(true),
            metric: args.metric,
            color_metric: args.color_metric,
        };
        if let (Some(_), Some(previous)) = (options.explore_node, old.last_options.as_ref()) {
            let mut base = previous.clone();
            base.explore_node = options.explore_node;
            base.expanded_nodes.clear();
            if base == options {
                options.expanded_nodes = previous.expanded_nodes.clone();
                if let Some(id) = previous.explore_node {
                    if !options.expanded_nodes.contains(&id) {
                        options.expanded_nodes.push(id);
                    }
                }
            }
        }
        let saved_options = options.clone();
        let data = old.data.clone();
        let graph = tokio::task::spawn_blocking(move || data.graph(&options))
            .await
            .map_err(|e| internal("viz.graph", e.to_string()))?
            .map_err(|e| store_error(&e))?;
        if ctx.cancel.is_cancelled() {
            return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
        }
        let dto = GraphDto {
            available_metrics: old.data.available_metrics().into_iter().collect(),
            provenance_missing: old.data.provenance_missing,
            nodes: graph
                .nodes
                .into_iter()
                .map(|n| GraphNode {
                    id: n.id,
                    label: n.label,
                    address: n.address,
                    depth: n.depth,
                    metric: n.metric,
                    color_value: n.color_value,
                    x: None,
                    y: None,
                    has_more: n.hidden_children > 0,
                    status_code: n.status_code,
                    indexability: n.indexability,
                    unique_inlinks: n.unique_inlinks,
                    link_score: n.link_score,
                    segment_id: n.segment_id,
                    segment_color: n.segment_color,
                    hidden_children: n.hidden_children,
                    hidden_descendants: n.hidden_descendants,
                })
                .collect(),
            edges: graph
                .edges
                .into_iter()
                .map(|(source, target)| GraphEdge { source, target })
                .collect(),
            truncated: graph.truncated,
        };
        {
            let state = state(app);
            let mut entries = state.lock();
            let current = entries.get_mut(&args.snapshot_id).ok_or_else(stale)?;
            if !Arc::ptr_eq(&old.data, &current.data) {
                return Err(stale());
            }
            current.last_graph = Some(dto.clone());
            current.last_options = Some(saved_options);
        }
        to_json(&dto)
    })
}

fn words<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: VizWordsArgs = parse_args(args)?;
        if args.kind != WordSource::Anchors {
            return Err(CmdError::new(ErrorCode::Unsupported, "err-viz-kind"));
        }
        let options = WordOptions {
            unit: match args.mode.as_deref().unwrap_or("phrase") {
                "phrase" => WordUnit::Phrase,
                "word" => WordUnit::Word,
                _ => return Err(invalid("mode")),
            },
            remove_stopwords: args.remove_stopwords.unwrap_or(true),
            language: args.language.unwrap_or_else(|| "en".into()),
            max_terms: args.max_terms.unwrap_or(100),
            include_nofollow: args.include_nofollow.unwrap_or(true),
            exclude_self_links: args.exclude_self_links.unwrap_or(false),
        };
        let unsupported_language = options
            .stopwords_unavailable()
            .then(|| options.language.clone());
        let snapshot = entry(app, &args.snapshot_id)?.data;
        let terms = tokio::task::spawn_blocking(move || snapshot.words(args.url_id, &options))
            .await
            .map_err(|e| internal("viz.words", e.to_string()))?
            .map_err(|e| store_error(&e))?;
        if let Some(language) = unsupported_language {
            app.send_event(
                &ctx.window,
                UiEvent::Notice {
                    notice: Notice {
                        level: NoticeLevel::Info,
                        code: "viz-stopwords-unavailable".into(),
                        args: BTreeMap::from([("language".into(), serde_json::json!(language))]),
                    },
                },
            );
        }
        to_json(
            &terms
                .into_iter()
                .map(|(word, count)| WordFreq { word, count })
                .collect::<Vec<_>>(),
        )
    })
}

fn focus<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: UrlIdArgs = parse_args(args)?;
        let state = state(app);
        let snapshot = state
            .lock()
            .values()
            .find(|e| e.window == ctx.window)
            .cloned();
        let (key, internal) = if let Some(snapshot) = snapshot {
            (
                snapshot.key,
                snapshot
                    .data
                    .is_internal(args.url_id)
                    .ok_or_else(|| invalid("urlId"))?,
            )
        } else {
            let s = app
                .sessions()
                .current(&ctx.window)
                .ok_or_else(|| no_crawl(&ctx.window.0))?;
            let url = app
                .read_pool()
                .run(s.key.clone(), move |file, _| {
                    os_store::queries::url_by_id(file.conn(), os_model::UrlId(args.url_id))
                })
                .await?;
            (s.key, url.ok_or_else(|| invalid("urlId"))?.is_internal)
        };
        let main = app
            .sessions()
            .current(&WindowId("main".into()))
            .ok_or_else(|| no_crawl("main"))?;
        if main.key != key {
            return Err(CmdError::new(
                ErrorCode::Stale,
                "err-viz-main-crawl-changed",
            ));
        }
        if let Some(host) = app.window_host() {
            host.focus_main();
        }
        app.send_event(
            &WindowId("main".into()),
            UiEvent::FocusUrl {
                crawl_id: key.0,
                url_id: args.url_id,
                is_internal: internal,
            },
        );
        to_json(&())
    })
}

fn export<'a>(app: &'a App, _ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let mut args: VizExportArgs = parse_args(args)?;
        let snapshot = entry(app, &args.snapshot_id)?;
        let displayed = snapshot.last_graph.ok_or_else(|| invalid("nodes"))?;
        let expected_edges: BTreeSet<_> = displayed
            .edges
            .iter()
            .map(|e| (e.source, e.target))
            .collect();
        let actual_edges: BTreeSet<_> = args.edges.iter().map(|e| (e.source, e.target)).collect();
        if args.nodes.len() != displayed.nodes.len()
            || args.edges.len() != expected_edges.len()
            || actual_edges != expected_edges
        {
            return Err(invalid("nodes"));
        }
        let expected: HashMap<_, _> = displayed.nodes.iter().map(|n| (n.id, n)).collect();
        let mut seen = BTreeSet::new();
        for node in &mut args.nodes {
            if !seen.insert(node.id)
                || [node.x, node.y]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite())
            {
                return Err(invalid("nodes"));
            }
            let frozen = expected.get(&node.id).ok_or_else(|| invalid("nodes"))?;
            let (x, y) = (node.x, node.y);
            *node = (*frozen).clone();
            node.x = x;
            node.y = y;
        }
        args.edges = displayed.edges;
        let op = app
            .ops()
            .spawn("op-viz-export", true, move |cancel, progress| {
                Box::pin(async move {
                    tokio::task::spawn_blocking(move || export_csv(args, cancel, progress))
                        .await
                        .map_err(|e| internal("viz.exportCsv", e.to_string()))?
                })
            });
        to_json(&op)
    })
}

fn export_csv(
    args: VizExportArgs,
    cancel: os_analyze::analysis::CancelToken,
    progress: crate::ops::ProgressSink,
) -> CmdResult {
    use std::io::Write;
    let io = |e: std::io::Error| CmdError::new(ErrorCode::Io, "err-io").with_detail(e.to_string());
    let check = || {
        if cancel.is_cancelled() {
            Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
        } else {
            Ok(())
        }
    };
    check()?;
    let dir = std::path::PathBuf::from(&args.dir);
    if args.dir.is_empty() {
        return Err(invalid("dir"));
    }
    std::fs::create_dir_all(&dir).map_err(io)?;
    let parents: HashMap<_, _> = args.edges.iter().map(|e| (e.target, e.source)).collect();
    let total = (args.nodes.len() + args.edges.len()) as u64;
    let nonce = uuid::Uuid::new_v4();
    let nodes_path = dir.join(format!(".nodes-{nonce}.tmp"));
    let edges_path = dir.join(format!(".edges-{nonce}.tmp"));
    let result = (|| {
        let mut nodes = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&nodes_path)
                .map_err(io)?,
        );
        let mut edges = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&edges_path)
                .map_err(io)?,
        );
        nodes.write_all(b"Id,Address,Parent Id,Depth,Status Code,Indexability,Segment,Size Metric,Color Value,x,y\r\n").map_err(io)?;
        edges
            .write_all(b"Source Id,Target Id,Edge Type\r\n")
            .map_err(io)?;
        for n in &args.nodes {
            check()?;
            let fields = [
                n.id.to_string(),
                n.address.clone().unwrap_or_default(),
                opt(parents.get(&n.id)),
                n.depth.to_string(),
                opt(n.status_code),
                n.indexability.clone().unwrap_or_default(),
                n.segment_id.clone().unwrap_or_default(),
                opt(n.metric),
                opt(n.color_value),
                opt(n.x),
                opt(n.y),
            ];
            nodes.write_all(csv_row(&fields).as_bytes()).map_err(io)?;
            progress.advance(1);
        }
        for e in &args.edges {
            check()?;
            writeln!(edges, "{},{},tree\r", e.source, e.target).map_err(io)?;
            progress.advance(1);
        }
        nodes.flush().map_err(io)?;
        edges.flush().map_err(io)?;
        drop(nodes);
        drop(edges);
        let bytes = std::fs::metadata(&nodes_path).map_err(io)?.len()
            + std::fs::metadata(&edges_path).map_err(io)?.len();
        check()?;
        std::fs::rename(&nodes_path, dir.join("nodes.csv")).map_err(io)?;
        std::fs::rename(&edges_path, dir.join("edges.csv")).map_err(io)?;
        progress.report(total, Some(total));
        to_json(&FileResult {
            path: args.dir,
            bytes,
        })
    })();
    let _ = std::fs::remove_file(nodes_path);
    let _ = std::fs::remove_file(edges_path);
    result
}
fn opt<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}
fn csv_row(fields: &[String]) -> String {
    let fields: Vec<_> = fields
        .iter()
        .map(|v| {
            if v.contains([',', '"', '\r', '\n']) {
                format!("\"{}\"", v.replace('"', "\"\""))
            } else {
                v.clone()
            }
        })
        .collect();
    format!("{}\r\n", fields.join(","))
}

/// Eingefrorenen F05-Command-Katalog und Operationsverträge beibehalten.
pub fn register(r: &mut Registry) {
    for (name, args, ret, long_running, handler) in [
        (
            "viz.open",
            "VizOpenArgs",
            "VizOpenResult",
            false,
            open as crate::registry::Handler,
        ),
        (
            "viz.graph",
            "VizGraphArgs",
            "GraphDto",
            false,
            graph as crate::registry::Handler,
        ),
        (
            "viz.words",
            "VizWordsArgs",
            "WordFreq[]",
            false,
            words as crate::registry::Handler,
        ),
        (
            "viz.exportCsv",
            "VizExportArgs",
            "FileResult",
            true,
            export as crate::registry::Handler,
        ),
        (
            "viz.focusInMain",
            "UrlIdArgs",
            "void",
            false,
            focus as crate::registry::Handler,
        ),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running,
            },
            handler,
        );
    }
}
