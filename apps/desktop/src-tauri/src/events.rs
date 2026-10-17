//! Ereigniskanal je Fenster: Tauri-`Channel<UiEvent>` als `EventSink` und der Command `events.subscribe`.
//!
//! Das Frontend ruft `plugin:events|subscribe` mit `{ window, channel }` auf (`channel` = ein `Channel` aus
//! `@tauri-apps/api/core`); der Kern sendet danach alle gebündelten Ticks, `settingsChanged` und Meldungen in diesen
//! Kanal. Die Sichtbarkeit meldet [`crate::window`] direkt an `App::set_visibility`.

use std::str::FromStr;
use std::sync::Arc;

use os_app::dto::{CmdError, ErrorCode, UiEvent};
use os_app::events::{EventSink, SinkClosed};
use os_app::{App, WindowId};
use tauri::ipc::{Channel, InvokeError, InvokeMessage, InvokeResolver, JavaScriptChannelId};
use tauri::Runtime;

/// Senke auf einem Tauri-Kanal.
pub struct TauriSink {
    channel: Channel<UiEvent>,
}

impl TauriSink {
    /// Verpackt den Kanal.
    pub fn new(channel: Channel<UiEvent>) -> TauriSink {
        TauriSink { channel }
    }
}

impl EventSink for TauriSink {
    fn send(&self, ev: UiEvent) -> Result<(), SinkClosed> {
        self.channel.send(ev).map_err(|_| SinkClosed)
    }
}

fn invalid(field: &str, key: &str) -> InvokeError {
    InvokeError::from(CmdError::invalid_input(key, field))
}

/// `events.subscribe`: legt das Abonnement des Fensters an und antwortet mit der `SubscriptionId`.
pub fn subscribe<R: Runtime>(
    app: &App,
    message: &InvokeMessage<R>,
    resolver: InvokeResolver<R>,
    args: &serde_json::Value,
) {
    let label = message.webview_ref().label().to_string();
    let window = args
        .get("window")
        .and_then(serde_json::Value::as_str)
        .map_or(label, str::to_string);
    let Some(raw) = args.get("channel").and_then(serde_json::Value::as_str) else {
        resolver.invoke_error(invalid("channel", "err-events-channel-missing"));
        return;
    };
    let Ok(id) = JavaScriptChannelId::from_str(raw) else {
        resolver.invoke_error(invalid("channel", "err-events-channel-invalid"));
        return;
    };
    let channel: Channel<UiEvent> = id.channel_on(message.webview());
    let sub = app.subscribe(WindowId(window), Arc::new(TauriSink::new(channel)));
    match serde_json::to_value(&sub) {
        Ok(v) => resolver.resolve(v),
        Err(e) => resolver
            .reject(CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())),
    }
}
