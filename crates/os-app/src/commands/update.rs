//! STUB(M3-U01)/STUB(M3-U02): explicit Unsupported responses until update services land.
use crate::{
    dto::CmdError,
    registry::{CmdResult, CommandMeta, Handler, Registry},
    runtime::{App, CallCtx},
};
use futures_util::future::BoxFuture;
use serde_json::Value;

macro_rules! stub {
    ($handler:ident, $command:literal, $owner:literal) => {
        fn $handler<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
            Box::pin(async { Err(CmdError::unsupported($command, $owner)) })
        }
    };
}
stub!(check, "update.check", "M3-U01");
stub!(status, "update.status", "M3-U01");
stub!(download, "update.download", "M3-U02");
stub!(install, "update.install", "M3-U02");
stub!(set_auto, "update.set_auto", "M3-U01");

pub fn register(r: &mut Registry) {
    for (name, args, ret, handler) in [
        ("update.check", "void", "UpdateState", check as Handler),
        ("update.status", "void", "UpdateState", status as Handler),
        (
            "update.download",
            "void",
            "UpdateState",
            download as Handler,
        ),
        (
            "update.install",
            "UpdateInstallArgs",
            "UpdateState",
            install as Handler,
        ),
        (
            "update.set_auto",
            "UpdateSetAutoArgs",
            "void",
            set_auto as Handler,
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
