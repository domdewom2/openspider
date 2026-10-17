//! events.* (UI-018, UI-143).
//!
//! * `events.subscribe`: braucht einen Kanal (Tauri `Channel`, Bridge-WebSocket) und läuft deshalb nicht über
//!   `App::dispatch`, sondern über `App::subscribe` (Hülle bzw. Bridge). Über `dispatch` antwortet der Command mit
//!   `Unsupported`/`err-events-subscribe-needs-channel`.
//! * `events.setVisibility`: Sichtbarkeit aus dem Frontend (`visibilitychange`); die Hülle meldet ihre Quelle über
//!   `App::set_visibility`. Sichtbar ist ein Fenster nur, wenn beide sichtbar melden (≤ 1 Hz sonst).
//! * `events.setInterval`: Live-Intervall 250 – 5000 ms.

use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::dto::{CmdError, ErrorCode, SetIntervalArgs, SetVisibilityArgs};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::parse_args;
use crate::runtime::{App, CallCtx, WindowId};

fn subscribe<'a>(_app: &'a App, _ctx: CallCtx, _args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async {
        Err(CmdError::new(
            ErrorCode::Unsupported,
            "err-events-subscribe-needs-channel",
        ))
    })
}

fn set_visibility<'a>(app: &'a App, _ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: SetVisibilityArgs = parse_args(args)?;
        app.set_frontend_visibility(WindowId(a.window), a.visible);
        Ok(Value::Null)
    })
}

fn set_interval<'a>(app: &'a App, _ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: SetIntervalArgs = parse_args(args)?;
        app.set_live_interval_ms(a.ms)?;
        Ok(Value::Null)
    })
}

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "events.subscribe",
            args: "SubscribeArgs",
            ret: "SubscriptionId",
            long_running: false,
        },
        subscribe,
    );
    r.register(
        CommandMeta {
            name: "events.setVisibility",
            args: "SetVisibilityArgs",
            ret: "void",
            long_running: false,
        },
        set_visibility,
    );
    r.register(
        CommandMeta {
            name: "events.setInterval",
            args: "SetIntervalArgs",
            ret: "void",
            long_running: false,
        },
        set_interval,
    );
}
