//! Overview and issues share the analyze filter counts and issue resources.
use super::catalog::{self, session, view_error};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use crate::pool::CrawlKey;
use futures_util::future::BoxFuture;
/// UI-061 summary shares one denominator and rounding rule with its export.
pub(crate) fn summary_rows(summary: Vec<(String, u64)>) -> Vec<SummaryRow> {
    let encountered = summary.first().map_or(0, |r| r.1);
    summary
        .into_iter()
        .map(|(key, count)| SummaryRow {
            key,
            count,
            pct: (encountered > 0)
                .then(|| (count as f64 * 1000.0 / encountered as f64).round() / 10.0),
        })
        .collect()
}
fn overview<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        if args.get("comparison").and_then(serde_json::Value::as_bool) == Some(true) {
            return super::compare::ui::overview(app, ctx).await;
        }
        let args: ScopeArgs = parse_args(args)?;
        if let Some(result) = super::compare::change_overview(app, &ctx, &args, true).await {
            return result;
        }
        let change_rows = super::compare::change_counts(app, &ctx, args.compare).await?;
        let compare_scope = if matches!(
            args.compare,
            Some(CompareClass::Changed | CompareClass::NotChanged)
        ) {
            None
        } else {
            args.compare
        };
        catalog::check_scope(&args.segment, &compare_scope)?;
        let s = session(app, &ctx)?;
        let facts = overview_facts(app, &s, ctx.cancel.clone()).await?;
        app.read_pool()
            .run_snapshot(s.key, move |file, _| {
                let summary = summary_rows(facts.summary.clone());
                let registry = os_analyze::tabs::configured_for_file(&s.config, file);
                let mut rows = catalog::count_rows(&registry, &s.config, &facts.counts);
                rows.retain(|r| r.tab != "Change Detection");
                rows.extend(change_rows);
                let tabs = registry
                    .iter()
                    .filter_map(|t| {
                        let filters = rows
                            .iter()
                            .filter(|r| r.tab == t.id)
                            .map(|r| r.filter.clone())
                            .collect::<Vec<_>>();
                        (!filters.is_empty()).then(|| OverviewTab {
                            tab: t.id.into(),
                            filters,
                        })
                    })
                    .collect();
                Ok(to_json(&OverviewDto { summary, tabs }))
            })
            .await?
    })
}
pub(crate) fn issue_rows(
    counts: &[os_analyze::views::FilterCount],
    args: &IssuesArgs,
) -> Vec<IssueRow> {
    let search = args.search.as_deref().unwrap_or("").to_lowercase();
    os_analyze::issues::issues_from_counts(counts)
        .into_iter()
        .map(|r| IssueRow {
            id: format!("{}/{}", r.tab, r.filter),
            tab: r.tab,
            filter: r.filter,
            issue_type: catalog::issue_type(r.issue_type),
            priority: catalog::priority(r.priority),
            count: r.count,
            pct: r.pct,
            segment_shares: vec![],
            delta: None,
        })
        .filter(|r| {
            args.issue_type.is_none_or(|t| t == r.issue_type)
                && args.priority.is_none_or(|p| p == r.priority)
                && format!("{}: {}", r.tab, r.filter)
                    .to_lowercase()
                    .contains(&search)
        })
        .collect()
}
fn issues<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: IssuesArgs = parse_args(args)?;
        catalog::check_scope(&args.segment, &args.compare)?;
        let s = session(app, &ctx)?;
        let facts = overview_facts(app, &s, ctx.cancel.clone()).await?;
        to_json(&issue_rows(&facts.counts, &args))
    })
}

/// UI-145/UI-147: Overview and Issues share one fact set per data version. Missing pieces are
/// counted by one cancellable operation per crawl ([`count_op`], progress in `Tick.ops`, UI-141);
/// a request joins it instead of scanning the crawl a second time. Cancelling the operation ends
/// the waiting requests with `Cancelled`; the next request resumes from the stored pieces.
pub(crate) async fn overview_facts(
    app: &App,
    s: &crate::runtime::sessions::SessionInfo,
    cancel: os_model::CancelToken,
) -> Result<std::sync::Arc<os_analyze::views::OverviewFacts>, CmdError> {
    let cfg = s.config.clone();
    let progress = app
        .read_pool()
        .run_cached(s.key.clone(), "overview", cancel.clone(), move |file, cancel| {
            Ok(os_analyze::views::overview_progress(file, &cfg, cancel, 0))
        })
        .await?
        .map_err(view_error)?;
    if !progress.complete() {
        let op = count_op(app, s);
        let status = crate::views::until_cancelled(&cancel, app.ops().wait(&op)).await?;
        match status.map(|st| (st.state, st.error)) {
            Some((OpState::Cancelled, _)) => {
                return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"))
            }
            Some((OpState::Failed, Some(error))) => return Err(error),
            _ => {}
        }
    }
    let cfg = s.config.clone();
    let (facts, pending) = app
        .read_pool()
        .run_cached(s.key.clone(), "overview", cancel, move |file, cancel| {
            Ok(os_analyze::views::overview_facts(file, &cfg, cancel))
        })
        .await?
        .map_err(view_error)?;
    if pending {
        crate::views::persist_facts(app, &s.key);
    }
    Ok(facts)
}

/// Label (i18n key) of the counting operation.
pub(crate) const COUNT_LABEL: &str = "sidebar-op-count";

/// Running counting operation per crawl.
#[derive(Default)]
pub(crate) struct CountOps(std::sync::Mutex<std::collections::HashMap<CrawlKey, String>>);
impl CountOps {
    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<CrawlKey, String>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Cancels the crawl's counting operation (closed, cleared or started crawl).
pub(crate) fn cancel_count(app: &App, key: &CrawlKey) {
    if let Some(op) = app.ext::<CountOps>().lock().remove(key) {
        app.ops().cancel(&op);
    }
}

/// Starts the one-time count of the missing Overview/Issues pieces as a cancellable operation, or
/// joins the running one. Up to four tabs per step (counted at once); every step is stored before
/// the next, so an interrupted count (cancel, closed crawl, ended process) resumes where it stopped.
/// Progress: counted tabs of all tabs.
pub(crate) fn count_op(app: &App, s: &crate::runtime::sessions::SessionInfo) -> String {
    let registry = app.ext::<CountOps>();
    let mut running = registry.lock();
    if let Some(op) = running.get(&s.key) {
        if app
            .ops()
            .status(op)
            .is_some_and(|st| st.state == OpState::Running)
        {
            return op.clone();
        }
    }
    let (worker, session) = (app.clone(), s.clone());
    let handle = app.ops().spawn(COUNT_LABEL, true, move |token, sink| {
        Box::pin(async move {
            let mut step = 0;
            loop {
                let cfg = session.config.clone();
                let progress = worker
                    .read_pool()
                    .run_cached(
                        session.key.clone(),
                        "overview",
                        token.clone(),
                        move |file, cancel| {
                            Ok(os_analyze::views::overview_progress(file, &cfg, cancel, step))
                        },
                    )
                    .await?
                    .map_err(view_error)?;
                sink.report(progress.done as u64, Some(progress.total as u64));
                // Stored pieces are an optional cache: a read-only crawl keeps them in memory.
                let stored = worker
                    .write_queue()
                    .submit_file(&session.key, |file| {
                        os_analyze::views::persist_facts(file).map_err(view_error)
                    })
                    .await;
                if let Err(error) = stored {
                    tracing::debug!(?error, "Overview-Zähler nicht gespeichert");
                }
                if progress.complete() {
                    return Ok(serde_json::Value::Null);
                }
                if token.is_cancelled() {
                    return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
                }
                // First report immediately (stored pieces only), then four tabs per step.
                step = 4;
            }
        })
    });
    running.insert(s.key.clone(), handle.op_id.clone());
    handle.op_id
}
fn explain<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: IssueExplainArgs = parse_args(args)?;
        let cfg = app
            .sessions()
            .current(&ctx.window)
            .map(|s| s.config)
            .unwrap_or_default();
        let entry = os_analyze::issues::catalog()
            .iter()
            .find(|e| {
                e.id == args.issue_id
                    || e.doc_key == args.issue_id
                    || format!("{}/{}", e.tab, e.filter) == args.issue_id
            })
            .ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-issue-not-found"))?;
        let registry = os_analyze::tabs::configured(&cfg);
        let params = registry
            .iter()
            .find(|t| t.id == entry.tab)
            .map(|t| (t.params)(&cfg))
            .unwrap_or_default();
        let lang = if args.lang == "de" {
            os_analyze::issues::Lang::De
        } else {
            os_analyze::issues::Lang::En
        };
        let text = os_analyze::issues::texts(&entry.doc_key, lang, &params)
            .ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-issue-text-not-found"))?;
        to_json(&IssueText {
            what: text.what,
            why: text.why,
            fix: text.fix,
            rule: Some(format!("{} / {}", entry.type_rule, entry.priority_rule)),
            learn_more_url: Some(format!("docs/issues/{}.md", entry.doc_key)),
        })
    })
}
pub fn register(r: &mut Registry) {
    for (name, args, ret, h) in [
        (
            "sidebar.overview",
            "ScopeArgs",
            "OverviewDto",
            overview as crate::registry::Handler,
        ),
        ("sidebar.issues", "IssuesArgs", "IssueRow[]", issues),
        (
            "sidebar.issueExplain",
            "IssueExplainArgs",
            "IssueText",
            explain,
        ),
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
