//! `view.*`: tab views run on the K-1 view sessions (`crate::views`, M2-K05); detail and link views keep their
//! offset queries. The command API is unchanged from M2-K06.
use super::catalog::{self, session, view_error};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        sessions::SessionInfo,
        App, CallCtx, WindowId,
    },
    views::{Built, LinkView, TabView},
};
use futures_util::future::BoxFuture;
use os_analyze::{
    defs,
    details::{DetailKind, DetailReq},
    views,
};
use os_model::{CrawlConfig, UrlId};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

/// Builds that take longer return `Building` with a cancellable operation (UI-141: indicator after 300 ms).
pub const SYNC_BUILD: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub(crate) struct OpenView {
    pub owner: WindowId,
    pub session: SessionInfo,
    pub spec: ViewSpec,
    pub columns: Vec<String>,
    pub version: u64,
    /// K-1 session of a tab view; its published state overrides the fields above.
    pub tab: Option<Arc<TabView>>,
    /// Materialized link IDs of an inlinks/outlinks/resources view (K-1g).
    pub links: Option<Arc<LinkView>>,
}
/// Link tables with a materialized ID list; the anchor aggregation stays a detail source.
fn link_list(app: &App, s: &SessionInfo, spec: &ViewSpec) -> Option<Arc<LinkView>> {
    (matches!(
        spec,
        ViewSpec::Inlinks { .. } | ViewSpec::Outlinks { .. } | ViewSpec::Resources { .. }
    ) && detail_spec(spec).is_none())
    .then(|| LinkView::new(app, s.key.clone()))
}
#[derive(Default)]
struct Views {
    initialized: AtomicBool,
    entries: Mutex<HashMap<String, OpenView>>,
}
fn registry(app: &App) -> std::sync::Arc<Views> {
    let views = app.ext::<Views>();
    if !views.initialized.swap(true, Ordering::SeqCst) {
        app.on_window_closed(Box::new(|app, window| {
            app.ext::<Views>()
                .entries
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .retain(|_, v| &v.owner != window);
        }));
        app.crawl_events().subscribe(Box::new(|app, event| {
            use crate::runtime::lifecycle_bus::CrawlEvent;
            if let CrawlEvent::Closed(key) | CrawlEvent::Cleared(key) = event {
                app.ext::<Views>()
                    .entries
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .retain(|_, v| &v.session.key != key);
            }
        }));
    }
    views
}
fn get_owned(app: &App, ctx: &CallCtx, id: &str) -> Result<OpenView, CmdError> {
    let mut v = registry(app)
        .entries
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(id)
        .cloned()
        .filter(|v| v.owner == ctx.window)
        .ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-view-not-found"))?;
    if let Some(p) = v.tab.as_ref().and_then(|t| t.published()) {
        v.session = p.session;
        v.spec = p.spec;
        v.columns = p.columns;
        v.version = p.version;
    }
    if session(app, ctx)?.key != v.session.key {
        return Err(CmdError::new(ErrorCode::Stale, "err-view-stale"));
    }
    Ok(v)
}
pub(crate) fn get(app: &App, ctx: &CallCtx, id: &str) -> Result<OpenView, CmdError> {
    let view = get_owned(app, ctx, id)?;
    if session(app, ctx)?.config != view.session.config {
        return Err(stale());
    }
    Ok(view)
}
pub(crate) fn stale() -> CmdError {
    CmdError::new(ErrorCode::Stale, "err-view-stale")
}
pub(crate) fn sorts(sort: &[SortKey]) -> Vec<defs::SortKey> {
    sort.iter()
        .map(|s| defs::SortKey {
            column: s.column.clone(),
            descending: s.dir == SortDir::Desc,
        })
        .collect()
}
fn rule(node: &RuleNode) -> os_analyze::search::RuleNode {
    use os_analyze::search as s;
    match node {
        RuleNode::Group { op, children } => {
            let nodes = children.iter().map(rule).collect();
            if *op == RuleBool::And {
                s::RuleNode::And(nodes)
            } else {
                s::RuleNode::Or(nodes)
            }
        }
        RuleNode::Cond { column, op, value } => {
            let core = match op {
                RuleOp::Contains => s::RuleOp::Contains,
                RuleOp::NotContains => s::RuleOp::NotContains,
                RuleOp::Equals => s::RuleOp::Equals,
                RuleOp::NotEquals => s::RuleOp::NotEquals,
                RuleOp::StartsWith => s::RuleOp::StartsWith,
                RuleOp::EndsWith => s::RuleOp::EndsWith,
                RuleOp::Regex | RuleOp::NotRegex => s::RuleOp::Regex,
                RuleOp::GreaterThan => s::RuleOp::Greater,
                RuleOp::LessThan => s::RuleOp::Less,
                RuleOp::IsEmpty => s::RuleOp::IsEmpty,
                RuleOp::IsNotEmpty => s::RuleOp::IsNotEmpty,
            };
            let node = s::RuleNode::Rule(s::Rule {
                column: column.clone().unwrap_or_default(),
                op: core,
                value: value.clone().unwrap_or_default(),
            });
            if *op == RuleOp::NotRegex {
                s::RuleNode::Not(Box::new(node))
            } else {
                node
            }
        }
    }
}
pub(crate) fn tab_query(
    spec: &ViewSpec,
    columns: &[String],
    offset: u64,
    limit: u32,
) -> Result<views::ViewQuery, CmdError> {
    if let ViewSpec::Tab {
        segment, compare, ..
    } = spec
    {
        catalog::check_scope(segment, compare)?;
    }
    query_of(spec, columns, offset, limit)
}
/// K-1 session specification of a tab view. Segment and comparison come from `views::RestrictProvider`; the
/// search is evaluated as a Unicode bitmap restriction with the build's cancel token (K2-38).
pub(crate) fn session_spec(
    app: &App,
    s: &SessionInfo,
    spec: &ViewSpec,
    columns: &[String],
    cancel: os_model::CancelToken,
) -> Result<views::session::SessionSpec, CmdError> {
    let ViewSpec::Tab {
        segment, compare, ..
    } = spec
    else {
        return Err(CmdError::invalid_input("err-tab-required", "spec"));
    };
    let mut restrict = crate::views::restrictions(app, &s.key, segment.as_ref(), compare.as_ref())?;
    let q = query_of(spec, columns, 0, 1)?;
    if let Some(search) = os_analyze::search::SearchFilter::new(&q, &s.config, cancel) {
        restrict.push(Arc::new(search));
    }
    Ok(views::session::SessionSpec {
        tab: q.tab,
        filter: q.filter,
        search: None,
        sort: q.sort,
        columns: q.columns,
        restrict,
    })
}
fn query_of(
    spec: &ViewSpec,
    columns: &[String],
    offset: u64,
    limit: u32,
) -> Result<views::ViewQuery, CmdError> {
    let ViewSpec::Tab {
        tab,
        filter,
        search,
        sort,
        ..
    } = spec
    else {
        return Err(CmdError::invalid_input("err-tab-required", "spec"));
    };
    let mut q = views::ViewQuery::new(tab, filter);
    q.columns = if columns.is_empty() {
        views::ColumnSelection::Default
    } else {
        views::ColumnSelection::Named(columns.to_vec())
    };
    q.sort = sorts(sort);
    q.limit = limit;
    q.start = views::PageStart::Offset(offset);
    q.search = search.as_ref().map(|s| match s {
        SearchSpec::Simple { text } => views::SearchSpec::Simple(text.clone()),
        SearchSpec::Rules {
            tree,
            case_sensitive,
        } => views::SearchSpec::Rules {
            tree: rule(tree),
            case_sensitive: *case_sensitive,
        },
    });
    Ok(q)
}
pub(crate) fn detail_kind(detail: DetailTable) -> DetailKind {
    match detail {
        DetailTable::Headers => DetailKind::Headers,
        DetailTable::Cookies => DetailKind::Cookies,
        DetailTable::ConsoleLog => DetailKind::ConsoleLog,
        DetailTable::BlockedResources => DetailKind::BlockedResources,
        DetailTable::Network => DetailKind::Network,
        DetailTable::Spelling => DetailKind::Spelling,
        DetailTable::A11yRules => DetailKind::A11yRules,
        DetailTable::A11yNodes => DetailKind::A11yNodes,
        DetailTable::Duplicates => DetailKind::Duplicates,
        DetailTable::NGrams => DetailKind::NGrams,
        DetailTable::StructuredDataItems => DetailKind::StructuredDataItems,
        DetailTable::StructuredDataIssues => DetailKind::StructuredDataIssues,
        DetailTable::RedirectChain => DetailKind::RedirectChain,
        DetailTable::SitemapMembership => DetailKind::SitemapMembership,
        DetailTable::CustomSearch => DetailKind::CustomSearch,
        DetailTable::CustomExtraction => DetailKind::CustomExtraction,
        DetailTable::CustomJs => DetailKind::CustomJs,
        DetailTable::ApiValues => DetailKind::ApiValues,
        DetailTable::Forms => DetailKind::Forms,
        DetailTable::Hreflang => DetailKind::Hreflang,
        DetailTable::CrawlPath => DetailKind::CrawlPath,
        DetailTable::Anchors => DetailKind::Anchors,
    }
}
pub(crate) fn detail_gate(kind: DetailKind, cfg: &CrawlConfig) -> Option<EmptyState> {
    if matches!(
        kind,
        DetailKind::StructuredDataItems | DetailKind::StructuredDataIssues
    ) && !(cfg.extraction.structured_data.json_ld
        || cfg.extraction.structured_data.microdata
        || cfg.extraction.structured_data.rdfa)
    {
        return Some(catalog::empty(
            EmptyCode::FeatureDisabled,
            Some("extraction.structured_data".into()),
        ));
    }
    let feature = match kind {
        DetailKind::Headers => Some("extraction.http_headers"),
        DetailKind::Cookies => Some("extraction.cookies"),
        DetailKind::StructuredDataItems | DetailKind::StructuredDataIssues => None,
        DetailKind::Forms => Some("extraction.forms"),
        _ => None,
    };
    if let Some(k) = feature.filter(|k| !catalog::config_on(cfg, k)) {
        return Some(catalog::empty(EmptyCode::FeatureDisabled, Some(k.into())));
    }
    // M2-D19 (UI-047): ohne Rechtschreib- und Grammatikprüfung bleibt der Detail-Tab leer.
    if kind == DetailKind::Spelling && !cfg.spelling.enabled && !cfg.grammar.enabled {
        return Some(catalog::empty(
            EmptyCode::FeatureDisabled,
            Some("spelling.enabled".into()),
        ));
    }
    if matches!(
        kind,
        DetailKind::ConsoleLog
            | DetailKind::Network
            | DetailKind::BlockedResources
            | DetailKind::A11yNodes
            | DetailKind::A11yRules
    ) && cfg.rendering.mode != os_model::RenderingMode::JavaScript
    {
        return Some(catalog::empty(EmptyCode::NeedsJsRendering, None));
    }
    None
}
pub(crate) fn detail_spec(
    spec: &ViewSpec,
) -> Option<(
    DetailKind,
    Vec<UrlId>,
    serde_json::Value,
    Vec<defs::SortKey>,
)> {
    match spec {
        ViewSpec::Detail {
            detail,
            url_ids,
            params,
            sort,
        } => Some((
            detail_kind(*detail),
            url_ids.iter().copied().map(UrlId).collect(),
            params.clone(),
            sorts(sort),
        )),
        ViewSpec::Images {
            url_ids,
            search,
            sort,
        } => Some((
            DetailKind::ImageInstances,
            url_ids.iter().copied().map(UrlId).collect(),
            serde_json::json!({"search":search}),
            sorts(sort),
        )),
        ViewSpec::Inlinks {
            url_ids,
            filter,
            sort,
            search,
            ..
        }
        | ViewSpec::Outlinks {
            url_ids,
            filter,
            sort,
            search,
            ..
        } if filter.mode == Some(LinkMode::Anchors) => Some((
            DetailKind::Anchors,
            url_ids.iter().copied().map(UrlId).collect(),
            serde_json::json!({"incoming":matches!(spec,ViewSpec::Inlinks{..}),"excludeSelf":filter.exclude_self.unwrap_or(true),"excludeNofollow":filter.exclude_nofollow.unwrap_or(false),"position":filter.position,"anchor":filter.anchor,"search":search}),
            sorts(sort),
        )),
        _ => None,
    }
}
pub(crate) fn link_selection(
    spec: &ViewSpec,
    cfg: &CrawlConfig,
    offset: u64,
    limit: u32,
) -> Result<
    (
        os_store::queries::app_support::LinkSelection,
        os_store::queries::PageReq,
    ),
    CmdError,
> {
    use os_store::queries::{app_support::LinkSelection, LinkSortKey, PageReq};
    let (ids, filter, search, sort, incoming, resources) = match spec {
        ViewSpec::Inlinks {
            url_ids,
            filter,
            search,
            sort,
            compare,
        }
        | ViewSpec::Outlinks {
            url_ids,
            filter,
            search,
            sort,
            compare,
        } => {
            catalog::check_scope(&None, compare)?;
            (
                url_ids,
                Some(filter),
                search,
                sort,
                matches!(spec, ViewSpec::Inlinks { .. }),
                false,
            )
        }
        ViewSpec::Resources {
            url_ids,
            filter,
            search,
            sort,
        } => {
            if filter.id != "All" {
                return Err(CmdError::invalid_input("err-unknown-filter", "filter"));
            }
            (url_ids, None, search, sort, false, true)
        }
        _ => return Err(CmdError::invalid_input("err-links-required", "spec")),
    };
    let mut selection = LinkSelection {
        url_ids: ids.iter().copied().map(UrlId).collect(),
        incoming,
        resources,
        exclude_resources: !incoming && !resources,
        search: search.clone(),
        include_uncrawlable: cfg.spider.store_uncrawlable_links,
        ..Default::default()
    };
    let mut page = PageReq::new(offset, limit);
    if let Some(f) = filter {
        match f.id.as_str() {
            "All" => {}
            "Hyperlinks" => selection.link_type = Some("Hyperlink".into()),
            "Images" => selection.link_type = Some("Image".into()),
            "Internal" => selection.internal = Some(true),
            "External" => selection.internal = Some(false),
            "Uncrawlable" => selection.uncrawlable_only = true,
            "Without Self Links" => selection.exclude_self = true,
            "Without Nofollow" => selection.exclude_nofollow = true,
            _ => return Err(CmdError::invalid_input("err-unknown-filter", "filter")),
        };
        selection.exclude_self |= f.exclude_self.unwrap_or(false);
        selection.exclude_nofollow |= f.exclude_nofollow.unwrap_or(false);
        selection.anchor = f.anchor.clone();
        page.position = f.position.clone();
    }
    page.sort = sort
        .iter()
        .map(|s| LinkSortKey {
            column: match s.column.as_str() {
                "From" => "Source",
                "To" => "Destination",
                "Anchor Text" => "Anchor",
                x => x,
            }
            .into(),
            desc: s.dir == SortDir::Desc,
        })
        .collect();
    Ok((selection, page))
}
fn tab_empty(state: os_model::FilterState, total: u64, searching: bool) -> Option<EmptyState> {
    match state {
        os_model::FilterState::NeedsConfig(k) => {
            Some(catalog::empty(EmptyCode::FeatureDisabled, Some(k)))
        }
        os_model::FilterState::NeedsRendering => {
            Some(catalog::empty(EmptyCode::NeedsJsRendering, None))
        }
        os_model::FilterState::NeedsCrawlAnalysis | os_model::FilterState::Stale => {
            Some(catalog::empty(EmptyCode::NeedsCrawlAnalysis, None))
        }
        os_model::FilterState::NeedsRecrawl => Some(catalog::empty(
            EmptyCode::NoDataYet,
            Some("needsRecrawl".into()),
        )),
        _ if total == 0 => Some(catalog::empty(
            if searching {
                EmptyCode::NoMatch
            } else {
                EmptyCode::NoDataYet
            },
            None,
        )),
        _ => None,
    }
}
/// Cells of columns with a missing prerequisite stay empty.
fn cells(rows: Vec<Vec<os_model::Value>>, meta: &[ColumnMeta]) -> Vec<Vec<serde_json::Value>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .zip(meta)
                .map(|(v, c)| {
                    if c.needs.is_some() {
                        serde_json::Value::Null
                    } else {
                        serde_json::to_value(v).unwrap_or_default()
                    }
                })
                .collect()
        })
        .collect()
}
#[derive(Clone)]
pub(crate) struct DataPage {
    pub keys: Vec<i64>,
    pub columns: Vec<ColumnMeta>,
    pub cells: Vec<Vec<serde_json::Value>>,
    pub total: u64,
    pub version: u64,
    pub empty: Option<EmptyState>,
}
pub(crate) async fn page(
    app: &App,
    s: &SessionInfo,
    spec: ViewSpec,
    columns: Vec<String>,
    offset: u64,
    limit: u32,
    expected: Option<u64>,
) -> Result<DataPage, CmdError> {
    page_query(
        app,
        s,
        spec,
        columns,
        offset,
        limit,
        PageSelection {
            expected,
            ids: None,
            ordered: false,
        },
    )
    .await
}
/// Explicit URL/link IDs are restricted in SQL before counting and paging.
pub(crate) async fn selected_page(
    app: &App,
    v: &OpenView,
    columns: Vec<String>,
    ids: Vec<i64>,
    offset: u64,
    limit: u32,
) -> Result<DataPage, CmdError> {
    page_query(
        app,
        &v.session,
        v.spec.clone(),
        columns,
        offset,
        limit,
        PageSelection {
            expected: Some(v.version),
            ids: Some(ids),
            ordered: false,
        },
    )
    .await
}
struct PageSelection {
    expected: Option<u64>,
    ids: Option<Vec<i64>>,
    /// `ids` are already a window of a materialized link list in view order (read by primary key).
    ordered: bool,
}
/// Window of a materialized link list (K-1g): rows by primary key in list order.
async fn ordered_link_page(
    app: &App,
    v: &OpenView,
    columns: Vec<String>,
    ids: Vec<i64>,
    limit: u32,
) -> Result<DataPage, CmdError> {
    page_query(
        app,
        &v.session,
        v.spec.clone(),
        columns,
        0,
        limit,
        PageSelection {
            expected: Some(v.version),
            ids: Some(ids),
            ordered: true,
        },
    )
    .await
}
async fn page_query(
    app: &App,
    s: &SessionInfo,
    spec: ViewSpec,
    columns: Vec<String>,
    offset: u64,
    limit: u32,
    selection: PageSelection,
) -> Result<DataPage, CmdError> {
    let PageSelection {
        expected,
        ids: selected_ids,
        ordered,
    } = selection;
    #[cfg(test)]
    app.ext::<PageReads>().0.fetch_add(1, Ordering::Relaxed);
    if !(1..=500).contains(&limit) {
        return Err(CmdError::invalid_input("err-invalid-limit", "limit"));
    }
    let cfg = s.config.clone();
    app.read_pool()
        .run_snapshot(s.key.clone(), move |file, _| {
            let version = os_store::queries::change_seq(file.conn())?;
            Ok((|| {
                if expected.is_some_and(|v| v != version) {
                    return Err(stale());
                }
                if let ViewSpec::Tab {
                    tab,
                    filter,
                    search,
                    ..
                } = &spec
                {
                    let mut q = tab_query(&spec, &columns, offset, limit)?;
                    if columns.is_empty() {
                        q.columns = views::ColumnSelection::Default;
                    }
                    let registry = os_analyze::tabs::configured_for_file(&cfg, file);
                    let p = if let Some(ids) = &selected_ids {
                        views::query_url_ids_in(
                            &registry,
                            file,
                            &cfg,
                            &q,
                            &ids.iter().copied().map(UrlId).collect::<Vec<_>>(),
                            false,
                        )
                    } else {
                        views::query_view_in(&registry, file, &cfg, &q)
                    }
                    .map_err(view_error)?;
                    let t = os_analyze::names::find_tab(&registry, tab).expect("resolved");
                    let meta = catalog::expanded_columns(t, &p.columns, &cfg);
                    let state = views::state_of_in(&registry, file, &cfg, tab, filter)
                        .map_err(view_error)?;
                    let empty = tab_empty(state, p.total, search.is_some());
                    let cells = cells(p.rows, &meta);
                    return Ok(DataPage {
                        keys: p.row_keys.iter().map(|id| id.0).collect(),
                        columns: meta,
                        cells,
                        total: p.total,
                        version,
                        empty,
                    });
                }
                let (names, kinds, rows, keys, total, mut empty) =
                    if let Some((kind, ids, params, sort)) = detail_spec(&spec) {
                        let gate = if ids.is_empty() {
                            Some(catalog::empty(EmptyCode::NoSelection, None))
                        } else {
                            detail_gate(kind, &cfg)
                        };
                        if gate.is_some() {
                            return Ok(DataPage {
                                keys: vec![],
                                columns: vec![],
                                cells: vec![],
                                total: 0,
                                version,
                                empty: gate,
                            });
                        }
                        let req = DetailReq {
                            url_ids: &ids,
                            params: &params,
                            sort: &sort,
                            offset,
                            limit: u64::from(limit),
                        };
                        let (_, source) = os_analyze::details::app_detail_source(kind);
                        let p = source(file, &cfg, &req).map_err(view_error)?;
                        let (names, kinds): (Vec<_>, Vec<_>) = p.columns.into_iter().unzip();
                        let keys = (0..p.rows.len())
                            .map(|i| {
                                offset
                                    .saturating_add(i as u64)
                                    .saturating_add(1)
                                    .min(i64::MAX as u64) as i64
                            })
                            .collect();
                        (names, kinds, p.rows, keys, p.total, None)
                    } else {
                        let (mut selection, req) = link_selection(&spec, &cfg, offset, limit)?;
                        let p = match selected_ids {
                            Some(ids) if ordered => {
                                os_store::queries::app_support::selected_links_by_ids(
                                    file.conn(),
                                    &selection,
                                    &ids,
                                )
                            }
                            ids => {
                                selection.row_ids = ids;
                                os_store::queries::app_support::selected_links(
                                    file.conn(),
                                    &selection,
                                    &req,
                                )
                            }
                        }
                        .map_err(|e| crate::runtime::error::store_error(&e))?;
                        let names = [
                            "Type",
                            "From",
                            "To",
                            "Size (Bytes)",
                            "Alt Text",
                            "Anchor Text",
                            "Status Code",
                            "Status",
                            "Follow",
                            "Target",
                            "Rel",
                            "Path Type",
                            "Link Path",
                            "Link Position",
                            "Link Origin",
                            "Link Crawlability",
                            "Uncrawlable Reason",
                            "Original URL",
                            "PDF Page",
                        ]
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>();
                        let kinds = (0..names.len())
                            .map(|i| match i {
                                1 | 2 | 17 => defs::ColumnKind::Url,
                                3 | 6 | 18 => defs::ColumnKind::Int,
                                8 => defs::ColumnKind::Bool,
                                _ => defs::ColumnKind::Text,
                            })
                            .collect();
                        let empty = selection
                            .url_ids
                            .is_empty()
                            .then(|| catalog::empty(EmptyCode::NoSelection, None));
                        (names, kinds, p.rows, p.keys, p.total, empty)
                    };
                let meta = names
                    .iter()
                    .zip(kinds)
                    .enumerate()
                    .map(|(i, (n, k))| {
                        let mut c = defs::ColumnDef::dynamic(n, "NULL", k, true);
                        defs::apply_meta_defaults(std::slice::from_mut(&mut c));
                        if (i >= 16
                            && matches!(
                                spec,
                                ViewSpec::Inlinks { .. }
                                    | ViewSpec::Outlinks { .. }
                                    | ViewSpec::Resources { .. }
                            ))
                            || (n == "Link Crawlability"
                                && matches!(spec, ViewSpec::Inlinks { .. }))
                        {
                            c.extension = true;
                            c.default_visible = false;
                        }
                        // M2-D19 (ONPAGE-157): `Page Section` und `Rule ID` erst nach Einblenden.
                        if matches!(
                            spec,
                            ViewSpec::Detail {
                                detail: DetailTable::Spelling,
                                ..
                            }
                        ) && os_analyze::tabs::content::SPELLING_DETAIL_EXTENSIONS
                            .contains(&n.as_str())
                        {
                            c.extension = true;
                            c.default_visible = false;
                        }
                        if matches!(spec, ViewSpec::Images { .. }) {
                            if matches!(n.as_str(), "Alt Text Length" | "Source") {
                                c.extension = true;
                                c.default_visible = false;
                            }
                            if n == "Display Dimensions (WxH)"
                                || (n == "Real Dimensions (WxH)"
                                    && !cfg.images.real_dimensions_from_header)
                            {
                                c.needs = Some(defs::Needs::Rendering);
                            }
                        }
                        catalog::column(&c, n, &cfg)
                    })
                    .collect::<Vec<_>>();
                let selected = if columns.is_empty() {
                    meta.iter()
                        .enumerate()
                        .filter(|(_, c)| c.default_visible)
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>()
                } else {
                    columns
                        .iter()
                        .map(|name| {
                            names.iter().position(|n| n == name).ok_or_else(|| {
                                CmdError::invalid_input("err-unknown-column", "columns")
                                    .with_arg("column", name.clone())
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                let cells = rows
                    .iter()
                    .map(|row| {
                        selected
                            .iter()
                            .map(|i| serde_json::to_value(&row[*i]).unwrap_or_default())
                            .collect()
                    })
                    .collect();
                if empty.is_none() && total == 0 {
                    empty = Some(catalog::empty(EmptyCode::NoDataYet, None));
                }
                Ok(DataPage {
                    keys,
                    columns: selected.iter().map(|i| meta[*i].clone()).collect(),
                    cells,
                    total,
                    version,
                    empty,
                })
            })())
        })
        .await?
}
fn open<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ViewOpenArgs = parse_args(args)?;
        let s = session(app, &ctx)?;
        let id = app
            .config()
            .ids
            .new_crawl_id(app.config().clock.now())
            .to_string();
        if matches!(args.spec, ViewSpec::Tab { .. }) {
            let tab = TabView::new(app, id.clone(), s.key.clone());
            let built = start_build(
                app,
                &tab,
                s.clone(),
                args.spec.clone(),
                args.columns.clone(),
            );
            let h = settle(app, &id, &args.spec, built).await?;
            insert(
                app,
                id,
                OpenView {
                    owner: ctx.window,
                    session: s,
                    spec: args.spec,
                    columns: args.columns,
                    version: h.version,
                    tab: Some(tab),
                    links: None,
                },
            );
            return to_json(&h);
        }
        let p = page(app, &s, args.spec.clone(), args.columns.clone(), 0, 1, None).await?;
        let h = handle(id.clone(), p);
        let links = link_list(app, &s, &args.spec);
        insert(
            app,
            id,
            OpenView {
                owner: ctx.window,
                session: s,
                spec: args.spec,
                columns: args.columns,
                version: h.version,
                tab: None,
                links,
            },
        );
        to_json(&h)
    })
}
async fn link_ids(app: &App, v: &OpenView, links: &LinkView) -> Result<Arc<Vec<i64>>, CmdError> {
    let (selection, page) = link_selection(&v.spec, &v.session.config, 0, 1)?;
    links.ids(app, v.version, selection, page).await
}
fn insert(app: &App, id: String, view: OpenView) {
    registry(app)
        .entries
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(id, view);
}
type Building = tokio::task::JoinHandle<Result<Built, CmdError>>;
/// Starts a private build; the published view stays readable until it succeeds (double buffer).
fn start_build(
    app: &App,
    tab: &Arc<TabView>,
    s: SessionInfo,
    spec: ViewSpec,
    columns: Vec<String>,
) -> (Building, os_model::CancelToken) {
    let token = os_model::CancelToken::default();
    let (app, tab, cancel) = (app.clone(), tab.clone(), token.clone());
    let task = app
        .handle()
        .spawn(async move { tab.build(&app, s, spec, columns, cancel).await });
    (task, token)
}
/// Waits up to [`SYNC_BUILD`]; slower builds continue as a cancellable operation (`ViewState::Building`).
async fn settle(
    app: &App,
    id: &str,
    spec: &ViewSpec,
    (mut task, token): (Building, os_model::CancelToken),
) -> Result<ViewHandle, CmdError> {
    match tokio::time::timeout(SYNC_BUILD, &mut task).await {
        Ok(joined) => {
            let built = joined
                .map_err(|e| crate::runtime::error::internal("view.build", e.to_string()))??;
            Ok(tab_handle(id.to_string(), spec, built))
        }
        Err(_) => {
            let (view_id, spec) = (id.to_string(), spec.clone());
            let op = app.ops().spawn("view-build", true, move |cancel, _| {
                Box::pin(async move {
                    let forward = token.clone();
                    let _bridge = cancel.on_cancel(move || forward.cancel());
                    if cancel.is_cancelled() {
                        token.cancel();
                    }
                    let built = task.await.map_err(|e| {
                        crate::runtime::error::internal("view.build", e.to_string())
                    })??;
                    to_json(&tab_handle(view_id, &spec, built))
                })
            });
            Ok(ViewHandle {
                view_id: id.to_string(),
                version: 0,
                total: None,
                columns: Vec::new(),
                state: ViewState::Building,
                op: Some(op),
                empty_state: None,
            })
        }
    }
}
fn tab_handle(view_id: String, spec: &ViewSpec, built: Built) -> ViewHandle {
    let searching = matches!(
        spec,
        ViewSpec::Tab {
            search: Some(_),
            ..
        }
    );
    ViewHandle {
        view_id,
        version: built.version,
        total: Some(built.total),
        empty_state: tab_empty(built.state, built.total, searching),
        columns: built.meta,
        state: ViewState::Ready,
        op: None,
    }
}
fn handle(view_id: String, p: DataPage) -> ViewHandle {
    ViewHandle {
        view_id,
        version: p.version,
        total: Some(p.total),
        columns: p.columns,
        state: ViewState::Ready,
        op: None,
        empty_state: p.empty,
    }
}
fn update<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ViewUpdateArgs = parse_args(args)?;
        let mut v = get_owned(app, &ctx, &args.view_id)?;
        v.session = session(app, &ctx)?;
        let tab = match (&args.spec, v.tab.take()) {
            (ViewSpec::Tab { .. }, Some(tab)) => Some(tab),
            (ViewSpec::Tab { .. }, _) => Some(TabView::new(
                app,
                args.view_id.clone(),
                v.session.key.clone(),
            )),
            _ => None,
        };
        let h = if let Some(tab) = &tab {
            let built = start_build(
                app,
                tab,
                v.session.clone(),
                args.spec.clone(),
                v.columns.clone(),
            );
            settle(app, &args.view_id, &args.spec, built).await?
        } else {
            let p = page(
                app,
                &v.session,
                args.spec.clone(),
                v.columns.clone(),
                0,
                1,
                None,
            )
            .await?;
            handle(args.view_id.clone(), p)
        };
        let state = registry(app);
        let mut entries = state.entries.lock().unwrap_or_else(|p| p.into_inner());
        if !entries.contains_key(&args.view_id) {
            return Err(CmdError::new(ErrorCode::NotFound, "err-view-not-found"));
        }
        v.links = link_list(app, &v.session, &args.spec);
        v.spec = args.spec;
        v.version = h.version;
        v.tab = tab;
        entries.insert(args.view_id.clone(), v);
        to_json(&h)
    })
}
fn rows<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: RowsArgs = parse_args(args)?;
        let v = get(app, &ctx, &args.view_id)?;
        if !(1..=500).contains(&args.limit) {
            return Err(CmdError::invalid_input("err-invalid-limit", "limit"));
        }
        if args.version != v.version {
            return Err(stale());
        }
        if let Some(tab) = &v.tab {
            let w = tab.rows(app, args.version, args.offset, args.limit).await?;
            let projected = w.meta.iter().map(|c| c.key.clone()).collect::<Vec<_>>();
            if !args.columns.is_empty() && args.columns != projected {
                // Other columns than the projection: same row keys, values read by ID.
                let p = selected_page(app, &v, args.columns, w.keys.clone(), 0, args.limit).await?;
                let by_key = p.keys.iter().zip(p.cells).collect::<HashMap<_, _>>();
                let width = p.columns.len();
                return to_json(&RowWindow {
                    view_id: args.view_id,
                    version: args.version,
                    offset: args.offset,
                    cells: w
                        .keys
                        .iter()
                        .map(|k| {
                            by_key
                                .get(k)
                                .cloned()
                                .unwrap_or_else(|| vec![serde_json::Value::Null; width])
                        })
                        .collect(),
                    row_keys: w.keys,
                    columns: p.columns.into_iter().map(|c| c.key).collect(),
                });
            }
            return to_json(&RowWindow {
                view_id: args.view_id,
                version: args.version,
                offset: args.offset,
                row_keys: w.keys,
                columns: projected,
                cells: cells(w.rows, &w.meta),
            });
        }
        let columns = if args.columns.is_empty() {
            v.columns.clone()
        } else {
            args.columns
        };
        let p = if let Some(links) = &v.links {
            let ids = link_ids(app, &v, links).await?;
            let start = (args.offset.min(ids.len() as u64)) as usize;
            let end = start.saturating_add(args.limit as usize).min(ids.len());
            ordered_link_page(app, &v, columns, ids[start..end].to_vec(), args.limit).await?
        } else {
            page(
                app,
                &v.session,
                v.spec,
                columns,
                args.offset,
                args.limit,
                Some(args.version),
            )
            .await?
        };
        to_json(&RowWindow {
            view_id: args.view_id,
            version: p.version,
            offset: args.offset,
            row_keys: p.keys,
            columns: p.columns.into_iter().map(|c| c.key).collect(),
            cells: p.cells,
        })
    })
}
fn locate<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: LocateArgs = parse_args(args)?;
        let v = get(app, &ctx, &args.view_id)?;
        if let Some(tab) = &v.tab {
            return to_json(&tab.locate(app, v.version, args.url_id).await?);
        }
        if let Some(links) = &v.links {
            let ids = link_ids(app, &v, links).await?;
            return to_json(
                &ids.iter()
                    .position(|id| *id == args.url_id)
                    .map(|i| i as u64),
            );
        }
        let mut offset = 0;
        loop {
            let p = page(
                app,
                &v.session,
                v.spec.clone(),
                v.columns.clone(),
                offset,
                500,
                Some(v.version),
            )
            .await?;
            if let Some(i) = p.keys.iter().position(|id| *id == args.url_id) {
                return to_json(&Some(offset + i as u64));
            }
            offset += p.keys.len() as u64;
            if p.keys.is_empty() || offset >= p.total {
                return to_json(&Option::<u64>::None);
            }
        }
    })
}
fn copy<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CopyArgs = parse_args(args)?;
        crate::clipboard::copy(app, &ctx, args)
            .await
            .and_then(|r| to_json(&r))
    })
}
fn close<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ViewCloseArgs = parse_args(args)?;
        let views = registry(app);
        let mut entries = views.entries.lock().unwrap_or_else(|p| p.into_inner());
        if !entries
            .get(&args.view_id)
            .is_some_and(|v| v.owner == ctx.window)
        {
            return Err(CmdError::new(ErrorCode::NotFound, "err-view-not-found"));
        }
        entries.remove(&args.view_id);
        to_json(&())
    })
}
pub fn register(r: &mut Registry) {
    for (name, args, ret, h) in [
        (
            "view.open",
            "ViewOpenArgs",
            "ViewHandle",
            open as crate::registry::Handler,
        ),
        ("view.update", "ViewUpdateArgs", "ViewHandle", update),
        ("view.rows", "RowsArgs", "RowWindow", rows),
        ("view.locate", "LocateArgs", "number | null", locate),
        ("view.copy", "CopyArgs", "CopyResult", copy),
        ("view.close", "ViewCloseArgs", "void", close),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            h,
        );
    }
}

#[cfg(test)]
#[derive(Default)]
struct PageReads(std::sync::atomic::AtomicUsize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate as os_app;
    include!("../../tests/cmd_copy_page_fixture.rs");

    // spec: UI-012
    // spec: UI-140
    #[tokio::test]
    async fn two_selected_ids_use_one_restricted_page_in_a_large_view() {
        assert_copy_uses_one_page(|app| app.ext::<PageReads>().0.load(Ordering::Relaxed)).await;
    }
}
