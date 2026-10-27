//! Accessibility node inspection (TECH-259).
use super::catalog::session;
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;
fn node<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: A11yNodeArgs = parse_args(args)?;
        let s = session(app, &ctx)?;
        if let Some(gate) =
            super::view::detail_gate(os_analyze::details::DetailKind::A11yNodes, &s.config)
        {
            return Err(
                CmdError::new(ErrorCode::Unsupported, "err-detail-unavailable")
                    .with_arg("state", serde_json::to_value(gate).unwrap_or_default()),
            );
        }
        app.read_pool()
            .run_snapshot(s.key, move |file, _| {
                let result =
                    os_analyze::tabs::accessibility::node(file, args.url_id, args.node_id)?;
                let result = result.map(|n| A11yNodeDetail {
                    summary_key: format!("a11y-rule-{}", n.rule_id),
                    rule_id: n.rule_id,
                    impact: n
                        .impact
                        .map(|value| os_analyze::tabs::accessibility::impact(Some(&value))),
                    target: n.target,
                    snippet: n.snippet,
                });
                Ok(match result {
                    Some(result) => to_json(&result),
                    None => Err(CmdError::new(
                        ErrorCode::NotFound,
                        "err-a11y-node-not-found",
                    )),
                })
            })
            .await?
    })
}
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "detail.a11yNode",
            args: "A11yNodeArgs",
            ret: "A11yNodeDetail",
            long_running: false,
        },
        node,
    );
}
