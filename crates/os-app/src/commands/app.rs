//! app.info (UI-127, OUT-227).
//!
//! Version, Build, Plattform und die wirksamen Pfade. Der Exportordner folgt der Einstellung `export.directory`,
//! sonst `<Dokumente>/OpenSpider/Exports` (OUT-222); eine Änderung gilt ohne Neustart.

use futures_util::future::BoxFuture;

use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::to_json;
use crate::runtime::info::app_info;
use crate::runtime::{App, CallCtx};

fn info<'a>(app: &'a App, _ctx: CallCtx, _args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { to_json(&app_info(app.config(), &app.settings_snapshot())) })
}

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "app.info",
            args: "void",
            ret: "AppInfo",
            long_running: false,
        },
        info,
    );
}
