//! Analysis command transport (UI-080, UI-082).
use crate::{
    dto::AnalysisStartArgs,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::error::{parse_args, to_json},
    session, App, CallCtx,
};
use futures_util::future::BoxFuture;
fn start<'a>(app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: AnalysisStartArgs = parse_args(args)?;
        let s = session::current(app, &ctx.window)?;
        session::control::ensure_idle(app, &s).await?;
        to_json(&session::analysis::start_operation(app, s, a.steps, false)?)
    })
}
fn status<'a>(app: &'a App, ctx: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let s = session::current(app, &ctx.window)?;
        to_json(&session::analysis::status(app, &s).await?)
    })
}
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "analysis.start",
            args: "AnalysisStartArgs",
            ret: "AnalysisStatus",
            long_running: true,
        },
        start,
    );
    r.register(
        CommandMeta {
            name: "analysis.status",
            args: "void",
            ret: "AnalysisStatus",
            long_running: false,
        },
        status,
    );
}
