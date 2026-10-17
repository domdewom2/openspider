//! op.cancel (UI-141).
//!
//! Bricht eine laufende Operation ab: die Marke wird gesetzt und die laufende SQLite-Anweisung unterbrochen (≤ 100 ms).
//! Das Ergebnis erscheint als `OpState::Cancelled` im nächsten Tick. Eine unbekannte oder bereits gemeldete Operation
//! ist `NotFound` (die Oberfläche darf das ignorieren), eine nicht abbrechbare `Unsupported`.

use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::dto::{CmdError, ErrorCode, OpCancelArgs};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::parse_args;
use crate::runtime::{App, CallCtx};

fn cancel<'a>(app: &'a App, _ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let OpCancelArgs { op_id } = parse_args(args)?;
        match app.ops().is_cancellable(&op_id) {
            None => {
                Err(CmdError::new(ErrorCode::NotFound, "err-op-not-found").with_arg("opId", op_id))
            }
            Some(false) => Err(
                CmdError::new(ErrorCode::Unsupported, "err-op-not-cancellable")
                    .with_arg("opId", op_id),
            ),
            Some(true) => {
                // false: die Operation ist zwischen Prüfung und Abbruch beendet worden – kein Fehler
                app.ops().cancel(&op_id);
                Ok(Value::Null)
            }
        }
    })
}

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "op.cancel",
            args: "OpCancelArgs",
            ret: "void",
            long_running: false,
        },
        cancel,
    );
}
