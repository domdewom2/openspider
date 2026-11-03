//! Commands `crash.*` (OUT-246/OUT-247; Registrierung additiv nach K2-61).
use super::{send, service};
use crate::dto::{CrashFrontendArgs, CrashIdArgs, CrashIdsArgs, CrashSendArgs};
use crate::registry::{CmdResult, CommandMeta, Handler, Registry};
use crate::runtime::error::{parse_args, to_json};
use crate::runtime::{App, CallCtx};
use futures_util::future::BoxFuture;
use serde_json::Value;

/// Trägt die Commands ein.
pub fn register(r: &mut Registry) {
    for (name, args, ret, handler) in [
        (
            "crash.overview",
            "void",
            "CrashOverview",
            overview as Handler,
        ),
        ("crash.read", "CrashIdArgs", "string", read as Handler),
        (
            "crash.send",
            "CrashSendArgs",
            "CrashSendResult",
            send as Handler,
        ),
        ("crash.dismiss", "CrashIdsArgs", "void", dismiss as Handler),
        ("crash.delete", "CrashIdArgs", "void", delete as Handler),
        (
            "crash.reportFrontend",
            "CrashFrontendArgs",
            "boolean",
            report_frontend as Handler,
        ),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            handler,
        );
    }
}

fn overview<'a>(app: &'a App, _: CallCtx, _: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { to_json(&service::overview(app)) })
}

fn read<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CrashIdArgs = parse_args(args)?;
        to_json(&service::read(app, &args.id)?)
    })
}

fn send<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CrashSendArgs = parse_args(args)?;
        to_json(&send::send(app, args).await?)
    })
}

fn dismiss<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CrashIdsArgs = parse_args(args)?;
        service::dismiss(app, &args.ids)?;
        Ok(Value::Null)
    })
}

fn delete<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CrashIdArgs = parse_args(args)?;
        service::delete(app, &args.id)?;
        Ok(Value::Null)
    })
}

fn report_frontend<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: CrashFrontendArgs = parse_args(args)?;
        to_json(&service::report_frontend(app, &args)?)
    })
}
