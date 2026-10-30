//! STUB(M3-U01/M3-U02): explicit Unsupported responses until update services land.
use crate::{
    dto::{CmdError, ErrorCode},
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{App, CallCtx},
};
use futures_util::future::BoxFuture;
use serde_json::Value;

fn unavailable<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async {
        Err(
            CmdError::new(ErrorCode::Unsupported, "err-update-unavailable")
                .with_detail("STUB(M3-U01/M3-U02)"),
        )
    })
}
pub fn register(r: &mut Registry) {
    for (name, args, ret) in [
        ("update.check", "void", "UpdateState"),
        ("update.status", "void", "UpdateState"),
        ("update.download", "void", "UpdateState"),
        ("update.install", "UpdateInstallArgs", "UpdateState"),
        ("update.set_auto", "UpdateSetAutoArgs", "void"),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            unavailable,
        );
    }
}
