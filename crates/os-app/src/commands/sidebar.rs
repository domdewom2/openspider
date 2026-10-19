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
        let args: ScopeArgs = parse_args(args)?;
        catalog::check_scope(&args.segment, &args.compare)?;
        let s = session(app, &ctx)?;
        app.read_pool()
            .run_snapshot(s.key, move |file, _| {
                let summary = os_store::queries::app_support::overview_summary(file.conn())?;
                let summary = summary_rows(summary);
                let registry = os_analyze::tabs::configured_for_file(&s.config, file);
                Ok(
                    os_analyze::views::filter_counts_in(&registry, file, &s.config)
                        .map_err(view_error)
                        .and_then(|counts| {
                            let rows = catalog::count_rows(&registry, &s.config, &counts);
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
                            to_json(&OverviewDto { summary, tabs })
                        }),
                )
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
        app.read_pool()
            .run_snapshot(s.key, move |file, _| {
                Ok(os_analyze::views::filter_counts(file, &s.config)
                    .map_err(view_error)
                    .and_then(|counts| to_json(&issue_rows(&counts, &args))))
            })
            .await?
    })
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
