//! All app-exit entry points share the session quit hook.
use crate::{
    dto::BoolArgs,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::error::{parse_args, to_json},
    App, CallCtx,
};
use futures_util::future::BoxFuture;
fn request<'a>(app: &'a App, _: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { to_json(&app.request_quit().await) })
}
fn confirm<'a>(app: &'a App, _: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: BoolArgs = parse_args(args)?;
        crate::quit::confirm(app, a.value).await?;
        Ok(serde_json::Value::Null)
    })
}
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "app.requestQuit",
            args: "void",
            ret: "QuitDecision",
            long_running: false,
        },
        request,
    );
    r.register(
        CommandMeta {
            name: "app.confirmQuit",
            args: "BoolArgs",
            ret: "void",
            long_running: false,
        },
        confirm,
    );
}
