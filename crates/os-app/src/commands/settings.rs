//! settings.get/set (OUT-222).
//!
//! `settings.get` liefert die App-Einstellungen (`settings.toml`) als `AppSettingsDto`; `settings.set` wendet einen
//! JSON-Merge-Patch an, schreibt die Datei atomar, meldet `settingsChanged` an alle Fenster und nennt in
//! `restartRequired` die Schlüssel, deren Änderung einen Neustart erfordert.

use futures_util::future::BoxFuture;

use crate::dto::{CmdError, ErrorCode, SettingsPatch};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{parse_args, to_json};
use crate::runtime::{App, CallCtx};

fn get<'a>(app: &'a App, _ctx: CallCtx, _args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { to_json(&app.settings().dto(&[])) })
}

fn set<'a>(app: &'a App, _ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let SettingsPatch { patch } = parse_args(args)?;
        let app = app.clone();
        // Dateizugriff nicht auf einem Tokio-Worker
        let dto = tokio::task::spawn_blocking(move || app.apply_settings_patch(&patch))
            .await
            .map_err(|e| {
                CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
            })??;
        to_json(&dto)
    })
}

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "settings.get",
            args: "void",
            ret: "AppSettingsDto",
            long_running: false,
        },
        get,
    );
    r.register(
        CommandMeta {
            name: "settings.set",
            args: "SettingsPatch",
            ret: "AppSettingsDto",
            long_running: false,
        },
        set,
    );
}
